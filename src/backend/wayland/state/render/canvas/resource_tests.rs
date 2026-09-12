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
    let geometry = FrameGeometry::new(inputs.width, inputs.height, inputs.scale);
    let mut surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        geometry.physical_width as i32,
        geometry.physical_height as i32,
    )
    .unwrap();
    {
        let cairo = cairo::Context::new(&surface).unwrap();
        if let Some(color) = inputs.background {
            cairo.set_source_rgba(color.r, color.g, color.b, color.a);
            cairo.paint().unwrap();
        }
        cairo.scale(inputs.scale as f64, inputs.scale as f64);
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
        let paper = inputs
            .background
            .filter(|_| inputs.grid.kind != crate::domain::BoardGridKind::None)
            .map(|color| crate::draw::BoardPaper::for_context(color, inputs.grid, &cairo).unwrap());
        if let Some(paper) = &paper {
            paper.paint(&cairo).unwrap();
        }
        let replay = crate::draw::EraserReplayContext {
            pattern: paper.as_ref().map(crate::draw::BoardPaper::pattern),
            surface: None,
            backdrop_cache_key: None,
            bg_color: inputs.background,
            logical_to_image_scale_x: 1.0,
            logical_to_image_scale_y: 1.0,
            logical_image_origin_x: 0.0,
            logical_image_origin_y: 0.0,
        };
        render_committed_canvas_shapes(
            measurer, shapes, layer, caches, &canvas, cached, &replay, None,
        );
    }
    surface.flush();
    surface.data().unwrap().to_vec()
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
    let replay = crate::draw::EraserReplayContext {
        pattern: None,
        surface: None,
        backdrop_cache_key: None,
        bg_color: None,
        logical_to_image_scale_x: 1.0,
        logical_to_image_scale_y: 1.0,
        logical_image_origin_x: 0.0,
        logical_image_origin_y: 0.0,
    };
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
                &replay,
                Some(&mut perf),
            );
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
