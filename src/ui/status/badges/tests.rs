use super::*;

fn paint(engine: &UiTextEngine, kind: usize, density: i32, standalone: bool) -> (Vec<u8>, f64) {
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, 640 * density, 480 * density).unwrap();
    let height;
    {
        let ctx = cairo::Context::new(&surface).unwrap();
        ctx.scale(f64::from(density), f64::from(density));
        height = match (kind, standalone) {
            (0, false) => {
                render_frozen_badge_with_engine(engine, &ctx, 640, 480);
                0.0
            }
            (0, true) => {
                render_frozen_badge(&ctx, 640, 480);
                0.0
            }
            (1, false) => render_zoom_badge_with_engine(engine, &ctx, 640, 480, 2.25, true),
            (1, true) => render_zoom_badge(&ctx, 640, 480, 2.25, true),
            (2, false) => render_pan_badge_with_engine(engine, &ctx, 640, 480, true, 35.0),
            (2, true) => render_pan_badge(&ctx, 640, 480, true, 35.0),
            (3, false) => {
                render_editing_badge_with_engine(engine, &ctx, 640, 480, 70.0);
                0.0
            }
            (3, true) => {
                render_editing_badge(&ctx, 640, 480, 70.0);
                0.0
            }
            (4, false) => {
                render_page_badge_with_engine(
                    engine,
                    &ctx,
                    640,
                    480,
                    1,
                    3,
                    "日本語 long board name beyond cutoff",
                    2,
                    5,
                );
                0.0
            }
            (4, true) => {
                render_page_badge(
                    &ctx,
                    640,
                    480,
                    1,
                    3,
                    "日本語 long board name beyond cutoff",
                    2,
                    5,
                );
                0.0
            }
            _ => unreachable!(),
        };
    }
    surface.flush();
    (surface.data().unwrap().to_vec(), height)
}

#[test]
fn fallback_badges_reuse_one_owner_and_preserve_pixels_and_stacking() {
    let engine = UiTextEngine::default();
    for density in [1, 2, 1] {
        for kind in 0..5 {
            let (retained, height) = paint(&engine, kind, density, false);
            let (fresh, fresh_height) = paint(&UiTextEngine::default(), kind, density, false);
            let (standalone, standalone_height) = paint(&engine, kind, density, true);
            assert!(
                retained == fresh && retained == standalone,
                "badge {kind} differs at density {density}"
            );
            assert!(retained.iter().any(|byte| *byte != 0));
            assert_eq!(height, fresh_height);
            assert_eq!(height, standalone_height);
            if matches!(kind, 1 | 2) {
                assert!(height > BADGE_STACK_GAP);
            }
        }
    }
}
