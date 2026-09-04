use super::{RenderCaches, RenderCtx};
use crate::draw::{BlurRectParams, BlurStyle, EmbeddedImage, EraserReplayContext, Shape};
use cairo::{Context, Format, ImageSurface};
use std::sync::Arc;

fn backdrop(red: bool) -> ImageSurface {
    let surface = ImageSurface::create(Format::ARgb32, 64, 64).unwrap();
    let ctx = Context::new(&surface).unwrap();
    ctx.set_source_rgb(
        if red { 0.9 } else { 0.1 },
        0.2,
        if red { 0.1 } else { 0.9 },
    );
    ctx.paint().unwrap();
    ctx.set_source_rgba(0.9, 0.8, 0.2, 0.7);
    ctx.rectangle(8.0, 12.0, 20.0, 32.0);
    ctx.fill().unwrap();
    surface
}

fn render_blur(
    caches: &mut RenderCaches,
    backdrop: &ImageSurface,
    generation: Option<u64>,
    params: BlurRectParams,
) -> Vec<u8> {
    let mut output = ImageSurface::create(Format::ARgb32, 64, 64).unwrap();
    let ctx = Context::new(&output).unwrap();
    let replay = EraserReplayContext {
        pattern: None,
        surface: Some(backdrop),
        backdrop_cache_key: generation,
        bg_color: None,
        logical_to_image_scale_x: 1.0,
        logical_to_image_scale_y: 1.0,
        logical_image_origin_x: 0.0,
        logical_image_origin_y: 0.0,
    };
    RenderCtx::new(&ctx, caches).render_blur_rect(params, &replay);
    ctx.status().unwrap();
    drop(ctx);
    output.data().unwrap().to_vec()
}

fn params(style: BlurStyle) -> BlurRectParams {
    BlurRectParams {
        x: 4,
        y: 4,
        w: 52,
        h: 52,
        strength: 12.0,
        style,
        cacheable: true,
    }
}

#[test]
fn blur_hits_match_uncached_pixels_and_keep_source_style_and_geometry_identity() {
    let backdrop = backdrop(true);
    let mut caches = RenderCaches::default();
    for generation in [1, 2] {
        for style in [BlurStyle::Gaussian, BlurStyle::Pixelate, BlurStyle::Secure] {
            for x in [4, 9] {
                let params = BlurRectParams { x, ..params(style) };
                let miss = render_blur(&mut caches, &backdrop, Some(generation), params);
                let hit = render_blur(&mut caches, &backdrop, Some(generation), params);
                let uncached = render_blur(
                    &mut caches,
                    &backdrop,
                    Some(generation),
                    BlurRectParams {
                        cacheable: false,
                        ..params
                    },
                );
                assert_eq!(hit, miss);
                assert_eq!(
                    hit, uncached,
                    "generation {generation}, style {style:?}, x {x}"
                );
            }
        }
    }
}

#[test]
fn independent_owners_do_not_confuse_identical_backdrop_generations() {
    let red = backdrop(true);
    let blue = backdrop(false);
    let params = params(BlurStyle::Pixelate);
    let mut first = RenderCaches::default();
    let mut second = RenderCaches::default();
    let red_pixels = render_blur(&mut first, &red, Some(1), params);
    let blue_pixels = render_blur(&mut second, &blue, Some(1), params);
    assert_ne!(red_pixels, blue_pixels);
    assert_eq!(red_pixels, render_blur(&mut first, &red, Some(1), params));
    assert_eq!(
        blue_pixels,
        render_blur(&mut second, &blue, Some(1), params)
    );
    assert_eq!(blue_pixels, render_blur(&mut first, &blue, Some(2), params));
}

#[test]
fn disabled_or_unkeyed_blur_bypasses_existing_entries_and_does_not_replace_them() {
    let red = backdrop(true);
    let blue = backdrop(false);
    let params = params(BlurStyle::Gaussian);
    let mut caches = RenderCaches::default();
    let red_pixels = render_blur(&mut caches, &red, Some(1), params);
    let expected_blue = render_blur(&mut RenderCaches::default(), &blue, Some(1), params);
    for generation in [Some(1), None] {
        let bypass = render_blur(
            &mut caches,
            &blue,
            generation,
            BlurRectParams {
                cacheable: generation.is_none(),
                ..params
            },
        );
        assert_eq!(bypass, expected_blue);
        assert_ne!(bypass, red_pixels);
        assert_eq!(render_blur(&mut caches, &red, Some(1), params), red_pixels);
    }
}

fn render_image(caches: &mut RenderCaches, shape: &Shape) -> Vec<u8> {
    let mut output = ImageSurface::create(Format::ARgb32, 80, 80).unwrap();
    let ctx = Context::new(&output).unwrap();
    ctx.translate(2.0, 3.0);
    RenderCtx::new(&ctx, caches).render_shape(shape);
    assert_eq!(ctx.matrix().x0(), 2.0);
    assert_eq!(ctx.matrix().y0(), 3.0);
    assert!(!ctx.has_current_point().unwrap());
    drop(ctx);
    output.data().unwrap().to_vec()
}

#[test]
fn decoded_image_hits_match_fresh_owner_and_owner_drop_releases_payload() {
    let mut png = Vec::new();
    backdrop(true).write_to_png(&mut png).unwrap();
    let bytes: Arc<[u8]> = png.into();
    let payload = Arc::downgrade(&bytes);
    let shape = Shape::Image {
        x: 70,
        y: 65,
        w: -60,
        h: -55,
        data: EmbeddedImage {
            mime_type: "image/png".into(),
            width: 64,
            height: 64,
            bytes,
        },
    };
    let mut caches = RenderCaches::default();
    let miss = render_image(&mut caches, &shape);
    let hit = render_image(&mut caches, &shape);
    let fresh = render_image(&mut RenderCaches::default(), &shape);
    assert_eq!(miss, hit);
    assert_eq!(hit, fresh);
    assert!(hit.iter().any(|byte| *byte != 0));
    drop(shape);
    assert!(
        payload.upgrade().is_some(),
        "owner retains decoded payload identity"
    );
    drop(caches);
    assert!(
        payload.upgrade().is_none(),
        "owner drop releases payload identity"
    );
}
