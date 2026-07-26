//! Blur style selection, recipes, and the rendered result per style.

use super::{BackdropStyle, BlurRectParams, blur_recipe, render_black_out_rect, render_blur_rect};
use crate::draw::render::types::EraserReplayContext;
use crate::draw::shape::BlurStyle;
use cairo::{Context, ImageSurface};

fn surface_with_context(width: i32, height: i32) -> (ImageSurface, Context) {
    let surface = ImageSurface::create(cairo::Format::ARgb32, width, height)
        .expect("test surface allocation");
    let ctx = Context::new(&surface).expect("test cairo context");
    (surface, ctx)
}

/// `EraserReplayContext` has no `Default`, so tests spell out the empty case.
fn no_backdrop<'a>() -> EraserReplayContext<'a> {
    EraserReplayContext {
        pattern: None,
        surface: None,
        backdrop_cache_key: None,
        bg_color: None,
        logical_to_image_scale_x: 1.0,
        logical_to_image_scale_y: 1.0,
    }
}

fn with_backdrop<'a>(surface: &'a ImageSurface) -> EraserReplayContext<'a> {
    EraserReplayContext {
        surface: Some(surface),
        ..no_backdrop()
    }
}

fn pixel(surface: &mut ImageSurface, x: i32, y: i32) -> (u8, u8, u8, u8) {
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().expect("surface data");
    let idx = y as usize * stride + x as usize * 4;
    (data[idx + 2], data[idx + 1], data[idx], data[idx + 3])
}

/// A backdrop of hard vertical stripes: easy to tell apart blurred, blocked,
/// flattened, and blacked-out results.
fn striped_backdrop(width: i32, height: i32) -> ImageSurface {
    let surface =
        ImageSurface::create(cairo::Format::ARgb32, width, height).expect("backdrop allocation");
    let ctx = Context::new(&surface).expect("backdrop context");
    ctx.set_source_rgb(0.0, 0.0, 0.0);
    let _ = ctx.paint();
    ctx.set_source_rgb(1.0, 1.0, 1.0);
    for x in (0..width).step_by(4) {
        ctx.rectangle(f64::from(x), 0.0, 2.0, f64::from(height));
    }
    let _ = ctx.fill();
    surface
}

#[test]
fn backdrop_style_excludes_black_out() {
    assert_eq!(
        BackdropStyle::from_style(BlurStyle::Gaussian),
        Some(BackdropStyle::Gaussian)
    );
    assert_eq!(
        BackdropStyle::from_style(BlurStyle::Pixelate),
        Some(BackdropStyle::Pixelate)
    );
    assert_eq!(
        BackdropStyle::from_style(BlurStyle::Secure),
        Some(BackdropStyle::Secure)
    );
    assert_eq!(
        BackdropStyle::from_style(BlurStyle::BlackOut),
        None,
        "black out must not reach the backdrop sampling path"
    );
}

#[test]
fn every_style_reports_whether_it_needs_a_backdrop() {
    for style in BlurStyle::ALL {
        assert_eq!(
            style.needs_backdrop(),
            BackdropStyle::from_style(style).is_some(),
            "{style:?} disagrees about needing a backdrop"
        );
    }
}

#[test]
fn blur_styles_cycle_through_every_variant_and_wrap() {
    let mut seen = Vec::new();
    let mut style = BlurStyle::Gaussian;
    for _ in 0..BlurStyle::ALL.len() {
        seen.push(style);
        style = style.next();
    }
    assert_eq!(seen, BlurStyle::ALL.to_vec());
    assert_eq!(style, BlurStyle::Gaussian, "cycling should wrap around");
}

#[test]
fn pixelate_uses_zero_padding_so_blocks_align_with_the_rect() {
    let recipe = blur_recipe(12.0, BackdropStyle::Pixelate);
    assert_eq!(recipe.padding_px, 0);
    assert!(recipe.overlay_alpha.abs() < f64::EPSILON, "no tint wash");
    assert!(recipe.primary_factor >= 4.0);
}

#[test]
fn pixelate_block_size_grows_with_strength() {
    let small = blur_recipe(1.0, BackdropStyle::Pixelate);
    let large = blur_recipe(50.0, BackdropStyle::Pixelate);
    assert!(
        large.primary_factor > small.primary_factor,
        "block edge should follow the size slider: {} vs {}",
        small.primary_factor,
        large.primary_factor
    );
}

#[test]
fn secure_style_collapses_to_a_single_sample() {
    let recipe = blur_recipe(30.0, BackdropStyle::Secure);
    assert_eq!(recipe.primary_factor, 1.0);
    assert_eq!(recipe.padding_px, 0);
}

