use crate::draw::shape::EmbeddedImage;
use crate::image_decode::{decode_rgba, format_from_mime_or_bytes};
use cairo::{Format, ImageSurface};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;

const IMAGE_CACHE_ENTRIES: usize = 32;
/// Per-render-thread budget for decoded ARGB32 image pixels.
const IMAGE_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug)]
struct ImageBytesIdentity(Arc<[u8]>);

impl PartialEq for ImageBytesIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ImageBytesIdentity {}

impl Hash for ImageBytesIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
        self.0.len().hash(state);
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct ImageCacheKey {
    mime_type: String,
    bytes: ImageBytesIdentity,
    width: u32,
    height: u32,
}

thread_local! {
    static IMAGE_CACHE: RefCell<ImageSurfaceCache> = RefCell::new(ImageSurfaceCache::new(
        IMAGE_CACHE_ENTRIES,
        IMAGE_CACHE_MAX_BYTES,
    ));
}

struct CachedImageSurface {
    surface: Rc<ImageSurface>,
    decoded_bytes: usize,
}

struct ImageSurfaceCache {
    entries: HashMap<ImageCacheKey, CachedImageSurface>,
    access_order: VecDeque<ImageCacheKey>,
    max_entries: usize,
    max_bytes: usize,
    cached_bytes: usize,
}

impl ImageSurfaceCache {
    fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            access_order: VecDeque::new(),
            max_entries,
            max_bytes,
            cached_bytes: 0,
        }
    }

    fn get(&mut self, key: &ImageCacheKey) -> Option<Rc<ImageSurface>> {
        let surface = Rc::clone(&self.entries.get(key)?.surface);
        self.touch(key);
        Some(surface)
    }

    fn insert(
        &mut self,
        key: ImageCacheKey,
        surface: Rc<ImageSurface>,
        decoded_bytes: usize,
    ) -> Rc<ImageSurface> {
        self.remove(&key);
        if self.max_entries == 0 || decoded_bytes > self.max_bytes {
            return surface;
        }

        self.cached_bytes += decoded_bytes;
        self.access_order.push_back(key.clone());
        self.entries.insert(
            key,
            CachedImageSurface {
                surface: Rc::clone(&surface),
                decoded_bytes,
            },
        );
        self.evict_if_needed();
        surface
    }

    fn touch(&mut self, key: &ImageCacheKey) {
        if let Some(index) = self
            .access_order
            .iter()
            .position(|candidate| candidate == key)
        {
            self.access_order.remove(index);
            self.access_order.push_back(key.clone());
        }
    }

    fn remove(&mut self, key: &ImageCacheKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.cached_bytes -= entry.decoded_bytes;
        }
        if let Some(index) = self
            .access_order
            .iter()
            .position(|candidate| candidate == key)
        {
            self.access_order.remove(index);
        }
    }

    fn evict_if_needed(&mut self) {
        while self.entries.len() > self.max_entries || self.cached_bytes > self.max_bytes {
            let Some(oldest) = self.access_order.pop_front() else {
                debug_assert!(self.entries.is_empty());
                self.cached_bytes = 0;
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.cached_bytes -= entry.decoded_bytes;
            }
        }
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
        bytes: ImageBytesIdentity(Arc::clone(&data.bytes)),
        width: data.width,
        height: data.height,
    };

    IMAGE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(surface) = cache.get(&key) {
            return Some(surface);
        }

        let (surface, decoded_bytes) = decode_surface(data)?;
        Some(cache.insert(key, Rc::new(surface), decoded_bytes))
    })
}

fn decode_surface(data: &EmbeddedImage) -> Option<(ImageSurface, usize)> {
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

    let decoded_bytes = pixels.len();
    ImageSurface::create_for_data(
        pixels,
        Format::ARgb32,
        width as i32,
        height as i32,
        stride as i32,
    )
    .ok()
    .map(|surface| (surface, decoded_bytes))
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

#[cfg(test)]
mod tests {
    use super::{ImageBytesIdentity, ImageCacheKey, ImageSurfaceCache};
    use cairo::{Format, ImageSurface};
    use std::rc::Rc;
    use std::sync::Arc;

    fn cache_key(marker: u8) -> (ImageCacheKey, std::sync::Weak<[u8]>) {
        let bytes: Arc<[u8]> = vec![marker].into();
        let weak = Arc::downgrade(&bytes);
        (
            ImageCacheKey {
                mime_type: "image/png".to_string(),
                bytes: ImageBytesIdentity(bytes),
                width: 1,
                height: 1,
            },
            weak,
        )
    }

    fn surface() -> Rc<ImageSurface> {
        Rc::new(ImageSurface::create(Format::ARgb32, 1, 1).expect("test surface"))
    }

    #[test]
    fn cache_identity_follows_shared_payload_allocation() {
        let bytes: Arc<[u8]> = vec![1, 2, 3].into();
        let shared = ImageBytesIdentity(Arc::clone(&bytes));
        let same_allocation = ImageBytesIdentity(Arc::clone(&bytes));
        let equal_bytes_in_another_allocation = ImageBytesIdentity(vec![1, 2, 3].into());

        assert_eq!(shared, same_allocation);
        assert_ne!(shared, equal_bytes_in_another_allocation);
    }

    #[test]
    fn cache_hit_becomes_most_recent_before_byte_budget_eviction() {
        let mut cache = ImageSurfaceCache::new(3, 8);
        let (a, _) = cache_key(1);
        let (b, _) = cache_key(2);
        let (c, _) = cache_key(3);

        drop(cache.insert(a.clone(), surface(), 4));
        drop(cache.insert(b.clone(), surface(), 4));
        assert!(cache.get(&a).is_some(), "reading A should promote it");

        drop(cache.insert(c.clone(), surface(), 4));

        assert!(cache.entries.contains_key(&a));
        assert!(!cache.entries.contains_key(&b));
        assert!(cache.entries.contains_key(&c));
        assert_eq!(cache.cached_bytes, 8);
    }

    #[test]
    fn oversized_surface_is_returned_but_not_retained() {
        let mut cache = ImageSurfaceCache::new(4, 3);
        let (key, payload) = cache_key(1);
        let surface = surface();
        let weak_surface = Rc::downgrade(&surface);

        let returned = cache.insert(key, surface, 4);

        assert!(cache.entries.is_empty());
        assert!(cache.access_order.is_empty());
        assert_eq!(cache.cached_bytes, 0);
        assert!(
            payload.upgrade().is_none(),
            "oversized payload key should not be retained"
        );
        assert!(weak_surface.upgrade().is_some());

        drop(returned);
        assert!(weak_surface.upgrade().is_none());
    }

    #[test]
    fn eviction_releases_surface_and_payload() {
        let mut cache = ImageSurfaceCache::new(1, 8);
        let (first_key, first_payload) = cache_key(1);
        let first_surface = surface();
        let weak_surface = Rc::downgrade(&first_surface);

        drop(cache.insert(first_key, first_surface, 4));
        assert!(first_payload.upgrade().is_some());
        assert!(weak_surface.upgrade().is_some());

        let (second_key, _) = cache_key(2);
        drop(cache.insert(second_key, surface(), 4));

        assert!(first_payload.upgrade().is_none());
        assert!(weak_surface.upgrade().is_none());
    }
}
