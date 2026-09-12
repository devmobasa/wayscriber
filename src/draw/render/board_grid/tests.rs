use super::*;
use crate::draw::{EraserBrush, EraserKind, EraserReplayContext};

fn pixels(kind: BoardGridKind, scale: f64, origin: (f64, f64), tiled: bool) -> Vec<u8> {
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 180, 140).unwrap();
    let ctx = Context::new(&surface).unwrap();
    ctx.scale(scale, scale);
    ctx.translate(-origin.0, -origin.1);
    let grid = BoardGrid::new(kind, 20);
    let color = Color::new(1.0, 1.0, 1.0, 1.0);
    if tiled {
        BoardPaper::for_context(color, grid, &ctx)
            .unwrap()
            .paint(&ctx)
            .unwrap();
    } else {
        paint_geometry(
            &ctx,
            color,
            grid,
            scale,
            Rectangle::new(origin.0, origin.1, 180.0 / scale, 140.0 / scale),
        )
        .unwrap();
    }
    drop(ctx);
    surface.data().unwrap().to_vec()
}

#[test]
fn board_grid_tiles_match_world_geometry_at_negative_origins_and_scales() {
    for kind in BoardGridKind::ALL {
        for scale in [1.0, 1.25, 2.0] {
            for origin in [(0.0, 0.0), (-71.0, -53.0), (-1_000_021.0, -2_000_003.0)] {
                let a = pixels(kind, scale, origin, true);
                let b = pixels(kind, scale, origin, false);
                let error: u64 = a
                    .iter()
                    .zip(&b)
                    .map(|(x, y)| u64::from(x.abs_diff(*y)))
                    .sum();
                // Subpixel edges may rasterize differently in repeated recordings.
                assert!(
                    error as f64 / (a.len() as f64) < 3.0,
                    "{kind:?} {scale} {origin:?}: mean channel error {} first tile {:?}, direct {:?}",
                    error as f64 / a.len() as f64,
                    &a[..4],
                    &b[..4]
                );
            }
        }
    }
}

#[test]
fn board_grid_paint_preserves_incoming_path_and_cairo_state() {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 80, 80).unwrap();
    let ctx = Context::new(&surface).unwrap();
    ctx.move_to(3.0, 7.0);
    ctx.line_to(55.0, 66.0);
    let before = format!("{:?}", ctx.copy_path().unwrap().iter().collect::<Vec<_>>());
    ctx.set_line_width(9.0);
    let matrix = ctx.matrix();
    let grid = BoardGrid::new(BoardGridKind::Isometric, 40);
    paint_geometry(
        &ctx,
        crate::draw::WHITE,
        grid,
        1.0,
        Rectangle::new(0.0, 0.0, 80.0, 80.0),
    )
    .unwrap();
    BoardPaper::for_context(crate::draw::WHITE, grid, &ctx)
        .unwrap()
        .paint(&ctx)
        .unwrap();
    assert_eq!(
        before,
        format!("{:?}", ctx.copy_path().unwrap().iter().collect::<Vec<_>>())
    );
    assert_eq!(ctx.line_width(), 9.0);
    assert_eq!(ctx.matrix(), matrix);
}

#[test]
fn board_grid_eraser_replays_paper_instead_of_removing_lines_or_dots() {
    for kind in BoardGridKind::ALL.into_iter().skip(1) {
        for brush_kind in [EraserKind::Circle, EraserKind::Rect] {
            let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();
            let ctx = Context::new(&surface).unwrap();
            ctx.translate(9.0, 7.0);
            let paper = BoardPaper::for_context(crate::draw::WHITE, BoardGrid::new(kind, 20), &ctx)
                .unwrap();
            paper.paint(&ctx).unwrap();
            let original = {
                let mut copy =
                    cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();
                let c = Context::new(&copy).unwrap();
                c.set_source_surface(&surface, 0.0, 0.0).unwrap();
                c.paint().unwrap();
                drop(c);
                copy.data().unwrap().to_vec()
            };
            ctx.set_source_rgb(1.0, 0.0, 0.0);
            ctx.rectangle(20.0, 20.0, 20.0, 20.0);
            ctx.fill().unwrap();
            let replay = EraserReplayContext {
                pattern: Some(paper.pattern()),
                surface: None,
                backdrop_cache_key: None,
                bg_color: Some(crate::draw::WHITE),
                logical_to_image_scale_x: 1.0,
                logical_to_image_scale_y: 1.0,
                logical_image_origin_x: 0.0,
                logical_image_origin_y: 0.0,
            };
            super::super::strokes::render_eraser_stroke(
                &ctx,
                &[(10, 30), (50, 30)],
                &EraserBrush {
                    kind: brush_kind,
                    size: 48.0,
                },
                &replay,
            );
            drop(ctx);
            let stride = surface.stride() as usize;
            let data = surface.data().unwrap();
            for y in 27..46 {
                for x in 29..48 {
                    let i = y * stride + x * 4;
                    assert!(
                        data[i..i + 4]
                            .iter()
                            .zip(&original[i..i + 4])
                            .all(|(a, b)| a.abs_diff(*b) <= 2),
                        "{kind:?} {brush_kind:?} at {x},{y}"
                    );
                }
            }
        }
    }
}

#[test]
fn board_grid_spacing_and_isometric_basis_are_stable() {
    let g = BoardGrid::new(BoardGridKind::Isometric, i64::MIN);
    assert_eq!(g.spacing(), 8);
    assert_eq!(BoardGrid::new(g.kind, i64::MAX).spacing(), 200);
    assert_eq!(g.disabled().spacing(), 8);
    let (width, height) = tile_size(g);
    assert!((width.hypot(height) / 2.0 - f64::from(g.spacing())).abs() < 1e-10);
    assert_eq!(indices(-41.0, -1.0, 20.0), -3..=0);
}
