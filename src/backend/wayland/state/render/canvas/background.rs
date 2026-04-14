use super::super::super::*;
use crate::draw::Color;

pub(super) struct CanvasEraserContext {
    surface: Option<cairo::ImageSurface>,
    pattern: Option<cairo::SurfacePattern>,
    backdrop_cache_key: Option<u64>,
    bg_color: Option<Color>,
    logical_to_image_scale_x: f64,
    logical_to_image_scale_y: f64,
}

impl CanvasEraserContext {
    pub(super) fn replay_context(&self) -> crate::draw::EraserReplayContext<'_> {
        crate::draw::EraserReplayContext {
            pattern: self.pattern.as_ref().map(|p| p as &cairo::Pattern),
            surface: self.surface.as_ref(),
            backdrop_cache_key: self.backdrop_cache_key,
            bg_color: self.bg_color,
            logical_to_image_scale_x: self.logical_to_image_scale_x,
            logical_to_image_scale_y: self.logical_to_image_scale_y,
        }
    }
}

impl WaylandState {
    pub(super) fn render_canvas_background(
        &mut self,
        ctx: &cairo::Context,
        scale: i32,
        phys_width: u32,
        phys_height: u32,
    ) -> Result<CanvasEraserContext> {
        let mut eraser_surface: Option<cairo::ImageSurface> = None;
        let mut eraser_pattern: Option<cairo::SurfacePattern> = None;
        let mut backdrop_cache_key: Option<u64> = None;
        let mut eraser_bg_color: Option<Color> = None;
        let mut logical_to_image_scale_x = 1.0;
        let mut logical_to_image_scale_y = 1.0;

        let allow_background_image =
            !self.zoom.is_engaged() || self.input_state.board_is_transparent();
        let zoom_render_image = if self.zoom.active && allow_background_image {
            self.zoom
                .image()
                .map(|image| (image, (self.zoom.image_generation() << 1) | 1))
                .or_else(|| {
                    self.frozen
                        .image()
                        .map(|image| (image, self.frozen.image_generation() << 1))
                })
        } else {
            None
        };
        let zoom_render_active = self.zoom.active && zoom_render_image.is_some();
        let background_image = if zoom_render_active {
            zoom_render_image
        } else if allow_background_image {
            self.frozen
                .image()
                .map(|image| (image, self.frozen.image_generation() << 1))
        } else {
            None
        };

        if let Some((image, cache_key)) = background_image {
            // SAFETY: we create a Cairo surface borrowing our owned buffer; it is dropped
            // before commit, and we hold the buffer alive via `image.data`.
            let surface = unsafe {
                cairo::ImageSurface::create_for_data_unsafe(
                    image.data.as_ptr() as *mut u8,
                    cairo::Format::ARgb32,
                    image.width as i32,
                    image.height as i32,
                    image.stride,
                )
            }
            .context("Failed to create frozen image surface")?;

            let scale_x = if image.width > 0 {
                phys_width as f64 / image.width as f64
            } else {
                1.0
            };
            let scale_y = if image.height > 0 {
                phys_height as f64 / image.height as f64
            } else {
                1.0
            };
            logical_to_image_scale_x = (scale as f64) / scale_x.max(f64::MIN_POSITIVE);
            logical_to_image_scale_y = (scale as f64) / scale_y.max(f64::MIN_POSITIVE);
            let _ = ctx.save();
            if zoom_render_active {
                let scale_x_safe = scale_x.max(f64::MIN_POSITIVE);
                let scale_y_safe = scale_y.max(f64::MIN_POSITIVE);
                let offset_x = self.zoom.view_offset.0 * (scale as f64) / scale_x_safe;
                let offset_y = self.zoom.view_offset.1 * (scale as f64) / scale_y_safe;
                ctx.scale(scale_x * self.zoom.scale, scale_y * self.zoom.scale);
                ctx.translate(-offset_x, -offset_y);
            } else if (scale_x - 1.0).abs() > f64::EPSILON || (scale_y - 1.0).abs() > f64::EPSILON {
                ctx.scale(scale_x, scale_y);
            }

            if let Err(err) = ctx.set_source_surface(&surface, 0.0, 0.0) {
                warn!("Failed to set frozen background surface: {}", err);
            } else if let Err(err) = ctx.paint() {
                warn!("Failed to paint frozen background: {}", err);
            }
            let _ = ctx.restore();

            let pattern = cairo::SurfacePattern::create(&surface);
            pattern.set_extend(cairo::Extend::Pad);
            let mut matrix = cairo::Matrix::identity();
            let scale_x_inv = 1.0 / (scale as f64 * scale_x.max(f64::MIN_POSITIVE));
            let scale_y_inv = 1.0 / (scale as f64 * scale_y.max(f64::MIN_POSITIVE));
            matrix.scale(scale_x_inv, scale_y_inv);
            pattern.set_matrix(matrix);
            eraser_surface = Some(surface);
            eraser_pattern = Some(pattern);
            backdrop_cache_key = Some(cache_key);
        } else {
            match self.input_state.boards.active_background() {
                crate::input::BoardBackground::Solid(color) => {
                    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
                    let _ = ctx.paint();
                    eraser_bg_color = Some(*color);
                }
                crate::input::BoardBackground::Transparent => {}
            }
        }

        Ok(CanvasEraserContext {
            surface: eraser_surface,
            pattern: eraser_pattern,
            backdrop_cache_key,
            bg_color: eraser_bg_color,
            logical_to_image_scale_x,
            logical_to_image_scale_y,
        })
    }
}
