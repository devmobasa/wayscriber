use super::types::EraserReplayContext;
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
};

const PLACEHOLDER_FILL: (f64, f64, f64, f64) = (0.12, 0.15, 0.2, 0.82);
const PLACEHOLDER_STROKE: (f64, f64, f64, f64) = (0.92, 0.94, 0.98, 0.35);
const BLUR_CACHE_MAX_ENTRIES: usize = 8;
const BLUR_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
struct BlurRecipe {
    primary_factor: f64,
    secondary_factor: f64,
    padding_px: i32,
    overlay_alpha: f64,
}

#[derive(Clone, Copy, Debug)]
struct BlurSurfaceStats {
    red: f64,
    green: f64,
    blue: f64,
    luminance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BlurCacheKey {
    backdrop_cache_key: u64,
    src_x: i32,
    src_y: i32,
    src_w: i32,
    src_h: i32,
    primary_factor: u16,
    secondary_factor: u16,
}

#[derive(Clone)]
struct CachedBlurRegion {
    surface: cairo::ImageSurface,
    stats: BlurSurfaceStats,
    approx_bytes: usize,
}

struct BlurRenderCache {
    entries: HashMap<BlurCacheKey, CachedBlurRegion>,
    access_order: VecDeque<BlurCacheKey>,
    max_entries: usize,
    max_bytes: usize,
    cached_bytes: usize,
}

impl BlurRenderCache {
    fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            access_order: VecDeque::new(),
            max_entries,
            max_bytes,
            cached_bytes: 0,
        }
    }

    fn get(&mut self, key: &BlurCacheKey) -> Option<CachedBlurRegion> {
        let entry = self.entries.get(key).cloned()?;
        self.touch(*key);
        Some(entry)
    }

    fn insert(&mut self, key: BlurCacheKey, entry: CachedBlurRegion) {
        if let Some(previous) = self.entries.remove(&key) {
            self.cached_bytes = self.cached_bytes.saturating_sub(previous.approx_bytes);
            self.access_order.retain(|existing| existing != &key);
        }

        self.cached_bytes = self.cached_bytes.saturating_add(entry.approx_bytes);
        self.entries.insert(key, entry);
        self.access_order.push_back(key);
        self.evict_if_needed();
    }

    fn touch(&mut self, key: BlurCacheKey) {
        self.access_order.retain(|existing| existing != &key);
        self.access_order.push_back(key);
    }

    fn evict_if_needed(&mut self) {
        while self.entries.len() > self.max_entries
            || (self.cached_bytes > self.max_bytes && self.entries.len() > 1)
        {
            let Some(oldest) = self.access_order.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.cached_bytes = self.cached_bytes.saturating_sub(entry.approx_bytes);
            }
        }
    }
}

thread_local! {
    static BLUR_RENDER_CACHE: RefCell<BlurRenderCache> = RefCell::new(
        BlurRenderCache::new(BLUR_CACHE_MAX_ENTRIES, BLUR_CACHE_MAX_BYTES)
    );
}

fn normalize_rect(x: i32, y: i32, w: i32, h: i32) -> Option<(f64, f64, f64, f64)> {
    let left = x.min(x + w) as f64;
    let top = y.min(y + h) as f64;
    let width = w.abs().max(1) as f64;
    let height = h.abs().max(1) as f64;
    (width > 0.0 && height > 0.0).then_some((left, top, width, height))
}

fn blur_recipe(strength: f64) -> BlurRecipe {
    let clamped = strength.clamp(1.0, 50.0);
    let normalized = ((clamped - 1.0) / 49.0).clamp(0.0, 1.0);

    // Bias the low end upward so the default thickness already produces
    // privacy-grade blur rather than a subtle aesthetic softening.
    let shaped = normalized.powf(0.6);
    let primary_factor = (8.0 + shaped * 28.0).round().clamp(8.0, 36.0);
    let secondary_factor = (primary_factor * (1.28 + normalized * 0.32))
        .round()
        .clamp(primary_factor + 2.0, 52.0);

    BlurRecipe {
        primary_factor,
        secondary_factor,
        padding_px: (secondary_factor.ceil() as i32).saturating_mul(2).max(12),
        overlay_alpha: 0.05 + shaped * 0.06,
    }
}

