use super::*;
use crate::backend::wayland::config_writer::ConfigMutation;

impl WaylandState {
    pub(in crate::backend::wayland) fn queue_config_mutation(
        &mut self,
        mutation: ConfigMutation,
        description: &str,
    ) -> bool {
        if self.config_writer.request(&mutation) {
            // Keep the runtime baseline aligned with accepted writes. The
            // worker owns retrying the same mutation, so a later config edit
            // cannot reintroduce the stale pre-request value while it waits.
            let _ = mutation.apply(&mut self.config);
            // Seeds are derived from the config this edit just changed, so
            // reseed here rather than waiting for an unrelated session, board,
            // or keybinding event: until then, override reconciliation and
            // redundant-override deletion would key off the pre-edit baseline.
            if mutation.affects_runtime_ui_seeds() {
                self.refresh_runtime_ui_config_seeds();
            }
            log::debug!("Queued {description}");
            true
        } else {
            log::warn!("Failed to queue {description}; runtime value remains session-only");
            false
        }
    }

    pub(in crate::backend::wayland) fn shutdown_config_writer(&mut self) {
        self.config_writer.shutdown();
    }

    /// Flush durable edits that are queued but not yet written. Pointer-driven
    /// accepts (the color picker's OK button) queue their config write from the
    /// release handler, which is not itself a drain site, so the exit path calls
    /// this before the process goes away.
    pub(in crate::backend::wayland) fn persist_pending_config_edits(&mut self) {
        if let Some(edit) = self.input_state.take_pending_quick_color_edit() {
            self.handle_quick_color_edit(edit);
        }
    }

    /// Persist an accepted quick-color recolor. The runtime palette already
    /// shows the color; this writes `drawing.quick_colors` through the same
    /// reload-and-save guard the other runtime-owned config edits use, so a
    /// long-lived snapshot cannot overwrite newer edits from the configurator
    /// or the file itself.
    pub(in crate::backend::wayland) fn handle_quick_color_edit(
        &mut self,
        edit: crate::input::state::QuickColorEdit,
    ) {
        let crate::input::state::QuickColorEdit { index, color } = edit;
        self.queue_config_mutation(
            ConfigMutation::QuickColor { index, color },
            "quick color persistence",
        );
    }

    pub(in crate::backend::wayland) fn handle_preset_action(
        &mut self,
        action: crate::input::state::PresetAction,
    ) {
        let mutation = match action {
            crate::input::state::PresetAction::Save { slot, preset } => {
                ConfigMutation::PresetSlot {
                    slot,
                    preset: Some(preset),
                }
            }
            crate::input::state::PresetAction::Clear { slot } => {
                ConfigMutation::PresetSlot { slot, preset: None }
            }
        };
        self.queue_config_mutation(mutation, "preset persistence");
    }
}
