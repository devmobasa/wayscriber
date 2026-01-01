use crate::draw::{Shape, ShapeId};
use crate::input::InputState;
use crate::util::Rect;

const TEXT_RESIZE_HANDLE_SIZE: i32 = 10;
const TEXT_RESIZE_HANDLE_OFFSET: i32 = 6;

impl InputState {
    fn text_resize_handle_rect(bounds: Rect) -> Option<Rect> {
        let size = TEXT_RESIZE_HANDLE_SIZE;
        let half = size / 2;
        let center_x = bounds.x + bounds.width + TEXT_RESIZE_HANDLE_OFFSET;
        let center_y = bounds.y + bounds.height + TEXT_RESIZE_HANDLE_OFFSET;
        Rect::new(center_x - half, center_y - half, size, size)
    }

    pub(crate) fn selected_text_resize_handle(&self) -> Option<(ShapeId, Rect)> {
        if self.selected_shape_ids().len() != 1 {
            return None;
        }
        let shape_id = self.selected_shape_ids()[0];
        let frame = self.canvas_set.active_frame();
        let shape = frame.shape(shape_id)?;
        if shape.locked {
            return None;
        }
        if !matches!(shape.shape, Shape::Text { .. } | Shape::StickyNote { .. }) {
            return None;
        }
        let bounds = shape.shape.bounding_box()?;
        let handle = Self::text_resize_handle_rect(bounds)?;
        Some((shape_id, handle))
    }

    pub(crate) fn hit_text_resize_handle(&self, x: i32, y: i32) -> Option<ShapeId> {
        let (shape_id, handle) = self.selected_text_resize_handle()?;
        let tolerance = self.hit_test_tolerance.ceil() as i32;
        let hit_rect = handle.inflated(tolerance).unwrap_or(handle);
        if hit_rect.contains(x, y) {
            Some(shape_id)
        } else {
            None
        }
    }
}
