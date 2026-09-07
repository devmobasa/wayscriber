use crate::app::document_workflow::LeaveAction;
use crate::app::effects::Effect;
use crate::app::state::{ConfiguratorApp, ConfirmationPrompt, PendingConfirmation, StatusMessage};

impl ConfiguratorApp {
    pub(crate) fn has_unresolved_editor(&self) -> bool {
        self.invalid_color_hex_count() > 0
            || self.shortcuts.editor().is_some()
            || self.shortcuts.recorder().is_some()
            || self.shortcuts.conflict().is_some()
    }

    pub(crate) fn has_unsaved_work(&self) -> bool {
        self.is_dirty || self.has_unresolved_editor()
    }

    pub(crate) fn pending_leave(&self) -> Option<LeaveAction> {
        match self.pending_confirmation {
            Some(PendingConfirmation::LeaveDraft(action)) => Some(action),
            _ => None,
        }
    }

    pub(in crate::app::update) fn request_leave(&mut self, action: LeaveAction) -> Vec<Effect> {
        if !self.document.allows_editing() {
            return Vec::new();
        }
        if self.has_unsaved_work() {
            self.pending_confirmation = Some(PendingConfirmation::LeaveDraft(action));
            self.status = StatusMessage::confirmation(ConfirmationPrompt::LeaveDraft);
            Vec::new()
        } else {
            self.continue_leave(action)
        }
    }

    pub(in crate::app::update) fn continue_leave(&mut self, action: LeaveAction) -> Vec<Effect> {
        self.pending_confirmation = None;
        match action {
            LeaveAction::Reload => self.begin_config_reload(),
            LeaveAction::Close => vec![Effect::CloseWindow],
        }
    }

    pub(in crate::app::update) fn save_before_leave(&mut self) -> Vec<Effect> {
        let Some(action) = self.pending_leave() else {
            return Vec::new();
        };
        self.pending_confirmation = None;
        self.save_with_continuation(Some(action))
    }

    pub(in crate::app::update) fn discard_before_leave(&mut self) -> Vec<Effect> {
        self.pending_leave()
            .map_or_else(Vec::new, |action| self.continue_leave(action))
    }

    pub(in crate::app::update) fn cancel_leave(&mut self) -> Vec<Effect> {
        if self.pending_leave().is_some() {
            self.handle_active_confirmation_canceled();
        }
        Vec::new()
    }
}