pub(super) fn render_blur_placeholder(
    ctx: &cairo::Context,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    selected: bool,
) {
    let Some((left, top, width, height)) = normalize_rect(x, y, w, h) else {
        return;
    };

    let _ = ctx.save();
    ctx.rectangle(left, top, width, height);
    let alpha = if selected { 0.92 } else { PLACEHOLDER_FILL.3 };
    ctx.set_source_rgba(
        PLACEHOLDER_FILL.0,
        PLACEHOLDER_FILL.1,
        PLACEHOLDER_FILL.2,
        alpha,
    );
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(
        PLACEHOLDER_STROKE.0,
        PLACEHOLDER_STROKE.1,
        PLACEHOLDER_STROKE.2,
        PLACEHOLDER_STROKE.3,
    );
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    let band_count = 4;
    let band_h = (height / band_count as f64).max(1.0);
    for idx in 0..band_count {
        if idx % 2 == 0 {
            continue;
        }
        ctx.rectangle(left, top + idx as f64 * band_h, width, band_h.min(height));
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.06);
        let _ = ctx.fill();
    }
    let _ = ctx.restore();
}

fn copy_surface_region(
    surface: &cairo::ImageSurface,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Option<cairo::ImageSurface> {
    let region = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).ok()?;
    let ctx = cairo::Context::new(&region).ok()?;
    let _ = ctx.set_source_surface(surface, -(x as f64), -(y as f64));
    let _ = ctx.paint();
    Some(region)
}

fn resample_dimensions(width: i32, height: i32, factor: f64) -> (i32, i32) {
    (
        ((width as f64) / factor).round().max(1.0) as i32,
        ((height as f64) / factor).round().max(1.0) as i32,
    )
}

fn resample_surface(
    source: &cairo::ImageSurface,
    width: i32,
    height: i32,
    filter: cairo::Filter,
) -> Option<cairo::ImageSurface> {
    let resampled = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).ok()?;
    let ctx = cairo::Context::new(&resampled).ok()?;
    let scale_x = width as f64 / source.width() as f64;
    let scale_y = height as f64 / source.height() as f64;
    ctx.scale(
        scale_x.max(f64::MIN_POSITIVE),
        scale_y.max(f64::MIN_POSITIVE),
    );
    let pattern = cairo::SurfacePattern::create(source);
    pattern.set_filter(filter);
    let _ = ctx.set_source(&pattern);
    let _ = ctx.paint();
    Some(resampled)
}

fn average_surface_stats(surface: &mut cairo::ImageSurface) -> Option<BlurSurfaceStats> {
    let width = surface.width().max(1) as usize;
    let height = surface.height().max(1) as usize;
    let stride = surface.stride().max(4) as usize;
    let step = (width.max(height) / 64).max(1);

    surface.flush();
    let data = surface.data().ok()?;
    let mut total_red = 0.0;
    let mut total_green = 0.0;
    let mut total_blue = 0.0;
    let mut total = 0.0;
    let mut count = 0usize;

    for y in (0..height).step_by(step) {
        let row = &data[y * stride..];
        for x in (0..width).step_by(step) {
            let idx = x * 4;
            if idx + 3 >= row.len() {
                break;
            }

            let alpha = row[idx + 3] as f64 / 255.0;
            if alpha <= f64::EPSILON {
                continue;
            }

            let blue = row[idx] as f64 / 255.0;
            let green = row[idx + 1] as f64 / 255.0;
            let red = row[idx + 2] as f64 / 255.0;
            let inv_alpha = alpha.recip();
            let red = (red * inv_alpha).clamp(0.0, 1.0);
            let green = (green * inv_alpha).clamp(0.0, 1.0);
            let blue = (blue * inv_alpha).clamp(0.0, 1.0);

            total_red += red;
            total_green += green;
            total_blue += blue;
            total += red * 0.299 + green * 0.587 + blue * 0.114;
            count += 1;
        }
    }

    (count > 0).then_some(BlurSurfaceStats {
        red: total_red / count as f64,
        green: total_green / count as f64,
        blue: total_blue / count as f64,
        luminance: total / count as f64,
    })
}

fn blur_overlay_palette(
    stats: BlurSurfaceStats,
    alpha: f64,
) -> ((f64, f64, f64, f64), (f64, f64, f64, f64)) {
    let fill = (stats.red, stats.green, stats.blue, alpha);

    if stats.luminance > 0.62 {
        (
            fill,
            (
                (stats.red * 0.58).clamp(0.0, 1.0),
                (stats.green * 0.58).clamp(0.0, 1.0),
                (stats.blue * 0.58).clamp(0.0, 1.0),
                (alpha + 0.08).min(0.22),
            ),
        )
    } else {
        (
            fill,
            (
                (stats.red + (1.0 - stats.red) * 0.32).clamp(0.0, 1.0),
                (stats.green + (1.0 - stats.green) * 0.32).clamp(0.0, 1.0),
                (stats.blue + (1.0 - stats.blue) * 0.32).clamp(0.0, 1.0),
                (alpha + 0.07).min(0.2),
            ),
        )
    }
}

