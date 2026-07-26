use super::{BackdropStyle, BlurSurfaceStats, blur_overlay_palette, blur_recipe};

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
