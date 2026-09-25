use super::*;
use crate::backend::wayland::state::canvas_layer::{CanvasLayerCache, CanvasLayerInputs};
use crate::backend::wayland::state::render::plan::{CanvasFrame, FrameGeometry};
use crate::draw::{Color, DrawnShape, EmbeddedImage, EraserBrush, EraserKind, Shape};

fn inputs() -> CanvasLayerInputs {
    CanvasLayerInputs {
        grid: Default::default(),
        width: 80,
        height: 64,
        scale: 1,
        raster_dimensions: None,
        origin: (0.0, 0.0),
        background: Some(Color {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 1.0,
        }),
        text_halo_enabled: true,
        board_key: (0, 0),
        generation: 1,
    }
}

fn shapes() -> Vec<DrawnShape> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    let source = cairo::ImageSurface::create(cairo::Format::ARgb32, 2, 2).unwrap();
    let context = cairo::Context::new(&source).unwrap();
    context.set_source_rgb(220.0 / 255.0, 40.0 / 255.0, 20.0 / 255.0);
    context.paint().unwrap();
    source.write_to_png(&mut bytes).unwrap();
    [
        Shape::Image {
            x: 5,
            y: 5,
            w: 28,
            h: 28,
            data: EmbeddedImage {
                mime_type: "image/png".into(),
                width: 2,
                height: 2,
                bytes: bytes.into_inner().into(),
            },
        },
        Shape::Text {
            x: 8,
            y: 52,
            text: "Cache".into(),
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            size: 14.0,
            font_descriptor: crate::draw::FontDescriptor::default(),
            background_enabled: false,
            wrap_width: None,
        },
        Shape::EraserStroke {
            points: vec![(6, 16), (31, 16)],
            brush: EraserBrush {
                size: 6.0,
                kind: EraserKind::Circle,
            },
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(id, shape)| DrawnShape::with_metadata(id as u64, shape, 0, false))
    .collect()
}

fn paint(
    measurer: &crate::draw::TextMeasurer,
    shapes: &[DrawnShape],
    layer: &CanvasLayerCache,
    caches: &mut crate::draw::RenderCaches,
    inputs: CanvasLayerInputs,
    cached: bool,
) -> Vec<u8> {
    paint_with_preferred_scale(measurer, shapes, layer, caches, inputs, cached, Some(180))
}

fn paint_with_preferred_scale(
    measurer: &crate::draw::TextMeasurer,
    shapes: &[DrawnShape],
    layer: &CanvasLayerCache,
    caches: &mut crate::draw::RenderCaches,
    inputs: CanvasLayerInputs,
    cached: bool,
    preferred_scale: Option<u32>,
) -> Vec<u8> {
    let mut geometry = FrameGeometry::new(inputs.width, inputs.height, inputs.scale);
    if let Some((pixel_width, pixel_height)) = inputs.raster_dimensions {
        geometry.physical_width = pixel_width;
        geometry.physical_height = pixel_height;
        geometry.stride = (pixel_width * 4) as i32;
        geometry.byte_len = pixel_height as usize * geometry.stride as usize;
        geometry.preferred_scale = preferred_scale;
        geometry.wire_scale = 1;
    }
    let mut surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        geometry.physical_width as i32,
        geometry.physical_height as i32,
    )
    .unwrap();
    {
        let cairo = cairo::Context::new(&surface).unwrap();
        cairo.scale(geometry.scale_x(), geometry.scale_y());
        cairo.translate(-inputs.origin.0, -inputs.origin.1);
        let frame = CanvasFrame {
            draw_committed: true,
            render_transients: false,
            transform_active: true,
            origin: inputs.origin,
            zoom_scale: None,
            text_halo_enabled: inputs.text_halo_enabled,
            layer_cache_eligible: true,
        };
        let canvas = CanvasRenderCtx {
            cairo: &cairo,
            geometry: &geometry,
            canvas: &frame,
            damage_world: &[],
            now: Instant::now(),
        };
        let mut backdrop = background::CanvasEraserContext::for_board(inputs.background);
        let mut perf = PerfRenderBreakdown::default();
        render_committed_canvas_shapes(
            measurer,
            shapes,
            layer,
            caches,
            &canvas,
            cached,
            &mut backdrop,
            inputs.grid,
            Some(&mut perf),
        )
        .unwrap();
        if perf.canvas_layer_cache_used {
            assert!(
                backdrop.replay_context().pattern.is_none(),
                "cache hits must not construct or paint a paper source"
            );
        }
    }
    surface.flush();
    surface.data().unwrap().to_vec()
}

