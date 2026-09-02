use super::super::base::{InputEffect, InputState};

impl InputState {
    /// Marks a frozen-mode toggle request for the backend.
    pub(crate) fn request_frozen_toggle(&mut self) {
        self.emit_input_effect(InputEffect::FrozenPass {
            user_requested: true,
        });
    }

    /// Updates the cached frozen-mode status and triggers a redraw when it changes.
    pub fn set_frozen_active(&mut self, active: bool) {
        if self.frozen_active != active {
            self.frozen_active = active;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Returns whether frozen mode is active.
    pub fn frozen_active(&self) -> bool {
        self.frozen_active
    }

    /// Updates cached zoom status and triggers a redraw when it changes.
    pub fn set_zoom_status(
        &mut self,
        active: bool,
        locked: bool,
        scale: f64,
        view_offset: (f64, f64),
    ) {
        let changed = self.zoom_active != active
            || self.zoom_locked != locked
            || (self.zoom_scale - scale).abs() > f64::EPSILON
            || (self.zoom_view_offset.0 - view_offset.0).abs() > f64::EPSILON
            || (self.zoom_view_offset.1 - view_offset.1).abs() > f64::EPSILON;
        if changed {
            self.zoom_active = active;
            self.zoom_locked = locked;
            self.zoom_scale = scale;
            self.zoom_view_offset = view_offset;
            self.sync_canvas_pointer_to_current_transform();
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Returns whether zoom mode is active.
    pub fn zoom_active(&self) -> bool {
        self.zoom_active
    }

    /// Returns whether zoom view is locked.
    pub fn zoom_locked(&self) -> bool {
        self.zoom_locked
    }

    /// Returns the current zoom scale.
    pub fn zoom_scale(&self) -> f64 {
        self.zoom_scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        InputState::from_seed(crate::input::InputStateSeed {
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thickness: 4.0,
            eraser_size: 4.0,
            eraser_mode: EraserMode::Brush,
            marker_opacity: 0.32,
            fill_enabled: false,
            font_size: 32.0,
            font_descriptor: FontDescriptor::default(),
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: false,
            show_status_bar: true,
            boards_config: BoardsConfig::default(),
            action_map: action_map,
            max_shapes_per_frame: usize::MAX,
            click_highlight_settings: ClickHighlightSettings::disabled(),
            undo_all_delay_ms: 0,
            redo_all_delay_ms: 0,
            custom_section_enabled: true,
            custom_undo_delay_ms: 0,
            custom_redo_delay_ms: 0,
            custom_undo_steps: 5,
            custom_redo_steps: 5,
            presenter_mode_config: PresenterModeConfig::default(),
        })
    }

    #[test]
    fn frozen_toggle_request_is_consumed_once() {
        let mut state = make_state();
        state.request_frozen_toggle();

        assert!(state.take_pending_frozen_toggle());
        assert!(!state.take_pending_frozen_toggle());
    }

    #[test]
    fn set_frozen_active_marks_redraw_only_on_change() {
        let mut state = make_state();
        state.needs_redraw = false;

        state.set_frozen_active(true);
        assert!(state.frozen_active());
        assert!(state.needs_redraw);

        state.needs_redraw = false;
        state.set_frozen_active(true);
        assert!(!state.needs_redraw);
    }

    #[test]
    fn set_zoom_status_updates_accessors_and_marks_redraw_only_on_change() {
        let mut state = make_state();
        state.needs_redraw = false;

        state.set_zoom_status(true, true, 2.0, (40.0, 60.0));
        assert!(state.zoom_active());
        assert!(state.zoom_locked());
        assert_eq!(state.zoom_scale(), 2.0);
        assert!(state.needs_redraw);

        state.needs_redraw = false;
        state.set_zoom_status(true, true, 2.0, (40.0, 60.0));
        assert!(!state.needs_redraw);
    }
}
