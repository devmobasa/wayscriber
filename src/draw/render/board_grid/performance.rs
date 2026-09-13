//! Opt-in source comparison; no compositor, font stack, or desktop windows.
use super::*;
use crate::draw::{EraserBrush, EraserKind, EraserReplayContext};
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
enum Source {
    Solid,
    Viewport,
    Tile,
}

fn render_case(
    source: Source,
    kind: BoardGridKind,
    spacing: i64,
    size: (i32, i32),
    erasers: usize,
    pdf: bool,
) -> (u128, usize) {
    let start = Instant::now();
    let target: cairo::Surface = if pdf {
        cairo::PdfSurface::for_stream(size.0 as f64, size.1 as f64, Vec::<u8>::new())
            .unwrap()
            .as_ref()
            .clone()
    } else {
        cairo::ImageSurface::create(cairo::Format::ARgb32, size.0, size.1)
            .unwrap()
            .as_ref()
            .clone()
    };
    let ctx = Context::new(&target).unwrap();
    ctx.translate(1_000_021.0, 2_000_003.0);
    let grid = BoardGrid::new(kind, spacing);
    let pattern = match source {
        Source::Solid => None,
        Source::Tile => Some(
            BoardPaper::for_context(crate::draw::WHITE, grid, &ctx)
                .unwrap()
                .pattern,
        ),
        Source::Viewport => {
            let bounds = Rectangle::new(-1_000_021.0, -2_000_003.0, size.0 as f64, size.1 as f64);
            let record =
                RecordingSurface::create(cairo::Content::ColorAlpha, Some(bounds)).unwrap();
            paint_geometry(
                &Context::new(&record).unwrap(),
                crate::draw::WHITE,
                grid,
                1.0,
                bounds,
            )
            .unwrap();
            Some(SurfacePattern::create(&record))
        }
    };
    if let Some(p) = pattern.as_ref() {
        ctx.set_source(p).unwrap();
    } else {
        ctx.set_source_rgb(1.0, 1.0, 1.0);
    }
    ctx.paint().unwrap();
    // Stable annotation workload, including content under eraser strokes.
    ctx.set_source_rgb(0.2, 0.3, 0.7);
    ctx.set_line_width(3.0);
    for n in 0..80 {
        let y = -2_000_003.0 + f64::from(n * 19);
        ctx.move_to(-1_000_021.0, y);
        ctx.line_to(-1_000_021.0 + f64::from(size.0), y + 60.0);
    }
    ctx.stroke().unwrap();
    let replay = EraserReplayContext {
        pattern: pattern.as_ref().map(|p| p.as_ref()),
        surface: None,
        backdrop_cache_key: None,
        bg_color: Some(crate::draw::WHITE),
        logical_to_image_scale_x: 1.0,
        logical_to_image_scale_y: 1.0,
        logical_image_origin_x: 0.0,
        logical_image_origin_y: 0.0,
    };
    // The direct raster path also replays one provisional eraser. Exports do not.
    for n in 0..erasers + usize::from(!pdf) {
        let x = -1_000_021 + (n as i32 * 73 % size.0);
        let y = -2_000_003 + (n as i32 * 47 % size.1);
        let brush = EraserBrush {
            kind: if n % 2 == 0 {
                EraserKind::Circle
            } else {
                EraserKind::Rect
            },
            size: 32.0,
        };
        super::super::strokes::render_eraser_stroke(
            &ctx,
            &[(x, y), (x + 100, y + 24), (x + 180, y + 10)],
            &brush,
            &replay,
        );
    }
    drop(ctx);
    let bytes = if pdf {
        target
            .finish_output_stream()
            .unwrap()
            .downcast::<Vec<u8>>()
            .unwrap()
            .len()
    } else {
        target.flush();
        size.0 as usize * size.1 as usize * 4
    };
    (start.elapsed().as_micros(), bytes)
}

#[test]
#[ignore = "opt-in board-paper CPU/PDF comparison; run with --nocapture --test-threads=1"]
fn board_grid_backdrop_performance() {
    let sources: &[Source] = if std::env::var_os("WAYSCRIBER_GRID_COMPARE_VIEWPORT").is_some() {
        &[Source::Solid, Source::Viewport, Source::Tile]
    } else {
        &[Source::Solid, Source::Tile]
    };
    println!("source,kind,spacing,width,height,erasers,pdf,median_us,p95_us,target_or_pdf_bytes");
    for size in [(1920, 1080), (3840, 2160)] {
        for kind in BoardGridKind::ALL.into_iter().skip(1) {
            for spacing in [8, 40] {
                for erasers in [0, 20, 200] {
                    for pdf in [false, true] {
                        for &source in sources {
                            render_case(source, kind, spacing, size, erasers, pdf);
                            let mut samples = Vec::new();
                            let mut bytes = 0;
                            for _ in 0..5 {
                                let (time, n) =
                                    render_case(source, kind, spacing, size, erasers, pdf);
                                samples.push(time);
                                bytes = n;
                            }
                            samples.sort_unstable();
                            println!(
                                "{source:?},{kind:?},{spacing},{},{},{erasers},{pdf},{},{},{bytes}",
                                size.0, size.1, samples[2], samples[4]
                            );
                        }
                    }
                }
            }
        }
    }
}
