use crate::util::Rect;
use crate::{canvas_export::CanvasExportRect, screen_pixels::ImagePixelRect};

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

/// Map authoritative source-image pixel edges directly into board-world space.
///
/// This intentionally bypasses the integer display rectangle used by picker
/// chrome. Its floor/ceil quantization can expand a crop at fractional output
/// scales. Zoom does not appear here: selecting an image-space rectangle has
/// already inverted the captured zoom view, so only the board offset and the
/// source image-to-logical-surface ratio remain.
pub(in crate::backend::wayland) fn world_rect_for_image_rect_exact(
    rect: ImagePixelRect,
    board_offset: (f64, f64),
    source: ScreenSourceToken,
) -> Option<CanvasExportRect> {
    if !board_offset.0.is_finite()
        || !board_offset.1.is_finite()
        || source.image_size.0 == 0
        || source.image_size.1 == 0
        || source.surface.0 == 0
        || source.surface.1 == 0
        || rect.x().checked_add(rect.width())? > source.image_size.0
        || rect.y().checked_add(rect.height())? > source.image_size.1
    {
        return None;
    }
    let scale_x = f64::from(source.surface.0) / f64::from(source.image_size.0);
    let scale_y = f64::from(source.surface.1) / f64::from(source.image_size.1);
    CanvasExportRect::new(
        board_offset.0 + f64::from(rect.x()) * scale_x,
        board_offset.1 + f64::from(rect.y()) * scale_y,
        f64::from(rect.width()) * scale_x,
        f64::from(rect.height()) * scale_y,
    )
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

    #[test]
    fn exact_image_mapping_preserves_native_and_fractional_scale_edges() {
        let rect = ImagePixelRect::new(150, 225, 300, 150, (1200, 900)).unwrap();
        let mut token = source(false);
        token.image_size = (1200, 900);
        token.surface = (800, 600);
        token.output_scale = 2;
        assert_eq!(
            world_rect_for_image_rect_exact(rect, (7.0, 11.0), token),
            CanvasExportRect::new(107.0, 161.0, 200.0, 100.0)
        );

        token.image_size = (1600, 1200);
        let rect = ImagePixelRect::new(200, 300, 400, 200, token.image_size).unwrap();
        assert_eq!(
            world_rect_for_image_rect_exact(rect, (7.0, 11.0), token),
            CanvasExportRect::new(107.0, 161.0, 200.0, 100.0)
        );
    }

    #[test]
    fn exact_image_mapping_is_invariant_to_the_captured_zoom_view() {
        let rect = ImagePixelRect::new(20, 30, 80, 40, (800, 600)).unwrap();
        let plain = source(false);
        let zoomed = source(true);
        assert_eq!(
            world_rect_for_image_rect_exact(rect, (7.0, 11.0), plain),
            world_rect_for_image_rect_exact(rect, (7.0, 11.0), zoomed)
        );
        assert_eq!(
            world_rect_for_image_rect_exact(rect, (7.0, 11.0), zoomed),
            CanvasExportRect::new(27.0, 41.0, 80.0, 40.0)
        );
    }
}