struct LocalChannelError {
    samples: u64,
    mean: f64,
    tiles: u32,
    worst_tile: f64,
}

/// Region mean plus the worst 8×8 window. Windows step by four pixels so a
/// defect on a boundary stays inside one sample. A small defect can disappear
/// into the region mean while still failing the window bound.
fn local_channel_error(
    direct: &[u8],
    cached: &[u8],
    raster_width: usize,
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
) -> LocalChannelError {
    let mut error = 0_u64;
    let mut samples = 0_u64;
    for y in top..bottom {
        for x in left..right {
            let offset = (y * raster_width + x) * 4;
            for channel in 0..4 {
                error += u64::from(direct[offset + channel].abs_diff(cached[offset + channel]));
                samples += 1;
            }
        }
    }

    let mut worst_tile = 0.0_f64;
    let mut tiles = 0_u32;
    let mut tile_y = top;
    while tile_y + 8 <= bottom {
        let mut tile_x = left;
        while tile_x + 8 <= right {
            let mut tile_error = 0_u64;
            let mut tile_samples = 0_u64;
            for y in tile_y..tile_y + 8 {
                for x in tile_x..tile_x + 8 {
                    let offset = (y * raster_width + x) * 4;
                    for channel in 0..4 {
                        tile_error +=
                            u64::from(direct[offset + channel].abs_diff(cached[offset + channel]));
                        tile_samples += 1;
                    }
                }
            }
            worst_tile = worst_tile.max(tile_error as f64 / tile_samples as f64);
            tiles += 1;
            tile_x += 4;
        }
        tile_y += 4;
    }

    LocalChannelError {
        samples,
        mean: if samples == 0 {
            0.0
        } else {
            error as f64 / samples as f64
        },
        tiles,
        worst_tile,
    }
}

#[test]
fn local_channel_error_keeps_a_concentrated_defect_out_of_the_region_mean() {
    let width = 64;
    let height = 32;
    let mut direct = vec![0_u8; width * height * 4];
    let cached = vec![0_u8; direct.len()];
    for y in 0..4 {
        for x in 0..4 {
            for channel in 0..4 {
                direct[(y * width + x) * 4 + channel] = 255;
            }
        }
    }

    let error = local_channel_error(&direct, &cached, width, 0, 0, width, height);

    assert!(error.mean < 3.0, "region mean hid nothing: {}", error.mean);
    assert!(
        error.worst_tile >= 24.0,
        "tile mean missed the defect: {}",
        error.worst_tile
    );
}

fn assert_pixels_match(actual: &[u8], expected: &[u8], label: &str) {
    assert_eq!(actual.len(), expected.len(), "{label}: buffer length");
    if let Some((index, (actual, expected))) = actual
        .iter()
        .zip(expected)
        .enumerate()
        .find(|(_, (a, b))| a != b)
    {
        panic!("{label}: first difference at byte {index}: actual={actual}, expected={expected}");
    }
}

fn fresh_baked(shapes: &[DrawnShape], request: CanvasLayerInputs) -> Vec<u8> {
    let measurer = crate::draw::TextMeasurer::default();
    let mut layer = CanvasLayerCache::new();
    let mut caches = crate::draw::RenderCaches::default();
    assert!(layer.ensure(&measurer, &mut caches, shapes, request));
    paint(&measurer, shapes, &layer, &mut caches, request, true)
}

