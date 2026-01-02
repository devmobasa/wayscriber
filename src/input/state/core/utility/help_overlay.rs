use super::super::base::{HelpOverlayView, InputState};

impl InputState {
    pub(crate) fn toggle_help_overlay(&mut self) {
        let now_visible = !self.show_help;
        self.show_help = now_visible;
        self.help_overlay_search.clear();
        self.help_overlay_scroll = 0.0;
        self.help_overlay_scroll_max = 0.0;
        if now_visible {
            self.help_overlay_view = HelpOverlayView::Full;
            self.help_overlay_page = 0;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn toggle_help_overlay_view(&mut self) {
        self.help_overlay_view = self.help_overlay_view.toggle();
        self.help_overlay_page = 0;
        self.help_overlay_scroll = 0.0;
        self.help_overlay_scroll_max = 0.0;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn help_overlay_page_count(&self) -> usize {
        self.help_overlay_view.page_count()
    }

    pub(crate) fn help_overlay_next_page(&mut self) -> bool {
        let next_page = self.help_overlay_page + 1;
        let page_count = self.help_overlay_page_count();
        if next_page < page_count {
            self.help_overlay_page = next_page;
            self.help_overlay_scroll = 0.0;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return true;
        }
        if matches!(self.help_overlay_view, HelpOverlayView::Quick) {
            self.help_overlay_view = HelpOverlayView::Full;
            let page_count = self.help_overlay_page_count();
            if page_count > 0 {
                self.help_overlay_page = next_page.min(page_count - 1);
            } else {
                self.help_overlay_page = 0;
            }
            self.help_overlay_scroll = 0.0;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return true;
        }
        false
    }

    pub(crate) fn help_overlay_prev_page(&mut self) -> bool {
        if self.help_overlay_page > 0 {
            self.help_overlay_page -= 1;
            self.help_overlay_scroll = 0.0;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }
}
