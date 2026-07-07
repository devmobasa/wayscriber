//! Committed-shapes layer cache for transformed (panned) canvas rendering.
//!
//! Board pan forces full-surface damage every frame, which previously replayed
//! every committed shape through Cairo per frame. This cache bakes the board
//! background plus all committed shapes into a world-space offscreen surface
//! (view + margin) once, so pan frames become a single aligned blit; the cache
//! is rebaked only when the view escapes the baked area or content changes.
//!
//! Scope: pure pan transforms (no zoom, no frozen backdrop image). Those keep
//! the existing per-frame render path, where the backdrop transform math makes
//! world-space baking unsound.

use log::debug;

use crate::draw::{Color, ShapeId};
use crate::util::Rect;

use super::WaylandState;

/// Extra logical pixels baked around the view so short pans reuse the cache.
const CANVAS_LAYER_MARGIN: i32 = 256;
/// Upper bound for the bake surface; beyond this fall back to direct rendering.
const MAX_CACHE_BYTES: usize = 256 * 1024 * 1024;
/// Cairo's image surface dimension limit (with headroom).
const CAIRO_MAX_DIM: i32 = 32000;

pub(in crate::backend::wayland) struct CanvasLayerCache {
    surface: Option<cairo::ImageSurface>,
    /// World-space origin and logical size covered by the surface.
    world_x: i32,
    world_y: i32,
    width: i32,
    height: i32,
    scale: i32,
    content_generation: u64,
    shapes_len: usize,
    last_shape_id: Option<ShapeId>,
    background: Option<Color>,
    board_key: (usize, usize),
    valid: bool,
}

impl CanvasLayerCache {
    pub(in crate::backend::wayland) fn new() -> Self {
        Self {
            surface: None,
            world_x: 0,
            world_y: 0,
            width: 0,
            height: 0,
            scale: 1,
            content_generation: 0,
            shapes_len: 0,
            last_shape_id: None,
            background: None,
            board_key: (0, 0),
            valid: false,
        }
    }

    /// Invalidates the cache and releases the bake surface.
    pub(in crate::backend::wayland) fn clear(&mut self) {
        self.valid = false;
        self.surface = None;
    }

    /// Paints the cached layer into a context already transformed to world
    /// coordinates at the given device scale. Returns false when no valid
    /// cache is available.
    pub(in crate::backend::wayland) fn blit(&self, ctx: &cairo::Context) -> bool {
        if !self.valid {
            return false;
        }
        let Some(surface) = self.surface.as_ref() else {
            return false;
        };
        let _ = ctx.save();
        ctx.translate(self.world_x as f64, self.world_y as f64);
        let inv = 1.0 / self.scale.max(1) as f64;
        ctx.scale(inv, inv);
        let ok = ctx.set_source_surface(surface, 0.0, 0.0).is_ok();
        if ok {
            let _ = ctx.paint();
        }
        let _ = ctx.restore();
        ok
    }
}

/// Renders one committed shape with the standard eraser/blur replay handling.
/// Shared between the direct canvas render path and the layer-cache bake.
pub(in crate::backend::wayland) fn render_committed_shape(
    ctx: &cairo::Context,
    drawn_shape: &crate::draw::DrawnShape,
    replay_ctx: &crate::draw::EraserReplayContext<'_>,
) {
    match &drawn_shape.shape {
        crate::draw::Shape::EraserStroke { points, brush } => {
            crate::draw::render_eraser_stroke(ctx, points, brush, replay_ctx);
        }
        crate::draw::Shape::BlurRect {
            x,
            y,
            w,
            h,
            strength,
        } => {
            crate::draw::render_blur_rect(
                ctx,
                crate::draw::BlurRectParams {
                    x: *x,
                    y: *y,
                    w: *w,
                    h: *h,
                    strength: *strength,
                    cacheable: true,
                },
                replay_ctx,
            );
        }
        other => {
            crate::draw::render_shape(ctx, other);
        }
    }
}

fn rects_intersect(a: Rect, b: Rect) -> bool {
    let a_right = a.x.saturating_add(a.width);
    let a_bottom = a.y.saturating_add(a.height);
    let b_right = b.x.saturating_add(b.width);
    let b_bottom = b.y.saturating_add(b.height);
    !(a.x >= b_right || a_right <= b.x || a.y >= b_bottom || a_bottom <= b.y)
}

impl WaylandState {
    /// True when committed canvas content may be served from the layer cache:
    /// a board pan transform without zoom or a frozen backdrop image (whose
    /// screen-anchored transforms make world-space baking unsound).
    pub(in crate::backend::wayland) fn canvas_layer_cache_usable(&self) -> bool {
        self.canvas_transform_active() && !self.zoom.active && self.frozen.image().is_none()
    }