#[test]
fn baked_and_direct_passes_match_fresh_owners_across_reuse_and_invalidation() {
    let measurer = crate::draw::TextMeasurer::default();
    let mut layer = CanvasLayerCache::new();
    let mut caches = crate::draw::RenderCaches::default();
    let mut shapes = shapes();
    let initial = inputs();
    for (iteration, request) in [
        initial,
        initial,
        CanvasLayerInputs {
            origin: (12.0, 8.0),
            ..initial
        },
        CanvasLayerInputs {
            scale: 2,
            ..initial
        },
        CanvasLayerInputs {
            raster_dimensions: Some((100, 80)),
            origin: (12.5, 8.25),
            ..initial
        },
        CanvasLayerInputs {
            raster_dimensions: Some((133, 107)),
            origin: (12.5, 8.25),
            ..initial
        },
        CanvasLayerInputs {
            board_key: (1, 1),
            ..initial
        },
        CanvasLayerInputs {
            generation: 2,
            ..initial
        },
        CanvasLayerInputs {
            origin: (600.0, 0.0),
            ..initial
        },
    ]
    .into_iter()
    .enumerate()
    {
        if request.generation == 2 {
            shapes[0].set_shape(Shape::Rect {
                x: 3,
                y: 3,
                w: 30,
                h: 20,
                color: Color {
                    r: 0.0,
                    g: 0.8,
                    b: 0.2,
                    a: 1.0,
                },
                thick: 2.0,
                fill: true,
            });
        }
        assert!(layer.ensure(&measurer, &mut caches, &shapes, request));
        let baked = paint(&measurer, &shapes, &layer, &mut caches, request, true);
        let direct = paint(&measurer, &shapes, &layer, &mut caches, request, false);
        // Direct eraser edges retain partial alpha; a baked surface is later
        // composited over the background. Compare each established rendering
        // route to itself with fresh resources, not to the other route.
        assert_pixels_match(
            &baked,
            &fresh_baked(&shapes, request),
            &format!("baked pass {iteration}"),
        );
        let mut fresh = crate::draw::RenderCaches::default();
        let expected_direct = paint(
            &crate::draw::TextMeasurer::default(),
            &shapes,
            &CanvasLayerCache::new(),
            &mut fresh,
            request,
            false,
        );
        assert_pixels_match(
            &direct,
            &expected_direct,
            &format!("direct pass {iteration}"),
        );
    }
}

#[test]
fn fractional_baked_pan_matches_direct_rendering() {
    let measurer = crate::draw::TextMeasurer::default();
    let mut layer = CanvasLayerCache::new();
    let mut caches = crate::draw::RenderCaches::default();
    let shapes = shapes();
    let mut previous_direct = None;
    for preferred_scale in [150, 180, 210] {
        let raster_width = (81 * preferred_scale + 60) / 120;
        let raster_height = (63 * preferred_scale + 60) / 120;
        for origin in [(0.0, 0.0), (3.25, 2.5), (8.5, 5.75), (12.25, 7.125)] {
            let request = CanvasLayerInputs {
                width: 81,
                height: 63,
                raster_dimensions: Some((raster_width, raster_height)),
                origin,
                ..inputs()
            };
            assert!(layer.ensure(&measurer, &mut caches, &shapes, request));
            let direct = paint_with_preferred_scale(
                &measurer,
                &shapes,
                &layer,
                &mut caches,
                request,
                false,
                Some(preferred_scale),
            );
            let cached = paint_with_preferred_scale(
                &measurer,
                &shapes,
                &layer,
                &mut caches,
                request,
                true,
                Some(preferred_scale),
            );
            if let Some(previous) = previous_direct {
                assert_ne!(direct, previous, "pan or scale must move drawn content");
            }
            previous_direct = Some(direct.clone());

            let scale = raster_width as f64 / request.width as f64;
            let left = ((5.0 - origin.0) * scale).max(0.0) as usize;
            let top = ((5.0 - origin.1) * scale).max(0.0) as usize;
            let right = ((36.0 - origin.0) * scale).min(raster_width as f64) as usize;
            let bottom = ((38.0 - origin.1) * scale).min(raster_height as f64) as usize;
            let error = local_channel_error(
                &direct,
                &cached,
                raster_width as usize,
                left,
                top,
                right,
                bottom,
            );
            assert!(error.samples > 0);
            assert!(error.tiles > 0, "content region must contain an 8x8 window");
            assert!(
                error.mean < 3.0,
                "local image error at {preferred_scale}% origin {origin:?}: {:.3}",
                error.mean
            );
            assert!(
                error.worst_tile < 24.0,
                "concentrated tile error at {preferred_scale}% origin {origin:?}: {:.3}",
                error.worst_tile
            );
        }
    }
}

