mod clipboard;

pub(crate) use clipboard::{LocalSelectionContext, SelectionClipboard};

use super::base::{InputState, PolygonClickState, SelectionAxis};
use crate::draw::ShapeId;
use crate::util::Rect;
use std::collections::HashSet;
use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub enum SelectionState {
    #[default]
    None,
    Active {
        shape_ids: Vec<ShapeId>,
        /// Cached HashSet for O(1) membership tests during rendering.
        shape_ids_set: HashSet<ShapeId>,
    },
}

/// Selection membership plus interaction memory shared by selection and polygon input.
#[derive(Debug, Clone, Default)]
pub(crate) struct SelectionInteraction {
    state: SelectionState,
    last_axis: Option<SelectionAxis>,
    last_polygon_click: Option<PolygonClickState>,
}

impl SelectionInteraction {
    pub(crate) fn selected_shape_ids(&self) -> &[ShapeId] {
        match &self.state {
            SelectionState::Active { shape_ids, .. } => shape_ids,
            SelectionState::None => &[],
        }
    }

    pub(crate) fn selected_shape_ids_set(&self) -> Option<&HashSet<ShapeId>> {
        match &self.state {
            SelectionState::Active { shape_ids_set, .. } => Some(shape_ids_set),
            SelectionState::None => None,
        }
    }

    pub(crate) fn has_selection(&self) -> bool {
        matches!(self.state, SelectionState::Active { .. })
    }

    pub(crate) fn clear(&mut self) {
        self.state = SelectionState::None;
        self.last_axis = None;
    }

    pub(crate) fn set(&mut self, ids: Vec<ShapeId>) {
        if ids.is_empty() {
            self.clear();
            return;
        }

        let mut seen = HashSet::with_capacity(ids.len());
        let mut ordered = Vec::with_capacity(ids.len());
        for id in ids {
            if seen.insert(id) {
                ordered.push(id);
            }
        }
        self.state = SelectionState::Active {
            shape_ids: ordered,
            shape_ids_set: seen,
        };
        self.last_axis = None;
    }

    pub(crate) fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = ShapeId>,
    {
        match &mut self.state {
            SelectionState::Active {
                shape_ids,
                shape_ids_set,
            } => {
                for id in iter {
                    if shape_ids_set.insert(id) {
                        shape_ids.push(id);
                    }
                }
                self.last_axis = None;
            }
            SelectionState::None => self.set(iter.into_iter().collect()),
        }
    }

    pub(crate) fn note_axis(&mut self, axis: SelectionAxis) {
        self.last_axis = Some(axis);
    }

    pub(crate) fn record_polygon_click(&mut self, x: i32, y: i32, at: Instant) {
        self.last_polygon_click = Some(PolygonClickState { x, y, at });
    }

    pub(crate) fn polygon_click_completes(
        &self,
        x: i32,
        y: i32,
        now: Instant,
        max_elapsed_ms: u64,
        max_distance: i32,
        has_minimum_points: bool,
    ) -> bool {
        let Some(last) = self.last_polygon_click else {
            return false;
        };
        has_minimum_points
            && now.duration_since(last.at).as_millis() <= max_elapsed_ms as u128
            && (x - last.x).abs() <= max_distance
            && (y - last.y).abs() <= max_distance
    }

    pub(crate) fn clear_polygon_click(&mut self) {
        self.last_polygon_click = None;
    }

    pub(crate) fn polygon_click(&self) -> Option<PolygonClickState> {
        self.last_polygon_click
    }

    pub(crate) fn restore_polygon_click(&mut self, click: Option<PolygonClickState>) {
        self.last_polygon_click = click;
    }
}

impl InputState {
    pub fn selected_shape_ids(&self) -> &[ShapeId] {
        self.selection_interaction.selected_shape_ids()
    }

    /// Returns a reference to the cached HashSet of selected shape IDs.
    /// Use this for O(1) membership tests instead of creating a new HashSet.
    pub fn selected_shape_ids_set(&self) -> Option<&HashSet<ShapeId>> {
        self.selection_interaction.selected_shape_ids_set()
    }

    pub fn has_selection(&self) -> bool {
        self.selection_interaction.has_selection()
    }

    pub fn clear_selection(&mut self) {
        self.selection_interaction.clear();
        self.close_properties_panel();
    }

    pub fn set_selection(&mut self, ids: Vec<ShapeId>) {
        self.selection_interaction.set(ids);
        self.close_properties_panel();
    }

    pub fn extend_selection<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = ShapeId>,
    {
        self.selection_interaction.extend(iter);
        self.close_properties_panel();
    }

    pub(crate) fn selection_bounding_box(&self, ids: &[ShapeId]) -> Option<Rect> {
        let frame = self.boards.active_frame();
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut found = false;

        for id in ids {
            if let Some(shape) = frame.shape(*id)
                && let Some(bounds) = shape.bounding_box()
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

    pub(crate) fn selection_screen_bounding_box(&self, ids: &[ShapeId]) -> Option<Rect> {
        self.selection_bounding_box(ids)
            .and_then(|bounds| self.screen_rect_for_canvas(bounds))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn selection_set_and_extend_preserve_order_without_duplicates_and_reset_axis() {
        let mut selection = SelectionInteraction::default();
        selection.set(vec![3, 1, 3]);
        selection.note_axis(SelectionAxis::Horizontal);
        selection.extend([1, 2, 3, 4]);

        assert_eq!(selection.selected_shape_ids(), [3, 1, 2, 4]);
        assert_eq!(
            selection.selected_shape_ids_set(),
            Some(&HashSet::from([1, 2, 3, 4]))
        );
        assert_eq!(selection.last_axis, None);

        selection.clear();
        assert!(!selection.has_selection());
        assert!(selection.selected_shape_ids_set().is_none());
    }

    #[test]
    fn polygon_click_requires_time_distance_and_minimum_points() {
        let mut selection = SelectionInteraction::default();
        let first = Instant::now();
        selection.record_polygon_click(20, 30, first);

        assert!(!selection.polygon_click_completes(21, 31, first, 400, 6, false));
        assert!(!selection.polygon_click_completes(27, 31, first, 400, 6, true));
        assert!(!selection.polygon_click_completes(
            21,
            31,
            first + Duration::from_millis(401),
            400,
            6,
            true,
        ));
        assert!(selection.polygon_click_completes(
            26,
            36,
            first + Duration::from_millis(400),
            400,
            6,
            true,
        ));
    }
}