fn build_blur_cache_key(
    replay_ctx: &EraserReplayContext<'_>,
    recipe: BlurRecipe,
    src_x: i32,
    src_y: i32,
    src_w: i32,
    src_h: i32,
) -> Option<BlurCacheKey> {
    Some(BlurCacheKey {
        backdrop_cache_key: replay_ctx.backdrop_cache_key?,
        src_x,
        src_y,
        src_w,
        src_h,
        primary_factor: recipe.primary_factor.round() as u16,
        secondary_factor: recipe.secondary_factor.round() as u16,
    })
}

fn cacheable_blur_entry(
    cache_key: Option<BlurCacheKey>,
    compute: impl FnOnce() -> Option<CachedBlurRegion>,
) -> Option<CachedBlurRegion> {
    if let Some(key) = cache_key
        && let Some(entry) = BLUR_RENDER_CACHE.with(|cache| cache.borrow_mut().get(&key))
    {
        return Some(entry);
    }

    let entry = compute()?;
    if let Some(key) = cache_key {
        BLUR_RENDER_CACHE.with(|cache| cache.borrow_mut().insert(key, entry.clone()));
    }
    Some(entry)
}

fn render_blur_region(
    surface: &cairo::ImageSurface,
    src_x: i32,
    src_y: i32,
    src_w: i32,
    src_h: i32,
    recipe: BlurRecipe,
) -> Option<CachedBlurRegion> {
    let crop = copy_surface_region(surface, src_x, src_y, src_w, src_h)?;
    let (small_w, small_h) = resample_dimensions(src_w, src_h, recipe.primary_factor);
    let downscaled = resample_surface(&crop, small_w, small_h, cairo::Filter::Best)?;
    let (tiny_w, tiny_h) = resample_dimensions(src_w, src_h, recipe.secondary_factor);
    let tiny = resample_surface(&downscaled, tiny_w, tiny_h, cairo::Filter::Best)?;
    let mut blurred = resample_surface(&tiny, src_w, src_h, cairo::Filter::Bilinear)?;
    let stats = average_surface_stats(&mut blurred).unwrap_or(BlurSurfaceStats {
        red: 0.6,
        green: 0.62,
        blue: 0.66,
        luminance: 0.62,
    });

    Some(CachedBlurRegion {
        approx_bytes: (src_w.max(1) as usize)
            .saturating_mul(src_h.max(1) as usize)
            .saturating_mul(4),
        surface: blurred,
        stats,
    })
}

pub fn render_blur_rect(
    ctx: &cairo::Context,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    strength: f64,
    replay_ctx: &EraserReplayContext<'_>,
    cacheable: bool,
) {
    let Some((left, top, width, height)) = normalize_rect(x, y, w, h) else {
        return;
    };

    let Some(surface) = replay_ctx.surface else {
        render_blur_placeholder(ctx, x, y, w, h, false);
        return;
    };

    let scale_x = replay_ctx.logical_to_image_scale_x.max(f64::MIN_POSITIVE);
    let scale_y = replay_ctx.logical_to_image_scale_y.max(f64::MIN_POSITIVE);
    let recipe = blur_recipe(strength);

    let src_x = ((left * scale_x).floor() as i32).saturating_sub(recipe.padding_px);
    let src_y = ((top * scale_y).floor() as i32).saturating_sub(recipe.padding_px);
    let src_x2 = ((left + width) * scale_x).ceil() as i32 + recipe.padding_px;
    let src_y2 = ((top + height) * scale_y).ceil() as i32 + recipe.padding_px;

    let src_x = src_x.clamp(0, surface.width().saturating_sub(1));
    let src_y = src_y.clamp(0, surface.height().saturating_sub(1));
    let src_x2 = src_x2.clamp(src_x + 1, surface.width());
    let src_y2 = src_y2.clamp(src_y + 1, surface.height());
    let src_w = src_x2 - src_x;
    let src_h = src_y2 - src_y;
    let cache_key =
        cacheable.then(|| build_blur_cache_key(replay_ctx, recipe, src_x, src_y, src_w, src_h));
    let cache_key = cache_key.flatten();
    let Some(blurred) = cacheable_blur_entry(cache_key, || {
        render_blur_region(surface, src_x, src_y, src_w, src_h, recipe)
    }) else {
        render_blur_placeholder(ctx, x, y, w, h, false);
        return;
    };
    let overlay_palette = blur_overlay_palette(blurred.stats, recipe.overlay_alpha);

    let dest_x = src_x as f64 / scale_x;
    let dest_y = src_y as f64 / scale_y;
    let dest_w = src_w as f64 / scale_x;
    let dest_h = src_h as f64 / scale_y;

    let _ = ctx.save();
    ctx.rectangle(left, top, width, height);
    ctx.clip();
    ctx.translate(dest_x, dest_y);
    ctx.scale(dest_w / src_w.max(1) as f64, dest_h / src_h.max(1) as f64);
    let pattern = cairo::SurfacePattern::create(&blurred.surface);
    pattern.set_filter(cairo::Filter::Bilinear);
    let _ = ctx.set_source(&pattern);
    let _ = ctx.paint();
    let _ = ctx.restore();

    let _ = ctx.save();
    ctx.rectangle(left, top, width, height);
    ctx.set_source_rgba(
        overlay_palette.0.0,
        overlay_palette.0.1,
        overlay_palette.0.2,
        overlay_palette.0.3,
    );
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(
        overlay_palette.1.0,
        overlay_palette.1.1,
        overlay_palette.1.2,
        overlay_palette.1.3,
    );
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
    let _ = ctx.restore();
}

