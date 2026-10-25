use crate::draw::{Shape, ShapeId, TextMeasurer};
use crate::input::InputState;
use crate::util::Rect;

const TEXT_RESIZE_HANDLE_SIZE: i32 = 10;
const TEXT_RESIZE_HANDLE_OFFSET: i32 = 6;

impl InputState {
    /// Returns no handle when it would end past `i32::MAX`, which text
    /// placed at the coordinate limit can reach.
    fn text_resize_handle_rect(bounds: Rect) -> Option<Rect> {
        let size = i64::from(TEXT_RESIZE_HANDLE_SIZE);
        let inset = i64::from(TEXT_RESIZE_HANDLE_OFFSET) - size / 2;
        let x = i64::from(bounds.x) + i64::from(bounds.width) + inset;
        let y = i64::from(bounds.y) + i64::from(bounds.height) + inset;
        let limit = i64::from(i32::MAX);
        if x + size > limit || y + size > limit {
            return None;
        }

        Rect::new(
            i32::try_from(x).ok()?,
            i32::try_from(y).ok()?,
            TEXT_RESIZE_HANDLE_SIZE,
            TEXT_RESIZE_HANDLE_SIZE,
        )
    }

    pub(crate) fn selected_text_resize_handle_with(
        &self,
        measurer: &TextMeasurer,
    ) -> Option<(ShapeId, Rect)> {
        if self.selected_shape_ids().len() != 1 {
            return None;
        }
        let shape_id = self.selected_shape_ids()[0];
        let frame = self.boards.active_frame();
        let shape = frame.shape(shape_id)?;
        if shape.locked {
            return None;
        }
        if !matches!(shape.shape, Shape::Text { .. } | Shape::StickyNote { .. }) {
            return None;
        }
        let bounds = shape.bounding_box_with(measurer)?;
        let handle = Self::text_resize_handle_rect(bounds)?;
        Some((shape_id, handle))
    }

    pub(crate) fn hit_text_resize_handle_with(
        &self,
        measurer: &TextMeasurer,
        x: i32,
        y: i32,
    ) -> Option<ShapeId> {
        let (shape_id, handle) = self.selected_text_resize_handle_with(measurer)?;
        let tolerance = self.hit_test_tolerance().ceil() as i32;
        let hit_rect = handle.inflated(tolerance).unwrap_or(handle);
        if hit_rect.contains(x, y) {
            Some(shape_id)
        } else {
            None
        }
    }
}
