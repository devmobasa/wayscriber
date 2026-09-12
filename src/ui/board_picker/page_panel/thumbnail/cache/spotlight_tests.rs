use super::*;
use crate::draw::{Color, Frame, Shape};

fn pixels(
    frame: &Frame,
    background: &BoardBackground,
    scales: (f64, f64),
    format: cairo::Format,
    partial_clip: bool,
    cache: Option<&mut ThumbnailCache>,
) -> Vec<u8> {
    let (device, matrix) = scales;
    let mut surface = cairo::ImageSurface::create(
        format,
        (220.0 * device * matrix).ceil() as i32,
        (170.0 * device * matrix).ceil() as i32,
    )
    .unwrap();
    surface.set_device_scale(device, device);
    {
        let ctx = cairo::Context::new(&surface).unwrap();
        ctx.scale(matrix, matrix);
        ctx.set_source_rgba(0.2, 0.3, 0.4, 0.75);
        ctx.paint().unwrap();
        for i in 0..30 {
            ctx.set_source_rgba(0.7, 0.5, 0.3, 0.6);
            ctx.rectangle(f64::from(i * 9), 0.0, 3.0, 170.0);
            ctx.fill().unwrap();
        }
        if partial_clip {
            ctx.rectangle(45.0, 35.0, 100.0, 75.0);
            ctx.clip();
        }
        let engine = UiTextEngine::default();
        let measurer = TextMeasurer::default();
        let mut draw = crate::draw::RenderCaches::default();
        let mut render = RenderCtx::new(&ctx, &mut draw);
        let args = PageContentArgs {
            render: &mut render,
            frame,
            background,
            grid: Default::default(),
            x: 35.5,
            y: 27.25,
            width: 128.0,
            height: 96.0,
            screen_width: 400,
            screen_height: 300,
            text_halo_enabled: true,
        };
        if let Some(cache) = cache {
            render_cached_page_content(&engine, &measurer, cache, args);
        } else {
            render_page_content(&engine, &measurer, args);
        }
    }
    let mut data = surface.data().unwrap().to_vec();
    if format == cairo::Format::Rgb24 {
        for pixel in data.as_chunks_mut::<4>().0 {
            let value = u32::from_ne_bytes(*pixel) | 0xff00_0000;
            pixel.copy_from_slice(&value.to_ne_bytes());
        }
    }
    data
}

#[test]
fn edge_spotlights_preserve_sampling_on_first_repeated_and_clipped_paints() {
    for (cx, cy) in [(440, 140), (-40, 140), (200, -40), (200, 340)] {
        let mut frame = super::tests::sample_frame();
        frame.add_shape(Shape::Spotlight {
            cx,
            cy,
            rx: 240,
            ry: 140,
            magnification: 2.5,
        });
        for background in [
            BoardBackground::Solid(Color::new(0.8, 0.9, 1.0, 1.0)),
            BoardBackground::Transparent,
        ] {
            let magnifying = matches!(background, BoardBackground::Solid(_));
            for scales in [(1.0, 1.0), (1.25, 1.0), (2.0, 1.0), (1.0, 1.25)] {
                for format in [cairo::Format::ARgb32, cairo::Format::Rgb24] {
                    let mut cache = ThumbnailCache::default();
                    for partial_clip in [false, true, false] {
                        let expected =
                            pixels(&frame, &background, scales, format, partial_clip, None);
                        for _ in 0..2 {
                            let actual = pixels(
                                &frame,
                                &background,
                                scales,
                                format,
                                partial_clip,
                                Some(&mut cache),
                            );
                            let mismatches =
                                actual.iter().zip(&expected).filter(|(a, b)| a != b).count();
                            let max_delta = actual
                                .iter()
                                .zip(&expected)
                                .map(|(a, b)| a.abs_diff(*b))
                                .max()
                                .unwrap();
                            let exact = magnifying || partial_clip;
                            assert!(
                                if exact {
                                    mismatches == 0
                                } else {
                                    max_delta <= 1 && mismatches <= expected.len() / 1000
                                },
                                "Spotlight ({cx}, {cy}), {background:?}, {scales:?}, {format:?}, partial clip {partial_clip}: {mismatches} differing bytes, maximum delta {max_delta}"
                            );
                        }
                    }
                    if magnifying {
                        assert!(cache.entries.is_empty());
                        assert_eq!(cache.hits, 0);
                    } else {
                        assert_eq!(cache.hits, 3, "transparent previews still reuse the raster");
                    }
                }
            }
        }
    }
}
