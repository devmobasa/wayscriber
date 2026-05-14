use crate::draw::shape::EmbeddedImage;
use crate::image_decode::{decode_rgba, format_from_mime_or_bytes};
use cairo::{Format, ImageSurface};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

const IMAGE_CACHE_ENTRIES: usize = 32;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct ImageCacheKey {
    mime_type: String,
    len: usize,
    hash: u64,
    width: u32,
    height: u32,
}

thread_local! {
    static IMAGE_CACHE: RefCell<ImageSurfaceCache> = RefCell::new(ImageSurfaceCache::new());
}

struct ImageSurfaceCache {
    entries: HashMap<ImageCacheKey, Rc<ImageSurface>>,
    order: VecDeque<ImageCacheKey>,
}

impl ImageSurfaceCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&mut self, key: &ImageCacheKey) -> Option<Rc<ImageSurface>> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: ImageCacheKey, surface: Rc<ImageSurface>) -> Rc<ImageSurface> {
        if self.entries.contains_key(&key) {
            self.entries.insert(key.clone(), surface.clone());
            return surface;
        }

        self.order.push_back(key.clone());
        self.entries.insert(key, surface.clone());
        while self.order.len() > IMAGE_CACHE_ENTRIES {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        surface
    }
}

pub fn render_image_shape(
    ctx: &cairo::Context,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    data: &EmbeddedImage,
) {
    if w == 0 || h == 0 {
        return;
    }
    let Some(surface) = cached_surface(data) else {
        render_missing_image_placeholder(ctx, x, y, w, h);
        return;
    };

    let width = w.saturating_abs().max(1) as f64;
    let height = h.saturating_abs().max(1) as f64;
    let draw_x = if w < 0 { x + w } else { x };
    let draw_y = if h < 0 { y + h } else { y };

    let _ = ctx.save();
    ctx.rectangle(draw_x as f64, draw_y as f64, width, height);
    ctx.clip();
    ctx.translate(draw_x as f64, draw_y as f64);
    ctx.scale(
        width / surface.width().max(1) as f64,
        height / surface.height().max(1) as f64,
    );
    let _ = ctx.set_source_surface(surface.as_ref(), 0.0, 0.0);
    let _ = ctx.paint();
    let _ = ctx.restore();
}

fn cached_surface(data: &EmbeddedImage) -> Option<Rc<ImageSurface>> {
    let key = ImageCacheKey {
        mime_type: data.mime_type.clone(),
        len: data.bytes.len(),
        hash: content_hash(&data.bytes),
        width: data.width,
        height: data.height,
    };

    IMAGE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(surface) = cache.get(&key) {
            return Some(surface);
        }

        let surface = decode_surface(data).map(Rc::new)?;
        Some(cache.insert(key, surface))
    })
}

fn decode_surface(data: &EmbeddedImage) -> Option<ImageSurface> {
    let format = format_from_mime_or_bytes(&data.mime_type, &data.bytes)?;
    let image = decode_rgba(format, &data.bytes).ok()?;
    let width = image.width;
    let height = image.height;
    if width == 0 || height == 0 {
        return None;
    }

    let stride = Format::ARgb32.stride_for_width(width).ok()? as usize;
    let mut pixels = vec![0u8; stride * height as usize];
    for (row, source) in image.rgba.chunks_exact(width as usize * 4).enumerate() {
        let offset = row * stride;
        let row_bytes = &mut pixels[offset..offset + width as usize * 4];
        for (pixel, out) in source.chunks_exact(4).zip(row_bytes.chunks_exact_mut(4)) {
            let [r, g, b, a] = [pixel[0], pixel[1], pixel[2], pixel[3]];
            let premul =
                |channel: u8| -> u8 { ((channel as u16 * a as u16 + 127) / 255).min(255) as u8 };
            let r = premul(r);
            let g = premul(g);
            let b = premul(b);
            if cfg!(target_endian = "little") {
                out.copy_from_slice(&[b, g, r, a]);
            } else {
                out.copy_from_slice(&[a, r, g, b]);
            }
        }
    }

    ImageSurface::create_for_data(
        pixels,
        Format::ARgb32,
        width as i32,
        height as i32,
        stride as i32,
    )
    .ok()
}

fn content_hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn render_missing_image_placeholder(ctx: &cairo::Context, x: i32, y: i32, w: i32, h: i32) {
    let width = w.saturating_abs().max(1) as f64;
    let height = h.saturating_abs().max(1) as f64;
    let draw_x = if w < 0 { x + w } else { x } as f64;
    let draw_y = if h < 0 { y + h } else { y } as f64;

    let _ = ctx.save();
    ctx.rectangle(draw_x, draw_y, width, height);
    ctx.set_source_rgba(0.12, 0.12, 0.12, 0.24);
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(0.9, 0.9, 0.9, 0.8);
    ctx.set_line_width(2.0);
    let _ = ctx.stroke();
    ctx.move_to(draw_x, draw_y);
    ctx.line_to(draw_x + width, draw_y + height);
    ctx.move_to(draw_x + width, draw_y);
    ctx.line_to(draw_x, draw_y + height);
    let _ = ctx.stroke();
    let _ = ctx.restore();
}