#[test]
fn rejected_bake_clears_previous_layer_and_direct_fallback_still_paints() {
    let measurer = crate::draw::TextMeasurer::default();
    let mut layer = CanvasLayerCache::new();
    let mut caches = crate::draw::RenderCaches::default();
    let shapes = shapes();
    let request = inputs();
    assert!(layer.ensure(&measurer, &mut caches, &shapes, request));
    assert!(!layer.ensure(
        &measurer,
        &mut caches,
        &shapes,
        CanvasLayerInputs {
            width: 40_000,
            ..request
        }
    ));
    assert!(!layer.ensure(
        &measurer,
        &mut caches,
        &shapes,
        CanvasLayerInputs {
            raster_dimensions: Some((0, 96)),
            ..request
        }
    ));
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).unwrap();
    assert!(!layer.blit(&cairo::Context::new(&surface).unwrap()));
    assert_pixels_match(
        &paint(&measurer, &shapes, &layer, &mut caches, request, true),
        &paint(&measurer, &shapes, &layer, &mut caches, request, false),
        "invalid layer falls back to direct rendering",
    );
}

#[test]
fn each_scene_key_rebakes_without_shape_identity_changes() {
    let measurer = crate::draw::TextMeasurer::default();
    let initial = inputs();
    for (name, changed, replace_image) in [
        (
            "generation",
            CanvasLayerInputs {
                generation: 2,
                ..initial
            },
            true,
        ),
        (
            "board",
            CanvasLayerInputs {
                board_key: (1, 0),
                ..initial
            },
            true,
        ),
        (
            "page",
            CanvasLayerInputs {
                board_key: (0, 1),
                ..initial
            },
            true,
        ),
        (
            "background",
            CanvasLayerInputs {
                background: Some(Color {
                    r: 0.8,
                    g: 0.2,
                    b: 0.1,
                    a: 1.0,
                }),
                ..initial
            },
            false,
        ),
        (
            "halo",
            CanvasLayerInputs {
                text_halo_enabled: false,
                ..initial
            },
            false,
        ),
    ] {
        let mut layer = CanvasLayerCache::new();
        let mut caches = crate::draw::RenderCaches::default();
        let mut scene = shapes();
        assert!(layer.ensure(&measurer, &mut caches, &scene, initial));
        let before = paint(&measurer, &scene, &layer, &mut caches, initial, true);
        if replace_image {
            // Shape count and IDs remain unchanged; only the scene key can
            // invalidate the already baked pixels for this different scene.
            scene[0].set_shape(Shape::Rect {
                x: 3,
                y: 3,
                w: 30,
                h: 20,
                color: Color {
                    r: 0.0,
                    g: 0.8,
                    b: 0.2,
                    a: 1.0,
                },
                thick: 2.0,
                fill: true,
            });
        }
        assert!(layer.ensure(&measurer, &mut caches, &scene, changed));
        let actual = paint(&measurer, &scene, &layer, &mut caches, changed, true);
        let expected = fresh_baked(&scene, changed);
        assert!(before != expected, "fixture must change pixels for {name}");
        assert_pixels_match(
            &actual,
            &expected,
            &format!("stale layer after {name} changed"),
        );
    }
}

