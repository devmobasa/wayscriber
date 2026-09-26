use crate::draw::TextMeasurer;
use crate::draw::{Color, FontDescriptor};
use crate::input::state::TopMenuState;
use crate::input::{DrawingState, EraserMode, InputState, Tool};

use crate::ui::toolbar::PrecisionEntryTarget;
use crate::ui::toolbar::model::ToolbarSliderSpec;

impl InputState {
    /// Open the precise-entry popup for a pill numeral.
    pub(super) fn apply_toolbar_open_precision_entry(
        &mut self,
        target: PrecisionEntryTarget,
    ) -> bool {
        self.open_precision_entry(target);
        true
    }

    /// Commit a typed precise-entry value, clamped to the target's shared
    /// slider range, and close the popup if it is still open.
    pub(super) fn apply_toolbar_commit_precision_entry_with(
        &mut self,
        measurer: &TextMeasurer,
        target: PrecisionEntryTarget,
        value: f64,
    ) -> bool {
        let _ = self.cancel_precision_entry();
        if !value.is_finite() {
            return false;
        }
        match target {
            PrecisionEntryTarget::Thickness => {
                let spec = ToolbarSliderSpec::THICKNESS;
                self.apply_toolbar_set_thickness_with(measurer, value.clamp(spec.min, spec.max))
            }
            PrecisionEntryTarget::FontSize => {
                let spec = ToolbarSliderSpec::FONT_SIZE;
                self.apply_toolbar_set_font_size(value.clamp(spec.min, spec.max))
            }
        }
    }

    pub(super) fn apply_toolbar_select_tool_with(
        &mut self,
        measurer: &TextMeasurer,
        tool: Tool,
    ) -> bool {
        if matches!(self.state, DrawingState::TextInput { .. }) {
            self.cancel_text_input_with(measurer);
        }
        let mut changed = if tool == Tool::Highlight {
            let was_highlight_active = self.highlight_tool_active();
            let was_click_highlight_enabled = self.click_highlight_enabled();
            self.set_highlight_tool_with_measurer(measurer, true);
            let override_changed = self.set_tool_override_with(measurer, Some(tool));
            override_changed
                || was_highlight_active != self.highlight_tool_active()
                || was_click_highlight_enabled != self.click_highlight_enabled()
        } else {
            self.set_tool_override_with(measurer, Some(tool))
        };
        if self.toolbar.top_menu_flyout_open() {
            changed |= self.toolbar.close_top_menu();
        }
        changed
    }

