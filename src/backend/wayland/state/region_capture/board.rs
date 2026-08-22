use crate::util::Rect;

use super::super::screen_image::ScreenSourceToken;

pub(in crate::backend::wayland) fn world_rect_for_screen_rect(
    display: Rect,
    board_offset: (f64, f64),
    source: ScreenSourceToken,
) -> Option<Rect> {
    if !board_offset.0.is_finite()
        || !board_offset.1.is_finite()
        || (source.zoom_transformed && (!source.zoom_scale.is_finite() || source.zoom_scale <= 0.0))
    {
        return None;
    }
    let map = |x: f64, y: f64| {
        if source.zoom_transformed {
            (
                board_offset.0 + source.zoom_view_offset.0 + x / source.zoom_scale,
                board_offset.1 + source.zoom_view_offset.1 + y / source.zoom_scale,
            )
        } else {
            (board_offset.0 + x, board_offset.1 + y)
        }
    };
    let first = map(f64::from(display.x), f64::from(display.y));
    let second = map(
        f64::from(display.x.saturating_add(display.width)),
        f64::from(display.y.saturating_add(display.height)),
    );
    let left = first.0.min(second.0).floor() as i32;
    let top = first.1.min(second.1).floor() as i32;
    let right = first.0.max(second.0).ceil() as i32;
    let bottom = first.1.max(second.1).ceil() as i32;
    Rect::from_min_max(left, top, right, bottom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::state::screen_image::ScreenImageKind;

    fn source(zoom_transformed: bool) -> ScreenSourceToken {
        ScreenSourceToken {
            output_id: 1,
            output_layout_generation: 2,
            kind: ScreenImageKind::Frozen,
            image_generation: 3,
            image_size: (800, 600),
            stride: 3200,
            surface: (800, 600),
            output_scale: 1,
            output_transform: wayland_client::protocol::wl_output::Transform::Normal,
            zoom_transformed,
            zoom_scale: if zoom_transformed { 2.0 } else { 1.0 },
            zoom_view_offset: if zoom_transformed {
                (100.0, 50.0)
            } else {
                (0.0, 0.0)
            },
        }
    }

    #[test]
    fn identity_world_bounds_equal_the_display_bounds() {
        let display = Rect::new(20, 30, 80, 40).unwrap();
        assert_eq!(
            world_rect_for_screen_rect(display, (0.0, 0.0), source(false)),
            Some(display)
        );
    }

    #[test]
    fn world_bounds_include_board_pan_and_captured_zoom_view() {
        let display = Rect::new(20, 30, 80, 40).unwrap();
        assert_eq!(
            world_rect_for_screen_rect(display, (7.0, 11.0), source(true)),
            Rect::new(117, 76, 40, 20)
        );
        assert_ne!(
            world_rect_for_screen_rect(display, (7.0, 11.0), source(true)),
            Some(display)
        );
    }
}
