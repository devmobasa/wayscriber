use crate::draw::PageDeleteOutcome;
use crate::input::InputState;
use crate::input::state::UiToastKind;

impl InputState {
    pub(super) fn apply_toolbar_page_prev(&mut self) -> bool {
        if self.page_prev() {
            true
        } else {
            self.set_ui_toast(UiToastKind::Info, "Already on the first page.");
            false
        }
    }

    pub(super) fn apply_toolbar_page_next(&mut self) -> bool {
        if self.page_next() {
            true
        } else {
            self.set_ui_toast(UiToastKind::Info, "Already on the last page.");
            false
        }
    }

    pub(super) fn apply_toolbar_page_new(&mut self) -> bool {
        self.page_new();
        true
    }

    pub(super) fn apply_toolbar_page_duplicate(&mut self) -> bool {
        self.page_duplicate();
        true
    }

    pub(super) fn apply_toolbar_page_delete(&mut self) -> bool {
        if matches!(self.page_delete(), PageDeleteOutcome::Cleared) {
            self.set_ui_toast(UiToastKind::Info, "Cleared the last page.");
        }
        true
    }
}
