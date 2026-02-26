//! SVG icon rendering via resvg.
//!
//! SVG icons are embedded at compile time and lazily rasterized.
//! Rendered surfaces are cached per pixel-size so only the first draw at a
//! given size incurs the resvg rasterization cost.  Icons are painted via
//! [`cairo::Context::mask_surface`] so they automatically inherit the callers
//! current source color.

use cairo::{Context, Format, ImageSurface};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

// Cached render entry
struct CachedRender {
    /// Pre-converted Cairo ARGB32 pixel data (native byte-order, premultiplied).
    data: Vec<u8>,
    width: i32,
    height: i32,
    stride: i32,
}

// Parsed + cached SVG icon
struct SvgIcon {
    tree: resvg::usvg::Tree,
    /// Per-pixel-size cache of rasterised data. The `Mutex` is uncontended in
    /// practice because the Wayland event loop is single-threaded.
    cache: Mutex<HashMap<u32, CachedRender>>,
}

impl SvgIcon {
    fn parse(svg_data: &str) -> Self {
        let tree =
            resvg::usvg::Tree::from_str(svg_data, &resvg::usvg::Options::default())
                .expect("embedded SVG must be valid");
        Self {
            tree,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Render this icon into ctx at (`x`, `y`) with the given square
    /// `size`. The icon is painted using the context's current source color
    fn render(&self, ctx: &Context, x: f64, y: f64, size: f64) {
        if size <= 0.0 {
            return;
        }
        let px = size.ceil() as u32;
        if let Some(surface) = self.surface_for(px) {
            let _ = ctx.mask_surface(&surface, x, y);
        }
    }

    /// Return a Cairo ['ImageSurface'] for the requested pixel size, creating
    /// and caching the rasterised data on first call
    fn surface_for(&self, px: u32) -> Option<ImageSurface> {
        let mut cache = self.cache.lock().ok()?;
        if !cache.contains_key(&px) {
            cache.insert(px, self.rasterize(px)?);
        }
        let c = cache.get(&px)?;
        ImageSurface::create_for_data(
            c.data.clone(),
            Format::ARgb32,
            c.width,
            c.height,
            c.stride,
        )
        .ok()
    }

    /// Rasterize the SVG tree at `px x px` and convert the pixel data from
    /// tiny-skia premultiplied RGBA to Cairo premultiplied ARGB32 (BGRA on
    /// little-endian).
    fn rasterize(&self, px: u32) -> Option<CachedRender> {
        let mut pixmap = tiny_skia::Pixmap::new(px, px)?;

        let sz = self.tree.size();
        let sx = px as f32 / sz.width();
        let sy = px as f32 / sz.height();
        resvg::render(
            &self.tree,
            tiny_skia::Transform::from_scale(sx, sy),
            &mut pixmap.as_mut(),
        );

        let stride = Format::ARgb32.stride_for_width(px).ok()? as usize;
        let w = px as usize;
        let h = px as usize;
        let src = pixmap.data();
        let mut data = vec![0u8; stride * h];

        for row in 0..h {
            for col in 0..w {
                let si = (row * w + col) * 4;
                let di = row * stride + col * 4;
                // tiny-skia RGBA to Cairo ARGB32 little-endian (BGRA in memory)
                data[di] = src[si + 2]; // B
                data[di + 1] = src[si + 1]; // G
                data[di + 2] = src[si]; // R
                data[di + 3] = src[si + 3]; // A
            }
        }

        Some(CachedRender {
            data,
            width: px as i32,
            height: px as i32,
            stride: stride as i32,
        })
    }
}

// Embed SVG files and create lazy-parsed statics
macro_rules! svg_icon {
    ($name:ident, $path:expr) => {
        static $name: LazyLock<SvgIcon> =
            LazyLock::new(|| SvgIcon::parse(include_str!($path)));
    };
}

svg_icon!(SELECT,       "../../assets/icons/mouse-pointer.svg");
svg_icon!(PEN,          "../../assets/icons/pen-tool.svg");
svg_icon!(LINE,         "../../assets/icons/minus.svg");
svg_icon!(RECT,         "../../assets/icons/rectangle-horizontal.svg");
svg_icon!(CIRCLE,       "../../assets/icons/circle.svg");
svg_icon!(ARROW,        "../../assets/icons/arrow-up-right.svg");
svg_icon!(ERASER,       "../../assets/icons/eraser.svg");
svg_icon!(TEXT,         "../../assets/icons/type.svg");
svg_icon!(NOTE,         "../../assets/icons/sticky-note.svg");
svg_icon!(HIGHLIGHT,    "../../assets/icons/mouse-pointer-click.svg");
svg_icon!(MARKER,       "../../assets/icons/highlighter.svg");
svg_icon!(STEP_MARKER,  "../../assets/icons/list-ordered.svg");

// Render helpers, matching the draw_icon_* signatures
pub fn render_select(ctx: &Context, x: f64, y: f64, size: f64) {
    SELECT.render(ctx, x, y, size);
}

pub fn render_pen(ctx: &Context, x: f64, y: f64, size: f64) {
    PEN.render(ctx, x, y, size);
}

pub fn render_line(ctx: &Context, x: f64, y: f64, size: f64) {
    LINE.render(ctx, x, y, size);
}

pub fn render_rect(ctx: &Context, x: f64, y: f64, size: f64) {
    RECT.render(ctx, x, y, size);
}

pub fn render_circle(ctx: &Context, x: f64, y: f64, size: f64) {
    CIRCLE.render(ctx, x, y, size);
}

pub fn render_arrow(ctx: &Context, x: f64, y: f64, size: f64) {
    ARROW.render(ctx, x, y, size);
}

pub fn render_eraser(ctx: &Context, x: f64, y: f64, size: f64) {
    ERASER.render(ctx, x, y, size);
}

pub fn render_text(ctx: &Context, x: f64, y: f64, size: f64) {
    TEXT.render(ctx, x, y, size);
}

pub fn render_note(ctx: &Context, x: f64, y: f64, size: f64) {
    NOTE.render(ctx, x, y, size);
}

pub fn render_highlight(ctx: &Context, x: f64, y: f64, size: f64) {
    HIGHLIGHT.render(ctx, x, y, size);
}

pub fn render_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    MARKER.render(ctx, x, y, size);
}

pub fn render_step_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    STEP_MARKER.render(ctx, x, y, size);
}