    pub(super) fn apply_toolbar_set_color_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        color: Color,
    ) -> bool {
        self.apply_color_from_ui_with_measurer(measurer, color)
    }

    pub(super) fn apply_toolbar_set_thickness_with(
        &mut self,
        measurer: &TextMeasurer,
        value: f64,
    ) -> bool {
        self.set_thickness_for_active_tool_with(measurer, value)
    }

    pub(super) fn apply_toolbar_set_marker_opacity(&mut self, value: f64) -> bool {
        self.set_marker_opacity(value)
    }

    pub(super) fn apply_toolbar_set_spotlight_magnification(&mut self, value: f64) -> bool {
        self.set_spotlight_magnification(value)
    }

    pub(super) fn apply_toolbar_set_pen_smoothing(&mut self, level: u8) -> bool {
        self.set_pen_smoothing(level)
    }

    pub(super) fn apply_toolbar_nudge_stroke_setting(
        &mut self,
        setting: crate::ui::toolbar::model::StrokeSetting,
        steps: i32,
    ) -> bool {
        use crate::ui::toolbar::model::StrokeSetting;

        let level = match setting {
            StrokeSetting::Smoothing => self.style.pen_smoothing,
            StrokeSetting::ShapeDetection => self.style.shape_recognition_sensitivity,
        };
        let Some(target) = setting.wheel_target(level, steps) else {
            return false;
        };

        match setting {
            StrokeSetting::Smoothing => self.set_pen_smoothing(target),
            StrokeSetting::ShapeDetection => self.set_shape_recognition_sensitivity(target),
        }
    }

    /// Open/close the style pill's Pen feel panel. Opening it closes every
    /// other top-strip menu; the level changes made inside it keep it open
    /// (they name the panel as their popover in the event policy).
    pub(super) fn apply_toolbar_toggle_pen_feel_panel(&mut self, open: bool) -> bool {
        let changed = self
            .toolbar
            .set_top_menu_open(TopMenuState::PenFeelPanel, open);
        if changed {
            self.needs_redraw = true;
        }
        changed
    }

    /// Open the overlay's system font picker from the toolbar.
    ///
    /// The same route the color chip takes to the gradient picker: the toolbar
    /// asks, the overlay owns the modal.
    pub(super) fn apply_toolbar_open_font_picker(&mut self) -> bool {
        self.open_font_picker();
        true
    }

    pub(super) fn apply_toolbar_set_eraser_mode(&mut self, mode: EraserMode) -> bool {
        self.set_eraser_mode(mode)
    }

    pub(super) fn apply_toolbar_set_font(&mut self, descriptor: FontDescriptor) -> bool {
        self.set_font_descriptor(descriptor)
    }

    pub(super) fn apply_toolbar_set_font_bold_with(
        &mut self,
        measurer: &TextMeasurer,
        bold: bool,
    ) -> bool {
        self.set_font_bold_with(measurer, bold)
    }

    pub(super) fn apply_toolbar_set_font_size(&mut self, size: f64) -> bool {
        self.set_font_size(size)
    }

    pub(super) fn apply_toolbar_toggle_fill(&mut self, enable: bool) -> bool {
        self.set_fill_enabled(enable)
    }

    pub(super) fn apply_toolbar_set_polygon_sides(&mut self, sides: u8) -> bool {
        self.set_polygon_sides(sides)
    }

    pub(super) fn apply_toolbar_nudge_polygon_sides(&mut self, delta: i8) -> bool {
        self.nudge_polygon_sides(delta)
    }

    pub(super) fn apply_toolbar_toggle_arrow_labels(&mut self, enable: bool) -> bool {
        self.set_arrow_label_enabled(enable)
    }

    /// The pill button targets the next arrow only. Restyling a selection is
    /// the keyboard action's job, which routes on what is selected; a pill
    /// that silently retargeted itself would make the label it shows a lie.
    pub(super) fn apply_toolbar_cycle_arrow_style(&mut self) -> bool {
        self.cycle_arrow_style()
    }

    pub(super) fn apply_toolbar_reset_arrow_label_counter(&mut self) -> bool {
        self.reset_arrow_label_counter()
    }

    pub(super) fn apply_toolbar_reset_step_marker_counter(&mut self) -> bool {
        self.reset_step_marker_counter()
    }

    pub(super) fn apply_toolbar_nudge_thickness_with(
        &mut self,
        measurer: &TextMeasurer,
        delta: f64,
    ) -> bool {
        self.nudge_thickness_for_active_tool_with(measurer, delta)
    }

    pub(super) fn apply_toolbar_nudge_marker_opacity(&mut self, delta: f64) -> bool {
        self.set_marker_opacity(self.style.marker_opacity + delta)
    }

    pub(super) fn apply_toolbar_enter_text_mode_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
    ) -> bool {
        let _ = self.set_tool_override_with(resources.measurer, None);
        self.toolbar_enter_text_mode_with_resources(resources);
        self.close_top_toolbar_menus();
        true
    }

    pub(super) fn apply_toolbar_enter_sticky_note_mode_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
    ) -> bool {
        let _ = self.set_tool_override_with(resources.measurer, None);
        self.toolbar_enter_sticky_note_mode_with_resources(resources);
        self.close_top_toolbar_menus();
        true
    }

    pub(super) fn apply_toolbar_toggle_all_highlight_with(
        &mut self,
        measurer: &TextMeasurer,
        enable: bool,
    ) -> bool {
        // set_highlight_tool already handles both highlight tool and click highlight
        let currently_active = self.highlight_tool_active() || self.click_highlight_enabled();
        let mut changed = false;
        if currently_active != enable {
            self.set_highlight_tool_with_measurer(measurer, enable);
            self.needs_redraw = true;
            changed = true;
        }
        self.close_top_toolbar_menus() || changed
    }

    pub(super) fn apply_toolbar_toggle_highlight_tool_ring(&mut self, enable: bool) -> bool {
        self.set_highlight_tool_ring_enabled(enable)
    }

    pub(super) fn apply_toolbar_toggle_input_hud(&mut self, enable: bool) -> bool {
        // Presenter mode owns the HUD while it forces it on; the checkbox
        // must not fight it, exactly like `Action::ToggleInputHud`.
        if self.presenter_mode_active() && self.presenter_mode_config().enable_input_hud {
            return false;
        }
        self.set_input_hud_enabled(enable)
    }

    pub(super) fn apply_toolbar_apply_preset_with(
        &mut self,
        measurer: &TextMeasurer,
        slot: usize,
    ) -> bool {
        self.apply_preset_with(measurer, slot)
    }

    pub(super) fn apply_toolbar_save_preset(&mut self, slot: usize) -> bool {
        self.save_preset(slot)
    }

    pub(super) fn apply_toolbar_clear_preset(&mut self, slot: usize) -> bool {
        self.clear_preset(slot)
    }

    pub(super) fn apply_toolbar_copy_hex_color(&mut self) -> bool {
        self.request_copy_hex();
        true
    }

    pub(super) fn apply_toolbar_paste_hex_color(&mut self) -> bool {
        self.request_paste_hex();
        true
    }

    pub(super) fn apply_toolbar_open_color_picker_popup_with(
        &mut self,
        measurer: &TextMeasurer,
    ) -> bool {
        self.open_color_picker_popup_with_measurer(measurer);
        true
    }

    /// Open the color picker popup bound to a quick-color slot, so accepting
    /// it recolors that swatch. An index past the palette is a stale snapshot
    /// (the palette shrank between render and click) and opens nothing.
    pub(super) fn apply_toolbar_edit_quick_color_with(
        &mut self,
        measurer: &TextMeasurer,
        index: usize,
    ) -> bool {
        self.open_color_picker_popup_for_quick_color_with_measurer(measurer, index)
    }

    /// Open the color picker popup ready for typing: the hex field is
    /// focused and its content selected, so the first keystroke replaces it.
    pub(super) fn apply_toolbar_edit_hex_color_with(&mut self, measurer: &TextMeasurer) -> bool {
        self.open_color_picker_popup_with_measurer(measurer);
        self.color_picker_popup_set_hex_editing(true);
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::input::state::TopMenuState;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarEvent;

    #[test]
    fn pen_feel_panel_is_mutually_exclusive_with_the_other_top_menus() {
        let mut state = make_test_input_state();

        state.apply_toolbar_event(ToolbarEvent::ToggleLayoutMenu(true));
        assert!(state.apply_toolbar_event(ToolbarEvent::TogglePenFeelPanel(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::PenFeelPanel);

        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleTopOverflow(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::TopOverflow);

        state.apply_toolbar_event(ToolbarEvent::TogglePenFeelPanel(true));
        assert!(state.apply_toolbar_event(ToolbarEvent::TogglePenFeelPanel(false)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::Closed);
        assert!(!state.apply_toolbar_event(ToolbarEvent::TogglePenFeelPanel(false)));
    }

    /// The panel's own controls change the levels without closing it; the
    /// backend's dismissal policy spares them (see the event policy tests).
    #[test]
    fn level_changes_leave_the_pen_feel_panel_open() {
        let mut state = make_test_input_state();
        state.apply_toolbar_event(ToolbarEvent::TogglePenFeelPanel(true));

        state.apply_toolbar_event(ToolbarEvent::SetPenSmoothing(5));
        state.apply_toolbar_event(ToolbarEvent::SetShapeRecognitionSensitivity(1));

        assert_eq!(state.toolbar_top_menu(), TopMenuState::PenFeelPanel);
        assert_eq!(state.style.pen_smoothing, 5);
        assert_eq!(state.style.shape_recognition_sensitivity, 1);
    }

    #[test]
    fn selecting_a_tool_closes_the_pen_feel_panel() {
        let mut state = make_test_input_state();
        state.apply_toolbar_event(ToolbarEvent::TogglePenFeelPanel(true));

        state.apply_toolbar_event(ToolbarEvent::SelectTool(crate::input::Tool::Marker));

        assert_eq!(state.toolbar_top_menu(), TopMenuState::Closed);
    }

    #[test]
    fn edit_hex_color_opens_popup_with_hex_focused() {
        let mut state = make_test_input_state();

        let changed = state.apply_toolbar_event(ToolbarEvent::EditHexColor);

        assert!(changed);
        assert!(state.is_color_picker_popup_open());
        assert!(state.color_picker_popup_is_hex_editing());
        assert!(state.color_picker_popup_hex_selected());
    }
}