#[test]
fn black_out_paints_opaque_black() {
    let (mut surface, ctx) = surface_with_context(40, 40);
    render_black_out_rect(&ctx, 5, 5, 30, 30);
    drop(ctx);

    let (r, g, b, a) = pixel(&mut surface, 20, 20);
    assert_eq!((r, g, b), (0, 0, 0), "interior should be pure black");
    assert_eq!(a, 255, "black out must be fully opaque");
}

#[test]
fn black_out_renders_without_any_captured_backdrop() {
    let (mut surface, ctx) = surface_with_context(40, 40);
    let replay_ctx = no_backdrop();

    render_blur_rect(
        &ctx,
        BlurRectParams {
            x: 5,
            y: 5,
            w: 30,
            h: 30,
            strength: 12.0,
            style: BlurStyle::BlackOut,
            cacheable: false,
        },
        &replay_ctx,
    );
    drop(ctx);

    let (r, g, b, a) = pixel(&mut surface, 20, 20);
    assert_eq!(
        ((r, g, b), a),
        ((0, 0, 0), 255),
        "black out should not fall back to the placeholder"
    );
}

#[test]
fn secure_style_leaves_no_variation_across_the_region() {
    let backdrop = striped_backdrop(64, 64);
    let (mut surface, ctx) = surface_with_context(64, 64);
    let replay_ctx = with_backdrop(&backdrop);

    render_blur_rect(
        &ctx,
        BlurRectParams {
            x: 8,
            y: 8,
            w: 48,
            h: 48,
            strength: 12.0,
            style: BlurStyle::Secure,
            cacheable: false,
        },
        &replay_ctx,
    );
    drop(ctx);

    // Sample across what were alternating black and white stripes.
    let samples: Vec<_> = (12..52)
        .step_by(4)
        .map(|x| pixel(&mut surface, x, 32))
        .collect();
    let first = samples[0];
    for sample in &samples {
        let spread = i32::from(sample.0).abs_diff(i32::from(first.0));
        assert!(
            spread <= 2,
            "secure blur left detectable variation: {samples:?}"
        );
    }
}

#[test]
fn pixelate_keeps_variation_that_secure_removes() {
    let backdrop = striped_backdrop(64, 64);
    let replay_ctx = with_backdrop(&backdrop);

    let spread_for = |style: BlurStyle| {
        let (mut surface, ctx) = surface_with_context(64, 64);
        render_blur_rect(
            &ctx,
            BlurRectParams {
                x: 0,
                y: 0,
                w: 64,
                h: 64,
                strength: 1.0,
                style,
                cacheable: false,
            },
            &replay_ctx,
        );
        drop(ctx);
        let samples: Vec<u8> = (2..62)
            .step_by(2)
            .map(|x| pixel(&mut surface, x, 32).0)
            .collect();
        let min = samples.iter().copied().min().unwrap_or(0);
        let max = samples.iter().copied().max().unwrap_or(0);
        u32::from(max - min)
    };

    let pixelate_spread = spread_for(BlurStyle::Pixelate);
    let secure_spread = spread_for(BlurStyle::Secure);
    assert!(
        pixelate_spread > secure_spread,
        "pixelate should retain block contrast ({pixelate_spread}) that secure discards ({secure_spread})"
    );
}

#[test]
fn secure_style_stays_visible_against_the_content_it_covers() {
    // A flat average of the region can land indistinguishable from its
    // surroundings, which would read as "nothing was redacted".
    let backdrop = striped_backdrop(64, 64);
    let (mut surface, ctx) = surface_with_context(64, 64);
    let replay_ctx = with_backdrop(&backdrop);

    // Paint the backdrop first so the comparison is against real content.
    let _ = ctx.set_source_surface(&backdrop, 0.0, 0.0);
    let _ = ctx.paint();
    render_blur_rect(
        &ctx,
        BlurRectParams {
            x: 16,
            y: 16,
            w: 32,
            h: 32,
            strength: 12.0,
            style: BlurStyle::Secure,
            cacheable: false,
        },
        &replay_ctx,
    );
    drop(ctx);

    let inside = pixel(&mut surface, 32, 32);
    let outside = pixel(&mut surface, 4, 32);
    let contrast = i32::from(inside.0).abs_diff(i32::from(outside.0));
    assert!(
        contrast > 40,
        "secure panel must be distinguishable from surrounding content \
         (inside {inside:?} vs outside {outside:?})"
    );
}
