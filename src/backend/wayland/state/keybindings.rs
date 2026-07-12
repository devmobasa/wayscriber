use super::WaylandState;
use crate::config::{KeybindingsConfig, action_label};
use crate::input::state::{KeybindingEditOperation, KeybindingEditRequest, UiToastKind};

impl WaylandState {
    pub(in crate::backend::wayland) fn handle_keybinding_edit(
        &mut self,
        request: KeybindingEditRequest,
    ) {
        let mut next = self.config.clone();
        let bindings = match request.operation {
            KeybindingEditOperation::Replace(bindings) => bindings,
            KeybindingEditOperation::Delete => Vec::new(),
            KeybindingEditOperation::Reset => KeybindingsConfig::default()
                .bindings_for_action(request.action)
                .map(ToOwned::to_owned)
                .unwrap_or_default(),
        };
        if let Err(err) = next
            .keybindings
            .set_bindings_for_action(request.action, bindings)
        {
            self.input_state.set_ui_toast(UiToastKind::Warning, err);
            return;
        }
        let action_map = match next.keybindings.build_action_map() {
            Ok(map) => map,
            Err(err) => {
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, format!("Shortcut not changed: {err}"));
                return;
            }
        };
        let action_bindings = match next.keybindings.build_action_bindings() {
            Ok(bindings) => bindings,
            Err(err) => {
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, format!("Shortcut not changed: {err}"));
                return;
            }
        };
        if let Err(err) = next.save() {
            log::warn!("Failed to save keybinding edit: {err}");
            self.input_state.set_ui_toast(
                UiToastKind::Error,
                "Shortcut could not be saved (see logs).",
            );
            return;
        }

        self.config = next;
        self.input_state
            .set_keybinding_maps(action_map, action_bindings);
        self.toolbar.mark_dirty();
        self.input_state.set_ui_toast(
            UiToastKind::Info,
            format!("Updated shortcut for {}.", action_label(request.action)),
        );
    }
}
