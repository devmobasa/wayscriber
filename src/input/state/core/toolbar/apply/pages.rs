use crate::draw::PageDeleteOutcome;
use crate::input::InputState;
use crate::input::state::{Toast, ToastPriority};

impl InputState {
    pub(super) fn apply_toolbar_page_prev(&mut self) -> bool {
        if self.page_prev() {
            true
        } else {
            self.push_toast(
                ToastPriority::Info,
                "page.nav",
                Toast::info("Already on the first page."),
            );
            false
        }
    }

    pub(super) fn apply_toolbar_page_next(&mut self) -> bool {
        if self.page_next() {
            true
        } else {
            self.push_toast(
                ToastPriority::Info,
                "page.nav",
                Toast::info("Already on the last page."),
            );
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
            self.push_toast(
                ToastPriority::Info,
                "page.nav",
                Toast::info("Cleared the last page."),
            );
        }
        true
    }
}
