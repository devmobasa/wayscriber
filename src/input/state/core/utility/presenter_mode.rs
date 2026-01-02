use super::super::base::{InputState, PresenterRestore, UiToastKind};

impl InputState {
    pub(crate) fn toggle_presenter_mode(&mut self) -> bool {
        if self.presenter_mode {
            self.presenter_mode = false;
            if let Some(restore) = self.presenter_restore.take() {
                self.show_status_bar = restore.show_status_bar;
                self.show_tool_preview = restore.show_tool_preview;
                self.toolbar_visible = restore.toolbar_visible;
                self.toolbar_top_visible = restore.toolbar_top_visible;
                self.toolbar_side_visible = restore.toolbar_side_visible;
                if self.click_highlight_enabled() != restore.click_highlight_enabled {
                    self.toggle_click_highlight();
                }
            }
            self.set_ui_toast(UiToastKind::Info, "Stopping Presenter Mode");
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return self.presenter_mode;
        }

        let restore = PresenterRestore {
            show_status_bar: self.show_status_bar,
            show_tool_preview: self.show_tool_preview,
            toolbar_visible: self.toolbar_visible,
            toolbar_top_visible: self.toolbar_top_visible,
            toolbar_side_visible: self.toolbar_side_visible,
            click_highlight_enabled: self.click_highlight_enabled(),
        };
        self.presenter_restore = Some(restore);
        self.presenter_mode = true;

        if self.show_help {
            self.toggle_help_overlay();
        }

        self.show_status_bar = false;
        self.show_tool_preview = false;
        self.toolbar_visible = false;
        self.toolbar_top_visible = false;
        self.toolbar_side_visible = false;
        if !self.click_highlight_enabled() {
            self.toggle_click_highlight();
        }

        self.set_ui_toast(UiToastKind::Info, "Starting Presenter Mode");
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.presenter_mode
    }
}
