//! Opt-in end-to-end paper consumers. No compositor or native windows.
use super::*;
use crate::canvas_export::{
    BoardExportSnapshot, CanvasExportBackdropSnapshot, CanvasExportSnapshot, CanvasExportViewport,
    render_canvas_png,
};
use crate::domain::{BoardGrid, BoardGridKind};
use std::time::Instant;

fn workload(origin: (i32, i32), size: (u32, u32), count: usize) -> crate::draw::Frame {
    let mut frame = crate::draw::Frame::new();
    for n in 0..80 {
        frame.add_shape(Shape::Line {
            x1: origin.0,
            y1: origin.1 + n * 19,
            x2: origin.0 + size.0 as i32,
            y2: origin.1 + n * 19 + 60,
            color: crate::draw::BLUE,
            thick: 3.0,
        });
    }
    for n in 0..count {
        let (x, y) = (
            origin.0 + (n as i32 * 73 % size.0 as i32),
            origin.1 + (n as i32 * 47 % size.1 as i32),
        );
        frame.add_shape(Shape::EraserStroke {
            points: vec![(x, y), (x + 100, y + 24), (x + 180, y + 10)],
            brush: EraserBrush {
                kind: if n % 2 == 0 {
                    EraserKind::Circle
                } else {
                    EraserKind::Rect
                },
                size: 32.0,
            },
        });
    }
    frame
}

#[test]
#[ignore = "opt-in cold/warm pan and PNG benchmark"]
fn board_grid_consumers_performance() {
    println!(
        "kind,spacing,width,height,origin,erasers,cold_pan_median_us,cold_pan_p95_us,warm_pan_median_us,warm_pan_p95_us,png_median_us,png_p95_us,png_bytes,surface_bytes_proxy"
    );
    for size in [(1920, 1080), (3840, 2160)] {
        for origin in [(0, 0), (-1_000_021, -2_000_003)] {
            for kind in BoardGridKind::ALL {
                for spacing in [8, 40] {
                    for count in [0, 20, 200] {
                        let grid = BoardGrid::new(kind, spacing);
                        let frame = workload(origin, size, count);
                        let request = CanvasLayerInputs {
                            width: size.0,
                            height: size.1,
                            origin: (f64::from(origin.0), f64::from(origin.1)),
                            background: Some(crate::draw::WHITE),
                            grid,
                            ..inputs()
                        };
                        let snapshot = CanvasExportSnapshot {
                            viewport: CanvasExportViewport {
                                logical_width: size.0,
                                logical_height: size.1,
                                scale: 1,
                                origin_x: origin.0,
                                origin_y: origin.1,
                            },
                            backdrop: CanvasExportBackdropSnapshot::board_paper(
                                crate::draw::WHITE,
                                grid,
                            ),
                            board: BoardExportSnapshot {
                                frame: frame.clone_without_history(),
                            },
                            render_profile: None,
                            text_halo_enabled: true,
                            spotlight: Default::default(),
                        };
                        let mut samples = [Vec::new(), Vec::new(), Vec::new()];
                        let mut png_bytes = 0;
                        let measurer = crate::draw::TextMeasurer::default();
                        for sample in 0..6 {
                            let mut layer = CanvasLayerCache::new();
                            let mut caches = crate::draw::RenderCaches::default();
                            let start = Instant::now();
                            assert!(layer.ensure(&measurer, &mut caches, &frame.shapes, request));
                            let cold = start.elapsed().as_micros();
                            let target = cairo::ImageSurface::create(
                                cairo::Format::ARgb32,
                                size.0 as i32,
                                size.1 as i32,
                            )
                            .unwrap();
                            let ctx = cairo::Context::new(&target).unwrap();
                            ctx.translate(-f64::from(origin.0), -f64::from(origin.1));
                            let start = Instant::now();
                            assert!(layer.ensure(&measurer, &mut caches, &frame.shapes, request));
                            assert!(layer.blit(&ctx));
                            let warm = start.elapsed().as_micros();
                            let start = Instant::now();
                            png_bytes = render_canvas_png(&snapshot).unwrap().bytes.len();
                            let png = start.elapsed().as_micros();
                            if sample > 0 {
                                for (values, value) in samples.iter_mut().zip([cold, warm, png]) {
                                    values.push(value);
                                }
                            }
                        }
                        for values in &mut samples {
                            values.sort_unstable();
                        }
                        // Simultaneous bake + blit target + PNG target, excluding bounded tile and codec scratch.
                        let bytes = (u64::from(size.0 + 512) * u64::from(size.1 + 512)
                            + 2 * u64::from(size.0) * u64::from(size.1))
                            * 4;
                        println!(
                            "{kind:?},{spacing},{},{},{},{count},{},{},{},{},{},{},{png_bytes},{bytes}",
                            size.0,
                            size.1,
                            origin.0,
                            samples[0][2],
                            samples[0][4],
                            samples[1][2],
                            samples[1][4],
                            samples[2][2],
                            samples[2][4]
                        );
                    }
                }
            }
        }
    }
}