#[cfg(test)]
mod tests {
    use super::{
        BlurCacheKey, BlurRenderCache, BlurSurfaceStats, CachedBlurRegion, blur_overlay_palette,
        blur_recipe,
    };

    #[test]
    fn blur_recipe_keeps_default_strength_heavily_blurred_but_not_overwashed() {
        let recipe = blur_recipe(12.0);

        assert!(recipe.primary_factor >= 18.0);
        assert!(recipe.secondary_factor > recipe.primary_factor);
        assert!((0.05..=0.11).contains(&recipe.overlay_alpha));
    }

    #[test]
    fn blur_recipe_clamps_extremes() {
        let min = blur_recipe(-10.0);
        let max = blur_recipe(500.0);

        assert_eq!(min.primary_factor, 8.0);
        assert_eq!(min.secondary_factor, 10.0);
        assert!((0.05..=0.11).contains(&min.overlay_alpha));

        assert!(max.primary_factor <= 36.0);
        assert!(max.secondary_factor <= 52.0);
        assert!((0.05..=0.11).contains(&max.overlay_alpha));
    }

    #[test]
    fn overlay_palette_switches_contrast_for_light_and_dark_regions() {
        let dark_region = blur_overlay_palette(
            BlurSurfaceStats {
                red: 0.2,
                green: 0.24,
                blue: 0.3,
                luminance: 0.22,
            },
            0.1,
        );
        let light_region = blur_overlay_palette(
            BlurSurfaceStats {
                red: 0.82,
                green: 0.84,
                blue: 0.88,
                luminance: 0.84,
            },
            0.1,
        );

        assert!((dark_region.0.0 - 0.2).abs() < f64::EPSILON);
        assert!(dark_region.1.0 > dark_region.0.0);
        assert!((light_region.0.0 - 0.82).abs() < f64::EPSILON);
        assert!(light_region.1.0 < light_region.0.0);
    }

    #[test]
    fn blur_render_cache_returns_cached_entry_for_same_key() {
        let mut cache = BlurRenderCache::new(4, 1024);
        let key = BlurCacheKey {
            backdrop_cache_key: 1,
            src_x: 10,
            src_y: 20,
            src_w: 30,
            src_h: 40,
            primary_factor: 18,
            secondary_factor: 24,
        };
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).expect("surface");
        cache.insert(
            key,
            CachedBlurRegion {
                surface,
                stats: BlurSurfaceStats {
                    red: 0.4,
                    green: 0.42,
                    blue: 0.45,
                    luminance: 0.42,
                },
                approx_bytes: 64,
            },
        );

        let cached = cache.get(&key).expect("cached entry");
        assert!((cached.stats.luminance - 0.42).abs() < f64::EPSILON);
        assert_eq!(cached.surface.width(), 4);
        assert_eq!(cached.surface.height(), 4);
    }

    #[test]
    fn blur_render_cache_evicts_oldest_entry_when_budget_is_exceeded() {
        let mut cache = BlurRenderCache::new(2, 96);
        let make_key = |backdrop_cache_key| BlurCacheKey {
            backdrop_cache_key,
            src_x: 0,
            src_y: 0,
            src_w: 2,
            src_h: 2,
            primary_factor: 18,
            secondary_factor: 24,
        };
        let make_entry = || CachedBlurRegion {
            surface: cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).expect("surface"),
            stats: BlurSurfaceStats {
                red: 0.5,
                green: 0.52,
                blue: 0.55,
                luminance: 0.5,
            },
            approx_bytes: 64,
        };

        cache.insert(make_key(1), make_entry());
        cache.insert(make_key(2), make_entry());

        assert!(cache.get(&make_key(1)).is_none());
        assert!(cache.get(&make_key(2)).is_some());
    }
}