#[test]
#[ignore = "release timing workload; run with --release --ignored --nocapture"]
fn measure_sparse_damage_scan() {
    let measurer = crate::draw::TextMeasurer::default();
    let layer = CanvasLayerCache::new();
    let mut caches = crate::draw::RenderCaches::default();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1920, 1080).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let geometry = FrameGeometry::new(1920, 1080, 1);
    let frame = CanvasFrame {
        draw_committed: true,
        render_transients: false,
        transform_active: false,
        origin: (0.0, 0.0),
        zoom_scale: None,
        text_halo_enabled: true,
        layer_cache_eligible: false,
    };
    let damage = [crate::util::Rect {
        x: 0,
        y: 0,
        width: 32,
        height: 32,
    }];
    let canvas = CanvasRenderCtx {
        cairo: &ctx,
        geometry: &geometry,
        canvas: &frame,
        damage_world: &damage,
        now: Instant::now(),
    };
    let mut backdrop = background::CanvasEraserContext::for_board(None);

    for count in [100, 1_000, 10_000] {
        let shapes: Vec<_> = (0..count)
            .map(|i| {
                let x = (i % 100) * 18;
                let y = (i / 100) * 10;
                DrawnShape::with_metadata(
                    i as u64,
                    Shape::Line {
                        x1: x,
                        y1: y,
                        x2: x + 8,
                        y2: y + 5,
                        color: Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        },
                        thick: 2.0,
                    },
                    0,
                    false,
                )
            })
            .collect();
        let mut perf = PerfRenderBreakdown::default();
        let start = Instant::now();
        for _ in 0..500 {
            render_committed_canvas_shapes(
                &measurer,
                &shapes,
                &layer,
                &mut caches,
                &canvas,
                false,
                &mut backdrop,
                Default::default(),
                Some(&mut perf),
            )
            .unwrap();
        }
        eprintln!(
            "P03 shapes={count} tested={} rendered={} mean_us={:.2}",
            perf.shapes_tested,
            perf.shapes_rendered,
            start.elapsed().as_micros() as f64 / 500.0
        );
    }
}

#[test]
fn board_grid_baked_pan_matches_direct_and_invalidates_on_pattern_and_spacing() {
    use crate::domain::{BoardGrid, BoardGridKind};
    let measurer = crate::draw::TextMeasurer::default();
    let mut cache = CanvasLayerCache::new();
    let mut caches = crate::draw::RenderCaches::default();
    let shapes = shapes();
    for origin in [(0.0, 0.0), (-71.0, -53.0), (-1_000_021.0, -2_000_003.0)] {
        for kind in BoardGridKind::ALL {
            for spacing in [8, 40] {
                let request = CanvasLayerInputs {
                    grid: BoardGrid::new(kind, spacing),
                    origin,
                    ..inputs()
                };
                assert!(cache.ensure(&measurer, &mut caches, &shapes, request));
                let direct = paint(&measurer, &shapes, &cache, &mut caches, request, false);
                let cached = paint(&measurer, &shapes, &cache, &mut caches, request, true);
                let error: u64 = direct
                    .iter()
                    .zip(&cached)
                    .map(|(a, b)| u64::from(a.abs_diff(*b)))
                    .sum();
                assert!(
                    error as f64 / (direct.len() as f64) < 1.0,
                    "{kind:?} {spacing} {origin:?}: cached phase differs"
                );
            }
        }
    }
}

mod grid_performance;

#[test]
fn warm_4k_paper_render_skips_source_and_failed_blit_restores_paper() {
    let measurer = crate::draw::TextMeasurer::default();
    let request = CanvasLayerInputs {
        width: 3840,
        height: 2160,
        grid: crate::domain::BoardGrid::new(crate::domain::BoardGridKind::Isometric, 40),
        ..inputs()
    };
    let mut cache = CanvasLayerCache::new();
    let mut caches = crate::draw::RenderCaches::default();
    assert!(cache.ensure(&measurer, &mut caches, &[], request));
    let warm = paint(&measurer, &[], &cache, &mut caches, request, true);
    // Simulate an unavailable cache even though readiness was reported. The
    // production dispatcher must paint the backdrop before replaying shapes.
    cache.clear();
    let fallback = paint(&measurer, &[], &cache, &mut caches, request, true);
    let direct = paint(&measurer, &[], &cache, &mut caches, request, false);
    assert_eq!(fallback, direct);
    let mean_error = warm
        .iter()
        .zip(&direct)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum::<u64>() as f64
        / warm.len() as f64;
    assert!(mean_error < 1.0, "cached paper differs: {mean_error}");
}
