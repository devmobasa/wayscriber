mod clipboard;

pub(crate) use clipboard::LocalSelectionContext;
pub(in crate::input::state::core) use clipboard::SelectionClipboard;

use super::base::{InputState, SelectionAxis};
use crate::draw::{ShapeId, TextMeasurer};
use crate::util::Rect;
use std::collections::HashSet;
use std::time::Instant;

#[derive(Debug, Clone, Default)]
enum SelectionState {
    #[default]
    None,
    Active {
        shape_ids: Vec<ShapeId>,
        /// Cached HashSet for O(1) membership tests during rendering.
        shape_ids_set: HashSet<ShapeId>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(in crate::input::state::core) struct PolygonClickState {
    x: i32,
    y: i32,
    at: Instant,
}

/// Selection membership plus interaction memory shared by selection and polygon input.
#[derive(Debug, Clone, Default)]
pub(in crate::input::state) struct SelectionInteraction {
    state: SelectionState,
    last_axis: Option<SelectionAxis>,
    last_polygon_click: Option<PolygonClickState>,
}

impl SelectionInteraction {
    pub(in crate::input::state) fn selected_shape_ids(&self) -> &[ShapeId] {
        match &self.state {
            SelectionState::Active { shape_ids, .. } => shape_ids,
            SelectionState::None => &[],
        }
    }

    pub(in crate::input::state) fn selected_shape_ids_set(&self) -> Option<&HashSet<ShapeId>> {
        match &self.state {
            SelectionState::Active { shape_ids_set, .. } => Some(shape_ids_set),
            SelectionState::None => None,
        }
    }

    pub(in crate::input::state) fn has_selection(&self) -> bool {
        matches!(self.state, SelectionState::Active { .. })
    }

    pub(in crate::input::state) fn clear(&mut self) {
        self.state = SelectionState::None;
        self.last_axis = None;
    }

    pub(in crate::input::state) fn set(&mut self, ids: Vec<ShapeId>) {
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

    pub(in crate::input::state) fn extend<I>(&mut self, iter: I)
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

    pub(in crate::input::state) fn note_axis(&mut self, axis: SelectionAxis) {
        self.last_axis = Some(axis);
    }

    pub(in crate::input::state) fn record_polygon_click(&mut self, x: i32, y: i32, at: Instant) {
        self.last_polygon_click = Some(PolygonClickState { x, y, at });
    }

    pub(in crate::input::state) fn polygon_click_completes(
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

    pub(in crate::input::state) fn clear_polygon_click(&mut self) {
        self.last_polygon_click = None;
    }

    pub(in crate::input::state::core) fn polygon_click(&self) -> Option<PolygonClickState> {
        self.last_polygon_click
    }

    pub(in crate::input::state::core) fn restore_polygon_click(
        &mut self,
        click: Option<PolygonClickState>,
    ) {
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
        let measurer = TextMeasurer::default();
        self.clear_selection_with(&measurer);
    }

    pub(crate) fn clear_selection_with(&mut self, measurer: &TextMeasurer) {
        self.change_selection_with(measurer, SelectionInteraction::clear);
    }

    pub fn set_selection(&mut self, ids: Vec<ShapeId>) {
        let measurer = TextMeasurer::default();
        self.set_selection_with(&measurer, ids);
    }

    pub(crate) fn set_selection_with(&mut self, measurer: &TextMeasurer, ids: Vec<ShapeId>) {
        self.change_selection_with(measurer, |selection| selection.set(ids));
    }

    pub fn extend_selection<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = ShapeId>,
    {
        let measurer = TextMeasurer::default();
        self.extend_selection_with(&measurer, iter);
    }

    pub(crate) fn extend_selection_with<I>(&mut self, measurer: &TextMeasurer, iter: I)
    where
        I: IntoIterator<Item = ShapeId>,
    {
        self.change_selection_with(measurer, |selection| selection.extend(iter));
    }

    /// Applies one membership change and repaints the selection chrome both
    /// where it was drawn and where it now belongs.
    ///
    /// Every selection change goes through here, so a click, a rubber band, a
    /// deselect, or an undo cannot leave a stale halo behind or show the new
    /// one only after an unrelated repaint.
    fn change_selection_with(
        &mut self,
        measurer: &TextMeasurer,
        change: impl FnOnce(&mut SelectionInteraction),
    ) {
        let previous_ids = self.selected_shape_ids().to_vec();
        let previous_chrome = self.selection_chrome_bounds_with(measurer);

        change(&mut self.selection_interaction);
        self.close_properties_panel();

        if self.selected_shape_ids() != previous_ids.as_slice() {
            self.mark_selection_dirty_region(previous_chrome);
            self.mark_selection_chrome_dirty_with(measurer);
            self.needs_redraw = true;
        }
    }

    /// Canvas bounds of everything painted for the selection: the halos, the
    /// dashed handle frame, and the resize handle of a lone text shape, which
    /// sits outside the shape.
    pub(crate) fn selection_chrome_bounds_with(&self, measurer: &TextMeasurer) -> Option<Rect> {
        let bounds = self.selection_bounds_with(measurer)?;
        match self.selected_text_resize_handle_with(measurer) {
            Some((_, handle)) => bounds.union(handle),
            None => Some(bounds),
        }
    }

    /// Repaints the current selection chrome. Also used when an interaction
    /// ends and the handles it hid reappear without a membership change.
    pub(crate) fn mark_selection_chrome_dirty_with(&mut self, measurer: &TextMeasurer) {
        let chrome = self.selection_chrome_bounds_with(measurer);
        self.mark_selection_dirty_region(chrome);
    }

    pub(crate) fn selection_bounding_box_with(
        &self,
        measurer: &TextMeasurer,
        ids: &[ShapeId],
    ) -> Option<Rect> {
        let frame = self.boards.active_frame();
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut found = false;

        for id in ids {
            if let Some(shape) = frame.shape(*id)
                && let Some(bounds) = shape.bounding_box_with(measurer)
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

    pub(crate) fn selection_screen_bounding_box_with(
        &self,
        measurer: &TextMeasurer,
        ids: &[ShapeId],
    ) -> Option<Rect> {
        self.selection_bounding_box_with(measurer, ids)
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
