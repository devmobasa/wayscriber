use super::super::base::InputState;
use crate::config::Action;

impl InputState {
    /// Sets toolbar visibility flag (controls both top and side). Returns true if toggled.
    pub fn set_toolbar_visible(&mut self, visible: bool) -> bool {
        let any_change = self.toolbar_visible != visible
            || self.toolbar_top_visible != visible
            || self.toolbar_side_visible != visible;

        if !any_change {
            return false;
        }

        self.toolbar_visible = visible;
        self.toolbar_top_visible = visible;
        self.toolbar_side_visible = visible;
        self.needs_redraw = true;
        true
    }

    /// Returns whether any toolbar is marked visible.
    pub fn toolbar_visible(&self) -> bool {
        self.toolbar_visible || self.toolbar_top_visible || self.toolbar_side_visible
    }

    /// Returns whether the top toolbar is visible.
    pub fn toolbar_top_visible(&self) -> bool {
        self.toolbar_top_visible
    }

    /// Returns whether the side toolbar is visible.
    pub fn toolbar_side_visible(&self) -> bool {
        self.toolbar_side_visible
    }

    /// Initialize toolbar visibility from config (called at startup).
    #[allow(clippy::too_many_arguments)]
    pub fn init_toolbar_from_config(
        &mut self,
        layout_mode: crate::config::ToolbarLayoutMode,
        mode_overrides: crate::config::ToolbarModeOverrides,
        top_pinned: bool,
        side_pinned: bool,
        use_icons: bool,
        show_more_colors: bool,
        show_actions_section: bool,
        show_actions_advanced: bool,
        show_zoom_actions: bool,
        show_pages_section: bool,
        show_presets: bool,
        show_step_section: bool,
        show_text_controls: bool,
        show_settings_section: bool,
        show_delay_sliders: bool,
        show_marker_opacity_section: bool,
        show_preset_toasts: bool,
        show_tool_preview: bool,
    ) {
        self.toolbar_top_pinned = top_pinned;
        self.toolbar_side_pinned = side_pinned;
        self.toolbar_top_visible = top_pinned;
        self.toolbar_side_visible = side_pinned;
        self.toolbar_visible = top_pinned || side_pinned;
        self.toolbar_use_icons = use_icons;
        self.toolbar_layout_mode = layout_mode;
        self.toolbar_mode_overrides = mode_overrides;
        self.show_more_colors = show_more_colors;
        self.show_actions_section = show_actions_section;
        self.show_actions_advanced = show_actions_advanced;
        self.show_zoom_actions = show_zoom_actions;
        self.show_pages_section = show_pages_section;
        self.show_presets = show_presets;
        self.show_step_section = show_step_section;
        self.show_text_controls = show_text_controls;
        self.show_settings_section = show_settings_section;
        self.show_delay_sliders = show_delay_sliders;
        self.show_marker_opacity_section = show_marker_opacity_section;
        self.show_preset_toasts = show_preset_toasts;
        self.show_tool_preview = show_tool_preview;
        self.apply_toolbar_mode_overrides(layout_mode);
    }

    fn apply_toolbar_mode_overrides(&mut self, mode: crate::config::ToolbarLayoutMode) {
        let overrides = self.toolbar_mode_overrides.for_mode(mode);
        if let Some(value) = overrides.show_actions_section {
            self.show_actions_section = value;
        }
        if let Some(value) = overrides.show_actions_advanced {
            self.show_actions_advanced = value;
        }
        if let Some(value) = overrides.show_zoom_actions {
            self.show_zoom_actions = value;
        }
        if let Some(value) = overrides.show_pages_section {
            self.show_pages_section = value;
        }
        if let Some(value) = overrides.show_presets {
            self.show_presets = value;
        }
        if let Some(value) = overrides.show_step_section {
            self.show_step_section = value;
        }
        if let Some(value) = overrides.show_text_controls {
            self.show_text_controls = value;
        }
        if let Some(value) = overrides.show_settings_section {
            self.show_settings_section = value;
        }
    }

    pub(crate) fn apply_toolbar_mode_defaults(&mut self, mode: crate::config::ToolbarLayoutMode) {
        let defaults = mode.section_defaults();
        let overrides = self.toolbar_mode_overrides.for_mode(mode);
        self.show_actions_section = overrides
            .show_actions_section
            .unwrap_or(defaults.show_actions_section);
        self.show_actions_advanced = overrides
            .show_actions_advanced
            .unwrap_or(defaults.show_actions_advanced);
        self.show_zoom_actions = overrides
            .show_zoom_actions
            .unwrap_or(defaults.show_zoom_actions);
        self.show_pages_section = overrides
            .show_pages_section
            .unwrap_or(defaults.show_pages_section);
        self.show_presets = overrides.show_presets.unwrap_or(defaults.show_presets);
        self.show_step_section = overrides
            .show_step_section
            .unwrap_or(defaults.show_step_section);
        self.show_text_controls = overrides
            .show_text_controls
            .unwrap_or(defaults.show_text_controls);
        self.show_settings_section = overrides
            .show_settings_section
            .unwrap_or(defaults.show_settings_section);
    }

    /// Wrapper for undo that preserves existing action plumbing.
    pub fn toolbar_undo(&mut self) {
        self.handle_action(Action::Undo);
    }

    /// Wrapper for redo that preserves existing action plumbing.
    pub fn toolbar_redo(&mut self) {
        self.handle_action(Action::Redo);
    }

    /// Wrapper for clear that preserves existing action plumbing.
    pub fn toolbar_clear(&mut self) {
        self.handle_action(Action::ClearCanvas);
    }

    /// Wrapper for entering text mode.
    pub fn toolbar_enter_text_mode(&mut self) {
        self.handle_action(Action::EnterTextMode);
    }

    /// Wrapper for entering sticky note mode.
    pub fn toolbar_enter_sticky_note_mode(&mut self) {
        self.handle_action(Action::EnterStickyNoteMode);
    }
}
