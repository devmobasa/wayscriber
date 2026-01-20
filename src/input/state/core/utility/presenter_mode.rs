use super::super::base::{InputState, PresenterRestore, UiToastKind};
use crate::config::keybindings::Action;
use crate::input::tool::Tool;

impl InputState {
    pub(crate) fn toggle_presenter_mode(&mut self) -> bool {
        let config = self.presenter_mode_config.clone();
        if self.presenter_mode {
            self.presenter_mode = false;
            let mut status_changed = false;
            if let Some(restore) = self.presenter_restore.take() {
                if let Some(value) = restore.show_status_bar {
                    if self.show_status_bar != value {
                        status_changed = true;
                    }
                    self.show_status_bar = value;
                }
                if let Some(value) = restore.show_tool_preview {
                    self.show_tool_preview = value;
                }
                if let Some(value) = restore.toolbar_visible {
                    self.toolbar_visible = value;
                }
                if let Some(value) = restore.toolbar_top_visible {
                    self.toolbar_top_visible = value;
                }
                if let Some(value) = restore.toolbar_side_visible {
                    self.toolbar_side_visible = value;
                }
                if let Some(value) = restore.tool_override {
                    self.set_tool_override(value);
                }
                if let Some(value) = restore.click_highlight_enabled
                    && self.click_highlight_enabled() != value
                {
                    self.toggle_click_highlight();
                }
            }
            if config.show_toast {
                self.set_ui_toast(UiToastKind::Info, "Stopping Presenter Mode");
            }
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            if status_changed {
                self.mark_session_dirty();
            }
            return self.presenter_mode;
        }

        let mut restore = PresenterRestore {
            show_status_bar: None,
            show_tool_preview: None,
            toolbar_visible: None,
            toolbar_top_visible: None,
            toolbar_side_visible: None,
            click_highlight_enabled: None,
            tool_override: None,
        };

        if config.close_help_overlay && self.show_help {
            self.toggle_help_overlay();
        }

        self.cancel_active_interaction();
        let mut status_changed = false;
        if config.hide_status_bar {
            restore.show_status_bar = Some(self.show_status_bar);
            if self.show_status_bar {
                status_changed = true;
            }
            self.show_status_bar = false;
        }
        if config.hide_tool_preview {
            restore.show_tool_preview = Some(self.show_tool_preview);
            self.show_tool_preview = false;
        }
        if config.hide_toolbars {
            restore.toolbar_visible = Some(self.toolbar_visible);
            restore.toolbar_top_visible = Some(self.toolbar_top_visible);
            restore.toolbar_side_visible = Some(self.toolbar_side_visible);
            self.toolbar_visible = false;
            self.toolbar_top_visible = false;
            self.toolbar_side_visible = false;
        }
        if !matches!(
            config.tool_behavior,
            crate::config::PresenterToolBehavior::Keep
        ) {
            restore.tool_override = Some(self.tool_override());
            self.set_tool_override(Some(Tool::Highlight));
        }
        if config.enable_click_highlight {
            restore.click_highlight_enabled = Some(self.click_highlight_enabled());
            if !self.click_highlight_enabled() {
                self.toggle_click_highlight();
            }
        }

        self.presenter_restore = Some(restore);
        self.presenter_mode = true;
        if config.show_toast {
            self.set_ui_toast_with_action(
                UiToastKind::Info,
                "Presenter Mode active",
                "Exit",
                Action::TogglePresenterMode,
            );
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        if status_changed {
            self.mark_session_dirty();
        }
        self.presenter_mode
    }
}
