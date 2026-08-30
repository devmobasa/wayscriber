use super::{
    BackdropStyle, BlurCacheKey, BlurRectParams, BlurRenderCache, BlurStyle, BlurSurfaceStats,
    CachedBlurRegion, EraserReplayContext, blur_overlay_palette, blur_recipe, render_blur_rect,
};

#[test]
fn extreme_gaussian_rect_with_backdrop_saturates_source_padding() {
    let backdrop = cairo::ImageSurface::create(cairo::Format::ARgb32, 8, 8).expect("backdrop");
    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, 8, 8).expect("output");
    let ctx = cairo::Context::new(&output).expect("output context");
    let replay = EraserReplayContext {
        pattern: None,
        surface: Some(&backdrop),
        backdrop_cache_key: Some(1),
        bg_color: None,
        logical_to_image_scale_x: 1.0,
        logical_to_image_scale_y: 1.0,
        logical_image_origin_x: f64::from(i32::MAX),
        logical_image_origin_y: f64::from(i32::MAX),
    };

    render_blur_rect(
        &ctx,
        BlurRectParams {
            x: i32::MAX,
            y: i32::MAX,
            w: i32::MAX,
            h: i32::MAX,
            strength: 12.0,
            style: BlurStyle::Gaussian,
            cacheable: false,
        },
        &replay,
    );

    ctx.status().expect("extreme Gaussian render remains valid");
}

#[test]
fn blur_recipe_keeps_default_strength_heavily_blurred_but_not_overwashed() {
    let recipe = blur_recipe(12.0, BackdropStyle::Gaussian);

    assert!(recipe.primary_factor >= 18.0);
    assert!(recipe.secondary_factor > recipe.primary_factor);
    assert!((0.05..=0.11).contains(&recipe.overlay_alpha));
}

#[test]
fn blur_recipe_clamps_extremes() {
    let min = blur_recipe(-10.0, BackdropStyle::Gaussian);
    let max = blur_recipe(500.0, BackdropStyle::Gaussian);

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
        style: BackdropStyle::Gaussian,
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
        style: BackdropStyle::Gaussian,
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

#[test]
fn blur_render_cache_does_not_retain_an_entry_larger_than_its_byte_budget() {
    let mut cache = BlurRenderCache::new(4, 63);
    let key = BlurCacheKey {
        backdrop_cache_key: 1,
        src_x: 0,
        src_y: 0,
        src_w: 4,
        src_h: 4,
        primary_factor: 18,
        secondary_factor: 24,
        style: BackdropStyle::Gaussian,
    };
    let entry = CachedBlurRegion {
        surface: cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).expect("surface"),
        stats: BlurSurfaceStats {
            red: 0.5,
            green: 0.52,
            blue: 0.55,
            luminance: 0.5,
        },
        approx_bytes: 64,
    };

    let returned = cache.insert(key, entry);

    assert!(cache.get(&key).is_none());
    assert!(cache.entries.is_empty());
    assert!(cache.access_order.is_empty());
    assert_eq!(cache.cached_bytes, 0);
    assert_eq!(returned.surface.width(), 4);
    assert_eq!(returned.surface.height(), 4);
}