    /// Ensures the layer cache covers the current view with current content,
    /// rebaking when needed. Returns false when caching is unavailable and the
    /// caller must render shapes directly.
    pub(in crate::backend::wayland) fn ensure_canvas_layer_cache(
        &mut self,
        width: u32,
        height: u32,
        scale: i32,
    ) -> bool {
        let scale = scale.max(1);
        let (origin_x, origin_y) = self.canvas_view_origin();
        let view_x = origin_x.floor() as i32;
        let view_y = origin_y.floor() as i32;
        let logical_w = width.min(i32::MAX as u32) as i32;
        let logical_h = height.min(i32::MAX as u32) as i32;
        if logical_w <= 0 || logical_h <= 0 {
            return false;
        }

        let background = match self.input_state.boards.active_background() {
            crate::input::BoardBackground::Solid(color) => Some(*color),
            crate::input::BoardBackground::Transparent => None,
        };
        let board_key = (
            self.input_state.boards.active_index(),
            self.input_state.boards.active_page_index(),
        );
        let generation = self.input_state.canvas_content_generation();
        let frame = self.input_state.boards.active_frame();
        let shapes_len = frame.shapes.len();
        let last_shape_id = frame.shapes.last().map(|shape| shape.id);

        let cache = &self.canvas_layer_cache;
        let params_match = cache.valid
            && cache.surface.is_some()
            && cache.scale == scale
            && cache.content_generation == generation
            && cache.shapes_len == shapes_len
            && cache.last_shape_id == last_shape_id
            && cache.background == background
            && cache.board_key == board_key;
        let covers_view = view_x >= cache.world_x
            && view_y >= cache.world_y
            && view_x.saturating_add(logical_w) <= cache.world_x.saturating_add(cache.width)
            && view_y.saturating_add(logical_h) <= cache.world_y.saturating_add(cache.height);
        if params_match && covers_view {
            return true;
        }

        // (Re)bake centered on the current view.
        let world_x = view_x.saturating_sub(CANVAS_LAYER_MARGIN);
        let world_y = view_y.saturating_sub(CANVAS_LAYER_MARGIN);
        let bake_w = logical_w.saturating_add(CANVAS_LAYER_MARGIN * 2);
        let bake_h = logical_h.saturating_add(CANVAS_LAYER_MARGIN * 2);
        let phys_w = bake_w.saturating_mul(scale);
        let phys_h = bake_h.saturating_mul(scale);
        if phys_w <= 0 || phys_h <= 0 || phys_w > CAIRO_MAX_DIM || phys_h > CAIRO_MAX_DIM {
            self.canvas_layer_cache.clear();
            return false;
        }
        if phys_w as usize * phys_h as usize * 4 > MAX_CACHE_BYTES {
            self.canvas_layer_cache.clear();
            return false;
        }

        let reuse_surface = self
            .canvas_layer_cache
            .surface
            .as_ref()
            .is_some_and(|surface| surface.width() == phys_w && surface.height() == phys_h);
        if !reuse_surface {
            match cairo::ImageSurface::create(cairo::Format::ARgb32, phys_w, phys_h) {
                Ok(surface) => self.canvas_layer_cache.surface = Some(surface),
                Err(err) => {
                    debug!("canvas layer cache: surface allocation failed: {err}");
                    self.canvas_layer_cache.clear();
                    return false;
                }
            }
        }

        {
            let Some(surface) = self.canvas_layer_cache.surface.as_ref() else {
                return false;
            };
            let Ok(bake_ctx) = cairo::Context::new(surface) else {
                self.canvas_layer_cache.clear();
                return false;
            };

            bake_ctx.set_operator(cairo::Operator::Clear);
            let _ = bake_ctx.paint();
            bake_ctx.set_operator(cairo::Operator::Over);
            if let Some(color) = background {
                bake_ctx.set_source_rgba(color.r, color.g, color.b, color.a);
                let _ = bake_ctx.paint();
            }
            bake_ctx.scale(scale as f64, scale as f64);
            bake_ctx.translate(-(world_x as f64), -(world_y as f64));

            // Erasers clear down to the baked solid background; blur rects have
            // no backdrop image in this mode (same as the direct render path).
            let replay_ctx = crate::draw::EraserReplayContext {
                pattern: None,
                surface: None,
                backdrop_cache_key: None,
                bg_color: background,
                logical_to_image_scale_x: 1.0,
                logical_to_image_scale_y: 1.0,
            };

            let bake_bounds = Rect {
                x: world_x,
                y: world_y,
                width: bake_w,
                height: bake_h,
            };
            let frame = self.input_state.boards.active_frame();
            for drawn_shape in &frame.shapes {
                if let Some(bbox) = drawn_shape.bounding_box()
                    && rects_intersect(bbox, bake_bounds)
                {
                    render_committed_shape(&bake_ctx, drawn_shape, &replay_ctx);
                }
            }
        }
        if let Some(surface) = self.canvas_layer_cache.surface.as_ref() {
            surface.flush();
        }

        let cache = &mut self.canvas_layer_cache;
        cache.world_x = world_x;
        cache.world_y = world_y;
        cache.width = bake_w;
        cache.height = bake_h;
        cache.scale = scale;
        cache.content_generation = generation;
        cache.shapes_len = shapes_len;
        cache.last_shape_id = last_shape_id;
        cache.background = background;
        cache.board_key = board_key;
        cache.valid = true;
        debug!(
            "canvas layer cache: baked {}x{} logical at ({}, {})",
            bake_w, bake_h, world_x, world_y
        );
        true
    }
}
