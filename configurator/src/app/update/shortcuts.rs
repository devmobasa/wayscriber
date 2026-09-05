//! Shortcut recorder, chip, reset, text-editor, and conflict messages.

#[cfg(test)]
mod tests;

use wayscriber::config::Shortcut;

use crate::app::search::keybinding_row_visible;
use crate::app::state::{ConfirmationPrompt, PendingConfirmation};
use crate::models::keybindings::{
    AppendOutcome, KeyboardModifiers, RecordedDevice, RecordedKeyboard, RecorderDeviceKind,
    ShortcutManagerFilter, ShortcutManagerSort, ShortcutManagerSummary, ShortcutRecorderState,
    ShortcutTextEditor, append_binding, apply_recorded_replace, apply_text_replace,
    next_review_conflict, normalize_button_event, normalize_key_event, other_claimants,
    parse_keybindings, remove_binding, reset_field, reset_fields, sequence_keyboard_only_message,
    text_conflicts_for,
};
use crate::models::{KeybindingField, keybinding_tab};

use super::super::effects::Effect;
use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_shortcut_recording_started(
        &mut self,
        field: KeybindingField,
    ) -> Vec<Effect> {
        self.shortcuts
            .begin_recording(ShortcutRecorderState::new(field));
        Vec::new()
    }

    pub(super) fn handle_shortcut_sequence_recording_started(
        &mut self,
        field: KeybindingField,
    ) -> Vec<Effect> {
        self.shortcuts
            .begin_recording(ShortcutRecorderState::new_sequence(field));
        Vec::new()
    }

    pub(super) fn handle_shortcut_recording_canceled(
        &mut self,
        field: KeybindingField,
    ) -> Vec<Effect> {
        self.shortcuts.cancel_recording(field);
        Vec::new()
    }

    pub(super) fn handle_shortcut_recorder_key(
        &mut self,
        keyval: u32,
        modifiers: KeyboardModifiers,
    ) -> Vec<Effect> {
        let Some(recorder) = self.shortcuts.recorder_mut() else {
            return Vec::new();
        };
        match normalize_key_event(keyval, modifiers) {
            RecordedKeyboard::Pending { preview } => {
                recorder.apply_pending_preview(preview);
                Vec::new()
            }
            RecordedKeyboard::Unsupported { message } => {
                recorder.prompt = message;
                Vec::new()
            }
            RecordedKeyboard::Chord(binding) => {
                let field = recorder.field;
                let finished = recorder.push_keyboard_step(binding);
                if let Some(shortcut) = finished {
                    self.shortcuts.set_recorder(None);
                    self.commit_recorded_binding(field, shortcut)
                } else {
                    Vec::new()
                }
            }
        }
    }

    pub(super) fn handle_shortcut_recorder_button(
        &mut self,
        button: u32,
        kind: RecorderDeviceKind,
        modifiers: KeyboardModifiers,
    ) -> Vec<Effect> {
        let Some(recorder) = self.shortcuts.recorder_mut() else {
            return Vec::new();
        };
        let field = recorder.field;
        match normalize_button_event(button, kind, modifiers) {
            RecordedDevice::Unsupported { message } => {
                recorder.prompt = message;
                Vec::new()
            }
            RecordedDevice::Trigger(trigger) => {
                if recorder.is_sequence() {
                    recorder.prompt = sequence_keyboard_only_message().to_string();
                    return Vec::new();
                }
                self.shortcuts.set_recorder(None);
                self.commit_recorded_binding(field, trigger.into())
            }
        }
    }

    pub(super) fn handle_shortcut_sequence_finish(&mut self) -> Vec<Effect> {
        let Some(recorder) = self.shortcuts.take_recorder() else {
            return Vec::new();
        };
        match recorder.finish_sequence() {
            Some(shortcut) => self.commit_recorded_binding(recorder.field, shortcut),
            None => {
                self.shortcuts.set_recorder(Some(recorder));
                Vec::new()
            }
        }
    }

    pub(super) fn handle_shortcut_sequence_remove_last_step(&mut self) -> Vec<Effect> {
        if let Some(recorder) = self.shortcuts.recorder_mut() {
            recorder.remove_last_step();
        }
        Vec::new()
    }

    pub(super) fn handle_shortcut_removed(
        &mut self,
        field: KeybindingField,
        binding: Shortcut,
    ) -> Vec<Effect> {
        if self.shortcuts.conflict().is_some() {
            return Vec::new();
        }
        match remove_binding(&mut self.draft.keybindings, field, &binding) {
            Ok(()) => {
                self.status = StatusMessage::idle();
                self.refresh_dirty_flag();
            }
            Err(error) => {
                self.status = StatusMessage::error(error.message().to_string());
            }
        }
        Vec::new()
    }

    pub(super) fn handle_shortcut_reset_requested(
        &mut self,
        field: KeybindingField,
    ) -> Vec<Effect> {
        if self.shortcuts.conflict().is_some() {
            return Vec::new();
        }
        reset_field(
            &mut self.draft.keybindings,
            &self.defaults.keybindings,
            field,
        );
        self.status = StatusMessage::idle();
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_shortcut_text_edit_started(
        &mut self,
        field: KeybindingField,
    ) -> Vec<Effect> {
        let text = self
            .draft
            .keybindings
            .value_for(field)
            .unwrap_or_default()
            .to_string();
        self.shortcuts
            .begin_text_edit(ShortcutTextEditor::new(field, text));
        Vec::new()
    }

    pub(super) fn handle_shortcut_text_edit_changed(&mut self, text: String) -> Vec<Effect> {
        if let Some(editor) = self.shortcuts.editor_mut() {
            editor.text = text;
        }
        Vec::new()
    }

    pub(super) fn handle_shortcut_text_edit_applied(&mut self) -> Vec<Effect> {
        let Some(editor) = self.shortcuts.editor().cloned() else {
            return Vec::new();
        };
        match parse_keybindings(&editor.text) {
            Ok(parsed) => {
                let conflicts = text_conflicts_for(&self.draft.keybindings, editor.field, &parsed);
                if conflicts.is_empty() {
                    self.shortcuts.set_editor(None);
                    return self
                        .handle_keybinding_changed(editor.field, editor.text.trim().to_string());
                }
                self.shortcuts.set_editor(None);
                self.shortcuts
                    .set_conflict(Some(crate::models::PendingShortcutConflict::Text {
                        target: editor.field,
                        new_value: editor.text,
                        conflicts,
                    }));
            }
            Err(error) => {
                self.status = StatusMessage::error(error);
            }
        }
        Vec::new()
    }

    pub(super) fn handle_shortcut_text_edit_canceled(
        &mut self,
        field: KeybindingField,
    ) -> Vec<Effect> {
        self.shortcuts.cancel_text_edit(field);
        Vec::new()
    }

    pub(super) fn handle_shortcut_conflict_replace_confirmed(&mut self) -> Vec<Effect> {
        let Some(pending) = self.shortcuts.take_conflict() else {
            return Vec::new();
        };
        let result = match pending {
            crate::models::PendingShortcutConflict::Recorded {
                target,
                binding,
                claimants,
            } => apply_recorded_replace(&mut self.draft.keybindings, target, &binding, &claimants),
            crate::models::PendingShortcutConflict::Text {
                target,
                new_value,
                conflicts,
            } => apply_text_replace(&mut self.draft.keybindings, target, &new_value, &conflicts),
        };
        match result {
            Ok(()) => {
                self.refresh_dirty_flag();
                if self.shortcuts.review {
                    self.arm_next_shortcut_conflict();
                } else {
                    self.status = StatusMessage::idle();
                }
            }
            Err(error) => {
                self.status = StatusMessage::error(error.message().to_string());
            }
        }
        Vec::new()
    }

    pub(super) fn handle_shortcut_conflict_canceled(&mut self) -> Vec<Effect> {
        self.shortcuts.set_conflict(None);
        self.shortcuts.review = false;
        Vec::new()
    }

    pub(super) fn handle_window_escape_pressed(&mut self) -> Vec<Effect> {
        if self.shortcut_recorder_active() {
            return Vec::new();
        }
        self.handle_active_confirmation_canceled()
    }

    pub(crate) fn shortcut_manager_summary(&self) -> ShortcutManagerSummary {
        ShortcutManagerSummary::from_drafts(&self.draft.keybindings, &self.defaults.keybindings)
    }

    pub(crate) fn visible_keybinding_fields(&self) -> Vec<KeybindingField> {
        let search = self.search_summary();
        let scope = (!self.keybindings_show_all).then_some(self.active_keybindings_tab);
        self.shortcut_manager_summary().visible_fields(
            self.shortcut_filter,
            self.shortcut_sort,
            scope,
            |field| keybinding_row_visible(&search, field),
        )
    }

    pub(crate) fn select_keybinding_field(&mut self, field: KeybindingField) {
        self.selected_keybinding = Some(field);
        self.active_keybindings_tab = keybinding_tab(field);
        self.keybinding_focus_serial = self.keybinding_focus_serial.saturating_add(1);
    }

    pub(super) fn handle_shortcut_manager_show_all(&mut self) -> Vec<Effect> {
        self.keybindings_show_all = true;
        Vec::new()
    }

    pub(super) fn handle_shortcut_manager_filter_changed(
        &mut self,
        filter: ShortcutManagerFilter,
    ) -> Vec<Effect> {
        self.shortcut_filter = filter;
        Vec::new()
    }

    pub(super) fn handle_shortcut_manager_sort_changed(
        &mut self,
        sort: ShortcutManagerSort,
    ) -> Vec<Effect> {
        self.shortcut_sort = sort;
        Vec::new()
    }

    pub(super) fn handle_shortcut_manager_row_selected(
        &mut self,
        field: KeybindingField,
    ) -> Vec<Effect> {
        self.selected_keybinding = Some(field);
        Vec::new()
    }

    pub(super) fn handle_shortcut_manager_jump_to(
        &mut self,
        field: KeybindingField,
    ) -> Vec<Effect> {
        self.select_keybinding_field(field);
        Vec::new()
    }

    pub(super) fn handle_shortcut_reset_visible_requested(&mut self) -> Vec<Effect> {
        if self.document.is_loading()
            || self.document.is_saving()
            || self.shortcut_reset_visible_pending()
        {
            return Vec::new();
        }
        let fields = self.visible_keybinding_fields();
        if fields.is_empty() {
            return Vec::new();
        }
        self.pending_confirmation = Some(PendingConfirmation::ShortcutResetVisible(fields));
        self.status = StatusMessage::confirmation(ConfirmationPrompt::ShortcutResetVisible);
        Vec::new()
    }

    pub(super) fn handle_shortcut_reset_visible_confirmed(&mut self) -> Vec<Effect> {
        let Some(PendingConfirmation::ShortcutResetVisible(fields)) =
            self.pending_confirmation.take()
        else {
            return Vec::new();
        };
        reset_fields(
            &mut self.draft.keybindings,
            &self.defaults.keybindings,
            &fields,
        );
        self.status = StatusMessage::info("Reset visible keybindings (not saved).");
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_shortcut_reset_all_requested(&mut self) -> Vec<Effect> {
        if self.document.is_loading()
            || self.document.is_saving()
            || self.shortcut_reset_all_pending()
        {
            return Vec::new();
        }
        self.pending_confirmation = Some(PendingConfirmation::ShortcutResetAll);
        self.status = StatusMessage::confirmation(ConfirmationPrompt::ShortcutResetAll);
        Vec::new()
    }

    pub(super) fn handle_shortcut_reset_all_confirmed(&mut self) -> Vec<Effect> {
        if !self.shortcut_reset_all_pending() {
            return Vec::new();
        }
        self.pending_confirmation = None;
        self.draft.keybindings = self.defaults.keybindings.clone();
        self.clear_shortcut_editing();
        self.status = StatusMessage::info("Reset all keybindings (not saved).");
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(super) fn handle_shortcut_reset_canceled(&mut self) -> Vec<Effect> {
        if !self.shortcut_reset_visible_pending() && !self.shortcut_reset_all_pending() {
            return Vec::new();
        }
        self.handle_active_confirmation_canceled()
    }

    pub(super) fn handle_shortcut_conflict_review_started(&mut self) -> Vec<Effect> {
        if !self.shortcut_manager_summary().has_conflicts() {
            self.shortcuts.review = false;
            self.status = StatusMessage::info("No shortcut conflicts to review.");
            return Vec::new();
        }
        self.shortcuts.review = true;
        if self.shortcuts.conflict().is_some() {
            return Vec::new();
        }
        self.arm_next_shortcut_conflict();
        Vec::new()
    }

    fn arm_next_shortcut_conflict(&mut self) {
        match next_review_conflict(&self.draft.keybindings) {
            Some((field, binding, claimants)) => {
                self.select_keybinding_field(field);
                self.shortcuts.set_conflict(Some(
                    crate::models::PendingShortcutConflict::Recorded {
                        target: field,
                        binding,
                        claimants,
                    },
                ));
                self.status = StatusMessage::info("Review the next conflicting shortcut.");
            }
            None => {
                self.shortcuts.review = false;
                self.status = StatusMessage::success("No remaining shortcut conflicts.");
            }
        }
    }

    fn commit_recorded_binding(
        &mut self,
        field: KeybindingField,
        binding: Shortcut,
    ) -> Vec<Effect> {
        let claimants = other_claimants(&self.draft.keybindings, field, &binding);
        if !claimants.is_empty() {
            self.shortcuts
                .set_conflict(Some(crate::models::PendingShortcutConflict::Recorded {
                    target: field,
                    binding,
                    claimants,
                }));
            return Vec::new();
        }
        match append_binding(&mut self.draft.keybindings, field, &binding) {
            Ok(AppendOutcome::Added) => {
                self.status = StatusMessage::idle();
                self.refresh_dirty_flag();
            }
            Ok(AppendOutcome::AlreadyPresent) => {}
            Err(error) => {
                self.status = StatusMessage::error(error.message().to_string());
            }
        }
        Vec::new()
    }
}
