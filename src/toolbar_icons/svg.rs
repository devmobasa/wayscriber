//! SVG icon rendering via resvg.
//!
//! SVG icons are embedded at compile time and lazily rasterized.
//! Rendered surfaces are cached per pixel-size so only the first draw at a
//! given size incurs the resvg rasterization cost.  Icons are painted via
//! [`cairo::Context::mask_surface`] so they automatically inherit the callers
//! current source color.

use cairo::{Context, Format, ImageSurface};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::LazyLock;

const MAX_CACHED_SIZES: usize = 8;

thread_local! {
    static ICON_SURFACE_CACHE: RefCell<HashMap<usize, RenderCache>> = RefCell::new(HashMap::new());
}

// LRU cache for rasterized icon sizes.
struct RenderCache {
    entries: HashMap<u32, ImageSurface>,
    lru: VecDeque<u32>,
}

impl RenderCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            lru: VecDeque::new(),
        }
    }

    fn get(&mut self, px: u32) -> Option<ImageSurface> {
        let surface = self.entries.get(&px)?.clone();
        self.touch(px);
        Some(surface)
    }

    fn insert(&mut self, px: u32, surface: ImageSurface) {
        self.entries.insert(px, surface);
        self.touch(px);
        self.evict_oldest();
    }

    fn touch(&mut self, px: u32) {
        self.lru.retain(|cached_px| *cached_px != px);
        self.lru.push_back(px);
    }

    fn evict_oldest(&mut self) {
        while self.entries.len() > MAX_CACHED_SIZES {
            if let Some(oldest) = self.lru.pop_front() {
                self.entries.remove(&oldest);
            } else {
                break;
            }
        }
    }
}

// Parsed SVG icon
struct SvgIcon {
    tree: resvg::usvg::Tree,
}

impl SvgIcon {
    fn parse(svg_data: &str) -> Self {
        let tree = resvg::usvg::Tree::from_str(svg_data, &resvg::usvg::Options::default())
            .expect("embedded SVG must be valid");
        Self { tree }
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

    /// Return a Cairo [`ImageSurface`] for the requested pixel size, creating
    /// and caching the rasterised data on first call
    fn surface_for(&self, px: u32) -> Option<ImageSurface> {
        let key = self.cache_key();
        ICON_SURFACE_CACHE.with(|caches| {
            let mut caches = caches.borrow_mut();
            let cache = caches.entry(key).or_insert_with(RenderCache::new);
            if let Some(surface) = cache.get(px) {
                return Some(surface);
            }
            let surface = self.rasterize(px)?;
            cache.insert(px, surface.clone());
            Some(surface)
        })
    }

    /// Rasterize the SVG tree at `px x px` and convert the pixel data from
    /// tiny-skia premultiplied RGBA to Cairo premultiplied ARGB32 (BGRA on
    /// little-endian).
    fn rasterize(&self, px: u32) -> Option<ImageSurface> {
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

        ImageSurface::create_for_data(data, Format::ARgb32, px as i32, px as i32, stride as i32)
            .ok()
    }

    fn cache_key(&self) -> usize {
        self as *const Self as usize
    }

    #[cfg(test)]
    fn cache_len(&self) -> usize {
        let key = self.cache_key();
        ICON_SURFACE_CACHE.with(|caches| {
            let caches = caches.borrow();
            caches.get(&key).map_or(0, |cache| cache.entries.len())
        })
    }
}

// Embed SVG files and create lazy-parsed statics
macro_rules! svg_icon {
    ($name:ident, $path:expr) => {
        static $name: LazyLock<SvgIcon> = LazyLock::new(|| SvgIcon::parse(include_str!($path)));
    };
}

svg_icon!(SELECT, "../../assets/icons/mouse-pointer.svg");
svg_icon!(PEN, "../../assets/icons/pen-tool.svg");
svg_icon!(LINE, "../../assets/icons/minus.svg");
svg_icon!(RECT, "../../assets/icons/rectangle-horizontal.svg");
svg_icon!(CIRCLE, "../../assets/icons/circle.svg");
svg_icon!(ARROW, "../../assets/icons/arrow-up-right.svg");
svg_icon!(BLUR, "../../assets/icons/blur.svg");
svg_icon!(ERASER, "../../assets/icons/eraser.svg");
svg_icon!(TEXT, "../../assets/icons/type.svg");
svg_icon!(NOTE, "../../assets/icons/sticky-note.svg");
svg_icon!(HIGHLIGHT, "../../assets/icons/mouse-pointer-click.svg");
svg_icon!(MARKER, "../../assets/icons/highlighter.svg");
svg_icon!(STEP_MARKER, "../../assets/icons/list-ordered.svg");

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

pub fn render_blur(ctx: &Context, x: f64, y: f64, size: f64) {
    BLUR.render(ctx, x, y, size);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_icon_renders(name: &str, icon: &SvgIcon) {
        let surface = ImageSurface::create(Format::ARgb32, 24, 24).expect("surface");
        let ctx = Context::new(&surface).expect("context");
        ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        icon.render(&ctx, 0.0, 0.0, 24.0);
        surface.flush();

        let mut has_ink = false;
        surface
            .with_data(|pixels| {
                has_ink = pixels.chunks_exact(4).any(|pixel| pixel[3] != 0);
            })
            .expect("surface data");
        assert!(has_ink, "{name} icon rendered an empty surface");
    }

    #[test]
    fn embedded_icons_render_non_empty_alpha() {
        let icons: [(&str, &SvgIcon); 13] = [
            ("select", &*SELECT),
            ("pen", &*PEN),
            ("line", &*LINE),
            ("rect", &*RECT),
            ("circle", &*CIRCLE),
            ("arrow", &*ARROW),
            ("blur", &*BLUR),
            ("eraser", &*ERASER),
            ("text", &*TEXT),
            ("note", &*NOTE),
            ("highlight", &*HIGHLIGHT),
            ("marker", &*MARKER),
            ("step_marker", &*STEP_MARKER),
        ];

        for (name, icon) in icons {
            assert_icon_renders(name, icon);
        }
    }

    #[test]
    fn icon_surface_cache_stays_bounded() {
        let icon = &*SELECT;
        for px in 8..(8 + MAX_CACHED_SIZES as u32 + 5) {
            let _ = icon.surface_for(px).expect("surface for size");
        }

        assert!(
            icon.cache_len() <= MAX_CACHED_SIZES,
            "cache grew beyond limit: {} > {}",
            icon.cache_len(),
            MAX_CACHED_SIZES
        );
    }
}
