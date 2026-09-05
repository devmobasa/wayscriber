use super::super::base::InputState;
use super::types::SelectionPropertyKind;
use crate::draw::{Shape, TextMeasurer, with_legacy_measurer};

impl InputState {
    pub(crate) fn activate_properties_panel_entry(&mut self) -> bool {
        with_legacy_measurer(|measurer| self.activate_properties_panel_entry_with(measurer))
    }

    pub(crate) fn activate_properties_panel_entry_with(&mut self, measurer: &TextMeasurer) -> bool {
        self.adjust_properties_panel_entry_with(measurer, 0)
    }

    pub(crate) fn adjust_properties_panel_entry_with(
        &mut self,
        measurer: &TextMeasurer,
        direction: i32,
    ) -> bool {
        let index = self.current_properties_focus_or_hover();
        let Some(index) = index else {
            return false;
        };

        self.apply_properties_entry(measurer, index, direction)
    }

    fn apply_properties_entry(
        &mut self,
        measurer: &TextMeasurer,
        index: usize,
        direction: i32,
    ) -> bool {
        let entry = {
            let Some(panel) = self.properties.panel.as_ref() else {
                return false;
            };
            let Some(entry) = panel.entries.get(index) else {
                return false;
            };
            if entry.disabled {
                return false;
            }
            entry.clone()
        };

        let changed = self.dispatch_selection_property(measurer, entry.kind, direction);

        if changed {
            self.refresh_properties_panel_with(measurer);
        }

        changed
    }

    /// Whether the current selection holds at least one arrow.
    ///
    /// The arrow-style cycle needs this before it routes: a selection with no
    /// arrows in it should step the next-arrow default rather than silently do
    /// nothing.
    pub(crate) fn selection_contains_arrow(&self) -> bool {
        let frame = self.boards.active_frame();
        self.selected_shape_ids()
            .iter()
            .filter_map(|id| frame.shape(*id))
            .any(|drawn| matches!(drawn.shape, Shape::Arrow { .. }))
    }

    /// Style-pill path into the same apply machinery as the properties
    /// popup: adjusts the selection property of `kind` when the current
    /// selection exposes it and the entry is editable. Refreshes the
    /// popup if it happens to be open.
    pub(crate) fn adjust_selection_property_kind_with(
        &mut self,
        measurer: &TextMeasurer,
        kind: SelectionPropertyKind,
        direction: i32,
    ) -> bool {
        let ids = self.selected_shape_ids();
        if ids.is_empty() {
            return false;
        }
        let entries = self.build_selection_property_entries(ids);
        let Some(entry) = entries.into_iter().find(|entry| entry.kind == kind) else {
            return false;
        };
        if entry.disabled {
            return false;
        }

        let changed = self.dispatch_selection_property(measurer, kind, direction);

        if changed && self.is_properties_panel_open() {
            self.refresh_properties_panel_with(measurer);
        }

        changed
    }

    /// Arrow-style action path, whose command remains meaningful when every
    /// selected arrow is locked. Visible property controls stay disabled, while
    /// the action still reaches the shared apply reporter so it can explain why
    /// nothing changed.
    pub(crate) fn cycle_selected_arrow_style_from_action_with(
        &mut self,
        measurer: &TextMeasurer,
    ) -> bool {
        let changed =
            self.dispatch_selection_property(measurer, SelectionPropertyKind::ArrowStyle, 1);

        if changed && self.is_properties_panel_open() {
            self.refresh_properties_panel_with(measurer);
        }

        changed
    }

    fn dispatch_selection_property(
        &mut self,
        measurer: &TextMeasurer,
        kind: SelectionPropertyKind,
        direction: i32,
    ) -> bool {
        // Every property route lands here — the keyboard action, the toolbar's
        // AdjustSelectionProperty, and the shape properties panel — so this is
        // the one place that has to end a live bend drag first. That drag holds
        // a snapshot from before it started; a property change pushed on top of
        // it records one undo entry now, and the eventual release records a
        // second measured from the same stale snapshot, so undoing walks back
        // through a shape that was never on screen (and reverts the property
        // change along the way). Restyling is the case that bites hardest,
        // because leaving Curved hides the arc the drag is editing.
        self.finish_active_arrow_bend();
        match kind {
            SelectionPropertyKind::Color => self.apply_selection_color(measurer, direction),
            SelectionPropertyKind::Thickness => {
                self.apply_selection_thickness(measurer, direction_or_default(direction))
            }
            SelectionPropertyKind::Fill => self.apply_selection_fill(measurer, direction),
            SelectionPropertyKind::FontSize => {
                self.apply_selection_font_size(measurer, direction_or_default(direction))
            }
            SelectionPropertyKind::ArrowHead => {
                self.apply_selection_arrow_head(measurer, direction)
            }
            SelectionPropertyKind::ArrowStyle => {
                self.apply_selection_arrow_style(measurer, direction)
            }
            SelectionPropertyKind::ArrowLength => {
                self.apply_selection_arrow_length(measurer, direction_or_default(direction))
            }
            SelectionPropertyKind::ArrowAngle => {
                self.apply_selection_arrow_angle(measurer, direction_or_default(direction))
            }
            SelectionPropertyKind::TextBackground => {
                self.apply_selection_text_background(measurer, direction)
            }
            SelectionPropertyKind::SpotlightMagnification => self
                .apply_selection_spotlight_magnification(measurer, direction_or_default(direction)),
        }
    }
}

fn direction_or_default(direction: i32) -> i32 {
    // Treat activation (0) as a forward step; preserve negative direction.
    if direction < 0 { -1 } else { 1 }
}
