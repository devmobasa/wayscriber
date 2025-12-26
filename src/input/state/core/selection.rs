use super::base::InputState;
use crate::draw::ShapeId;
use crate::util::Rect;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum SelectionState {
    None,
    Active { shape_ids: Vec<ShapeId> },
}

impl InputState {
    pub fn selected_shape_ids(&self) -> &[ShapeId] {
        match &self.selection_state {
            SelectionState::Active { shape_ids } => shape_ids,
            _ => &[],
        }
    }

    pub fn has_selection(&self) -> bool {
        matches!(self.selection_state, SelectionState::Active { .. })
    }

    pub fn clear_selection(&mut self) {
        self.selection_state = SelectionState::None;
        self.last_selection_axis = None;
        self.close_properties_panel();
    }

    pub fn set_selection(&mut self, ids: Vec<ShapeId>) {
        if ids.is_empty() {
            self.selection_state = SelectionState::None;
            self.last_selection_axis = None;
            self.close_properties_panel();
            return;
        }

        let mut seen = HashSet::new();
        let mut ordered = Vec::new();
        for id in ids {
            if seen.insert(id) {
                ordered.push(id);
            }
        }
        self.selection_state = SelectionState::Active { shape_ids: ordered };
        self.last_selection_axis = None;
        self.close_properties_panel();
    }

    pub fn extend_selection<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = ShapeId>,
    {
        match &mut self.selection_state {
            SelectionState::Active { shape_ids } => {
                let mut seen: HashSet<ShapeId> = shape_ids.iter().copied().collect();
                for id in iter {
                    if seen.insert(id) {
                        shape_ids.push(id);
                    }
                }
                self.last_selection_axis = None;
                self.close_properties_panel();
            }
            _ => {
                let ids: Vec<ShapeId> = iter.into_iter().collect();
                self.set_selection(ids);
            }
        }
    }

    pub(crate) fn selection_bounding_box(&self, ids: &[ShapeId]) -> Option<Rect> {
        let frame = self.canvas_set.active_frame();
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut found = false;

        for id in ids {
            if let Some(shape) = frame.shape(*id)
                && let Some(bounds) = shape.shape.bounding_box()
            {
                min_x = min_x.min(bounds.x);
                min_y = min_y.min(bounds.y);
                max_x = max_x.max(bounds.x + bounds.width);
                max_y = max_y.max(bounds.y + bounds.height);
                found = true;
            }
        }

        if found {
            Rect::from_min_max(min_x, min_y, max_x, max_y)
        } else {
            None
        }
    }
}
