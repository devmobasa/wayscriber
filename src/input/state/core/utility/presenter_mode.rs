//! Presenter mode: a snapshot-and-restore switch over the chrome, the tool
//! override, the click highlight, and the input HUD.
//!
//! Only the tool override is session content; the chrome it hides is a
//! this-run preference that `ToolStateSnapshot` deliberately excludes, so
//! entering and leaving redraw without marking the session dirty. The tool
//! override marks it through `set_tool_override`, where the change actually
//! reaches the snapshot.

use super::super::base::{InputState, PresenterRestore};
use crate::domain::Action;
use crate::input::state::{Toast, ToastPriority};
use crate::input::tool::Tool;

impl InputState {
    pub(crate) fn toggle_presenter_mode(&mut self) -> bool {
        if self.presenter_mode {
            self.stop_presenter_mode()
        } else {
            self.start_presenter_mode()
        }
    }

    fn stop_presenter_mode(&mut self) -> bool {
        let config = self.presenter_mode_config.clone();
        self.presenter_mode = false;
        if let Some(restore) = self.presenter_restore.take() {
            if let Some(value) = restore.show_status_bar {
                self.ui_visibility.show_status_bar = value;
            }
            if let Some(value) = restore.show_tool_preview {
                self.ui_visibility.show_tool_preview = value;
            }
            if let Some(snapshot) = restore.toolbar_visibility {
                self.restore_toolbar_visibility(snapshot);
            }
            if let Some(value) = restore.tool_override {
                self.set_tool_override(value);
            }
            if let Some(value) = restore.click_highlight_enabled
                && self.click_highlight_enabled() != value
            {
                self.toggle_click_highlight();
            }
            if let Some(value) = restore.input_hud_enabled {
                self.set_input_hud_enabled(value);
            }
        }
        if config.show_toast {
            self.push_toast(
                ToastPriority::Info,
                "presenter",
                Toast::info("Stopping Presenter Mode"),
            );
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.presenter_mode
    }

    fn start_presenter_mode(&mut self) -> bool {
        let config = self.presenter_mode_config.clone();
        if self.light_mode {
            self.exit_light_mode();
        }
        if self.focus_mode_active() {
            // Restore Focus Mode's snapshot before Presenter Mode captures its
            // own chrome baseline. This keeps the two transient owners from
            // nesting and lets micro-toolbar presenter policy operate on the
            // real pre-Focus visibility.
            self.toggle_focus_mode();
        }

        let mut restore = PresenterRestore {
            show_status_bar: None,
            show_tool_preview: None,
            toolbar_visibility: None,
            click_highlight_enabled: None,
            input_hud_enabled: None,
            tool_override: None,
        };

        if config.close_help_overlay && self.help_overlay.visible {
            self.toggle_help_overlay();
        }

        self.cancel_active_interaction();
        if config.hide_status_bar {
            restore.show_status_bar = Some(self.ui_visibility.show_status_bar);
            self.ui_visibility.show_status_bar = false;
        }
        if config.hide_tool_preview {
            restore.show_tool_preview = Some(self.ui_visibility.show_tool_preview);
            self.ui_visibility.show_tool_preview = false;
        }
        if config.hide_toolbars {
            restore.toolbar_visibility = Some(self.toolbar_visibility_snapshot());
            match config.toolbar_mode {
                crate::config::PresenterToolbarMode::Hidden => {
                    self.hide_toolbar_visibility();
                }
                crate::config::PresenterToolbarMode::Micro => {
                    // The top strip stays up as the micro chip.
                    self.set_top_display_mode(crate::config::TopDisplayMode::Micro);
                }
            }
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
        if config.enable_input_hud {
            restore.input_hud_enabled = Some(self.input_hud_enabled());
            self.set_input_hud_enabled(true);
        }

        self.presenter_restore = Some(restore);
        self.presenter_mode = true;
        if config.show_toast {
            self.push_toast(
                ToastPriority::Action,
                "presenter",
                Toast::info("Presenter Mode active").action("Exit", Action::TogglePresenterMode),
            );
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.presenter_mode
    }
}
