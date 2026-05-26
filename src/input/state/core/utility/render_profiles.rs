use crate::render_profiles::{RenderColorProfile, RenderProfileSet};

use super::super::InputState;

impl InputState {
    #[allow(dead_code)] // Used by the binary Wayland backend; the library target has no backend entrypoint.
    pub(crate) fn set_render_profiles(&mut self, render_profiles: RenderProfileSet) {
        self.render_profiles = render_profiles;
    }

    pub fn active_render_profile(&self) -> Option<&RenderColorProfile> {
        self.render_profiles.active()
    }

    pub fn active_canvas_render_profile(&self) -> Option<&RenderColorProfile> {
        if self.render_profiles.applies_to_canvas() {
            self.render_profiles.active()
        } else {
            None
        }
    }

    pub fn active_ui_render_profile(&self) -> Option<&RenderColorProfile> {
        if self.render_profiles.applies_to_ui() {
            self.render_profiles.active()
        } else {
            None
        }
    }

    #[allow(dead_code)] // Used by the Wayland backend; the lib crate doesn't compile backend modules.
    pub(crate) fn export_render_profile(&self) -> Option<RenderColorProfile> {
        self.render_profiles.export_profile()
    }

    pub fn render_profile_generation(&self) -> u64 {
        self.render_profiles.generation()
    }

    pub(crate) fn activate_next_render_profile(&mut self) -> bool {
        self.activate_render_profile_with(|profiles| profiles.activate_next())
    }

    pub(crate) fn activate_previous_render_profile(&mut self) -> bool {
        self.activate_render_profile_with(|profiles| profiles.activate_previous())
    }

    pub(crate) fn deactivate_render_profile(&mut self) -> bool {
        self.activate_render_profile_with(|profiles| profiles.deactivate())
    }

    fn activate_render_profile_with(
        &mut self,
        update: impl FnOnce(&mut RenderProfileSet) -> bool,
    ) -> bool {
        if self.render_profiles.is_empty() {
            return false;
        }
        let changed = update(&mut self.render_profiles);
        if changed {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{RenderProfileConfig, RenderProfileExportMode, RenderProfilesConfig};
    use crate::input::state::test_support::make_test_input_state;
    use crate::render_profiles::RenderProfileSet;

    #[test]
    fn render_profile_actions_cycle_and_mark_redraw() {
        let mut state = make_test_input_state();
        state.set_render_profiles(RenderProfileSet::from_config(&RenderProfilesConfig {
            active: None,
            apply_to_canvas: true,
            apply_to_ui: true,
            export: RenderProfileExportMode::Off,
            export_profile: None,
            profiles: vec![RenderProfileConfig {
                id: "print".to_string(),
                name: "Print".to_string(),
                mappings: Vec::new(),
            }],
        }));
        state.needs_redraw = false;
        let generation = state.render_profile_generation();

        assert!(state.activate_next_render_profile());
        assert_eq!(
            state.active_render_profile().map(|profile| profile.name()),
            Some("Print")
        );
        assert!(state.needs_redraw);
        assert_ne!(state.render_profile_generation(), generation);

        state.needs_redraw = false;
        assert!(state.deactivate_render_profile());
        assert!(state.active_render_profile().is_none());
        assert!(state.needs_redraw);
    }

    #[test]
    fn render_profile_actions_noop_when_unconfigured() {
        let mut state = make_test_input_state();
        assert!(!state.activate_next_render_profile());
        assert!(!state.activate_previous_render_profile());
        assert!(!state.deactivate_render_profile());
    }
}
