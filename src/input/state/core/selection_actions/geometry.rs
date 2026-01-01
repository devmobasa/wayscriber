use super::super::base::InputState;
use crate::draw::ShapeId;
use crate::util::Rect;

fn selection_rect(start_x: i32, start_y: i32, end_x: i32, end_y: i32) -> Option<Rect> {
    let min_x = start_x.min(end_x);
    let min_y = start_y.min(end_y);
    let max_x = start_x.max(end_x);
    let max_y = start_y.max(end_y);
    Rect::from_min_max(min_x, min_y, max_x, max_y)
}

fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

impl InputState {
    pub(crate) fn selection_rect_from_points(
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    ) -> Option<Rect> {
        selection_rect(start_x, start_y, end_x, end_y)
    }

    pub(crate) fn shape_ids_in_rect(&self, rect: Rect) -> Vec<ShapeId> {
        let frame = self.canvas_set.active_frame();
        frame
            .shapes
            .iter()
            .filter_map(|shape| {
                shape
                    .shape
                    .bounding_box()
                    .and_then(|bounds| rects_intersect(rect, bounds).then_some(shape.id))
            })
            .collect()
    }
}
