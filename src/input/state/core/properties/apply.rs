use super::super::base::InputState;
use super::RecolorOpacity;
use super::types::{
    PanelAction, PropertiesPanelHit, PropertiesPanelLock, SelectionPropertyEntry,
    SelectionPropertyKind, SelectionPropertyValue,
};
use crate::draw::{ArrowStyle, Color, Shape, TextMeasurer};

impl InputState {
    /// Acts on the panel control `hit`: a swatch sets that color, a stepper
    /// button steps its property, and so on. Returns whether anything changed
    /// (for the "More colors" button: whether the color picker opened).
    pub(crate) fn activate_properties_panel_hit_with(
        &mut self,
        measurer: &TextMeasurer,
        hit: PropertiesPanelHit,
    ) -> bool {
        if hit == PropertiesPanelHit::Lock {
            return self.toggle_properties_panel_lock_with(measurer);
        }
        if let PropertiesPanelHit::Action(action) = hit {
            return self.run_properties_panel_action_with(measurer, action);
        }
        let Some(entry) = hit.row().and_then(|row| self.enabled_properties_entry(row)) else {
            return false;
        };

        let changed = match hit {
            // A slider acts on the press and the drag, not on the click.
            PropertiesPanelHit::Title
            | PropertiesPanelHit::Lock
            | PropertiesPanelHit::Action(_)
            | PropertiesPanelHit::Slider(_) => false,
            PropertiesPanelHit::Swatch { index, .. } => {
                let Some(color) = self
                    .properties
                    .panel
                    .as_ref()
                    .and_then(|panel| panel.swatches.get(index))
                    .map(|swatch| swatch.color)
                else {
                    return false;
                };
                self.set_selection_color_from_panel(measurer, color)
            }
            PropertiesPanelHit::MoreColors(_) => {
                return self.open_color_picker_popup_for_selection_with_measurer(measurer);
            }
            PropertiesPanelHit::StepDown(_) => {
                self.dispatch_selection_property(measurer, entry.kind, -1)
            }
            PropertiesPanelHit::StepUp(_) => {
                self.dispatch_selection_property(measurer, entry.kind, 1)
            }
            PropertiesPanelHit::Toggle(_) => {
                self.dispatch_selection_property(measurer, entry.kind, 0)
            }
            PropertiesPanelHit::ArrowHead { at_end, .. } => {
                if entry.state == SelectionPropertyValue::ArrowHead(Some(at_end)) {
                    return false;
                }
                let direction = if at_end { 1 } else { -1 };
                self.dispatch_selection_property(measurer, entry.kind, direction)
            }
            PropertiesPanelHit::ArrowStyle { style, .. } => {
                if entry.state == SelectionPropertyValue::ArrowStyle(Some(style)) {
                    return false;
                }
                self.set_selection_arrow_style_from_panel(measurer, style)
            }
            // A click beside a switch flips it, the way a click on a checkbox
            // label does; every other row only takes focus.
            PropertiesPanelHit::Row(_) => match entry.state {
                SelectionPropertyValue::Toggle(_) => {
                    self.dispatch_selection_property(measurer, entry.kind, 0)
                }
                _ => false,
            },
        };

        if changed {
            self.refresh_properties_panel_with(measurer);
        }
        changed
    }

    /// Steps the row under a wheel tick: up raises a number, turns a switch
    /// on, or moves to the next swatch or style, and down does the opposite.
    pub(crate) fn step_properties_panel_row_by_wheel_with(
        &mut self,
        measurer: &TextMeasurer,
        row: usize,
        scroll_direction: i32,
    ) -> bool {
        if scroll_direction == 0 {
            return false;
        }
        let Some(entry) = self.enabled_properties_entry(row) else {
            return false;
        };

        let changed = self.dispatch_selection_property(measurer, entry.kind, -scroll_direction);

        if changed {
            self.refresh_properties_panel_with(measurer);
        }
        changed
    }

    fn enabled_properties_entry(&self, row: usize) -> Option<SelectionPropertyEntry> {
        self.properties
            .panel
            .as_ref()?
            .entries
            .get(row)
            .filter(|entry| !entry.disabled)
            .cloned()
    }

    fn set_selection_color_from_panel(&mut self, measurer: &TextMeasurer, color: Color) -> bool {
        // Picking the color the selection already has is a no-op, not a
        // "No changes applied" toast. A swatch with the same hue still
        // changes a shape of another opacity.
        if !self.selection_recolor_changes(color, RecolorOpacity::Swatch) {
            return false;
        }
        self.finish_active_arrow_bend();
        self.apply_selection_color_value_with(measurer, color)
    }

    fn set_selection_arrow_style_from_panel(
        &mut self,
        measurer: &TextMeasurer,
        style: ArrowStyle,
    ) -> bool {
        // See `dispatch_selection_property`: a restyle must end a live bend
        // drag before it records its own undo entry.
        self.finish_active_arrow_bend();
        self.apply_selection_arrow_style_value(measurer, style)
    }

    /// Runs an actions-area button through the same selection edits as the
    /// context menu and the keyboard. A delete empties the selection, and the
    /// refresh then closes the panel with nothing left to show.
    fn run_properties_panel_action_with(
        &mut self,
        measurer: &TextMeasurer,
        action: PanelAction,
    ) -> bool {
        let enabled = self
            .properties
            .panel
            .as_ref()
            .is_some_and(|panel| panel.actions.enabled(action));
        if !enabled {
            return false;
        }

        let changed = match action {
            PanelAction::ToBack => self.move_selection_to_back_with(measurer),
            PanelAction::Backward => self.move_selection_backward_with(measurer),
            PanelAction::Forward => self.move_selection_forward_with(measurer),
            PanelAction::ToFront => self.move_selection_to_front_with(measurer),
            PanelAction::Duplicate => {
                // Duplicating selects the copies, and any selection change
                // closes the panel; reopen it on the copies it was asked for.
                let duplicated = self.duplicate_selection_with(measurer);
                if duplicated && !self.is_properties_panel_open() {
                    let _ = self.show_properties_panel_with(measurer);
                }
                duplicated
            }
            PanelAction::Delete => self.delete_selection_with(measurer),
        };

        if changed && self.is_properties_panel_open() {
            self.refresh_properties_panel_with(measurer);
        }
        changed
    }

    /// Locks every selected shape, or unlocks them all once every one is
    /// locked. Locked shapes refuse edits, so this is also how the panel's
    /// disabled rows come back.
    fn toggle_properties_panel_lock_with(&mut self, measurer: &TextMeasurer) -> bool {
        let Some(lock) = self.properties.panel.as_ref().map(|panel| panel.lock) else {
            return false;
        };
        let lock_all = lock != PropertiesPanelLock::Locked;

        let changed = self.set_selection_locked_with(measurer, lock_all);

        if changed {
            self.refresh_properties_panel_with(measurer);
        }
        changed
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
            SelectionPropertyKind::Opacity => {
                self.apply_selection_opacity(measurer, direction_or_default(direction))
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
    // Treat activation (0) as a forward step. The magnitude is kept, so a
    // coarse keyboard step moves a number several steps at once.
    if direction == 0 { 1 } else { direction }
}
