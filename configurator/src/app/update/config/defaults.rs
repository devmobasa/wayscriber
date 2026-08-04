use super::super::super::effects::Effect;
use super::super::super::state::{
    ConfiguratorApp, ConfirmationPrompt, PendingConfirmation, StatusMessage,
};

impl ConfiguratorApp {
    /// Arms the confirmation, and only that.
    ///
    /// Asking is one message and answering is another, so no amount of
    /// pressing the control that asks can replace the draft: a repeat while
    /// the confirmation already stands is nothing, which is what makes a
    /// double-click on "Defaults" harmless. Any other edit disarms it through
    /// `refresh_dirty_flag`, which is the same standing-down the Cancel
    /// control asks for explicitly.
    pub(in crate::app::update) fn handle_reset_to_defaults_requested(&mut self) -> Vec<Effect> {
        if self.is_loading || self.is_saving || self.defaults_reset_pending() {
            return Vec::new();
        }

        self.pending_confirmation = Some(PendingConfirmation::DefaultsReset);
        self.status = StatusMessage::confirmation(ConfirmationPrompt::DefaultsReset);
        Vec::new()
    }

    /// Applies the defaults, and only while the confirmation this answers is
    /// still armed.
    ///
    /// The typed pending identity is the whole guard: every transition that could have
    /// invalidated the question — a load, a save, a reload, an edit to the
    /// draft the user is about to lose — clears it, so a confirmation that
    /// outlived its question answers nothing.
    pub(in crate::app::update) fn handle_reset_to_defaults_confirmed(&mut self) -> Vec<Effect> {
        if !self.defaults_reset_pending() {
            return Vec::new();
        }

        self.draft = self.defaults.clone();
        self.override_mode = self.draft.ui_toolbar_layout_mode;
        self.boards_collapsed = vec![false; self.draft.boards.items.len()];
        self.color_picker_hex.clear();
        self.sync_all_color_picker_hex();
        self.clear_defaults_confirmation();
        self.status = StatusMessage::info("Loaded default configuration (not saved).");
        self.refresh_dirty_flag();
        Vec::new()
    }

    /// Stands the confirmation down, and takes its hint off the status line
    /// only while that hint is what the line still holds.
    ///
    /// Guarded on the same flag as the confirm: with nothing armed there is
    /// no question to withdraw, so a stray cancel must not wipe a status the
    /// user is reading. Disarming and clearing are separate because another
    /// operation may have replaced the hint with newer feedback while the
    /// question remained open.
    pub(in crate::app::update) fn handle_reset_to_defaults_canceled(&mut self) -> Vec<Effect> {
        if !self.defaults_reset_pending() {
            return Vec::new();
        }

        self.clear_defaults_confirmation();
        if self
            .status
            .is_confirmation(ConfirmationPrompt::DefaultsReset)
        {
            self.status = StatusMessage::idle();
        }
        Vec::new()
    }

    /// Cancels the one confirmation currently owned by the model.
    ///
    /// Escape does not know which inline controls are visible; the typed
    /// pending identity does. Taking that identity first makes a repeated key
    /// press a no-op, and the status is cleared only while it still belongs to
    /// the same question.
    pub(in crate::app::update) fn handle_active_confirmation_canceled(&mut self) -> Vec<Effect> {
        let Some(pending) = self.pending_confirmation.take() else {
            return Vec::new();
        };

        if self.status.is_confirmation(pending.prompt()) {
            self.status = StatusMessage::idle();
        }
        Vec::new()
    }
}
