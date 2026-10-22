use crate::draw::{PageDeleteOutcome, TextMeasurer};
use crate::i18n::tr;
use crate::input::InputState;
use crate::input::state::{Toast, ToastPriority};

impl InputState {
    pub(super) fn apply_toolbar_page_prev_with(&mut self, measurer: &TextMeasurer) -> bool {
        if self.page_prev_with_measurer(measurer) {
            true
        } else {
            self.push_toast(
                ToastPriority::Info,
                "page.nav",
                Toast::info(tr("Already on the first page.")),
            );
            false
        }
    }

    pub(super) fn apply_toolbar_page_next_with(&mut self, measurer: &TextMeasurer) -> bool {
        if self.page_next_with_measurer(measurer) {
            true
        } else {
            self.push_toast(
                ToastPriority::Info,
                "page.nav",
                Toast::info(tr("Already on the last page.")),
            );
            false
        }
    }

    pub(super) fn apply_toolbar_page_new_with(&mut self, measurer: &TextMeasurer) -> bool {
        self.page_new_with_measurer(measurer);
        true
    }

    pub(super) fn apply_toolbar_page_duplicate_with(&mut self, measurer: &TextMeasurer) -> bool {
        self.page_duplicate_with_measurer(measurer);
        true
    }

    pub(super) fn apply_toolbar_page_delete_with(&mut self, measurer: &TextMeasurer) -> bool {
        if matches!(
            self.page_delete_with_measurer(measurer),
            PageDeleteOutcome::Cleared
        ) {
            self.push_toast(
                ToastPriority::Info,
                "page.nav",
                Toast::info(tr("Cleared the last page.")),
            );
        }
        true
    }
}
