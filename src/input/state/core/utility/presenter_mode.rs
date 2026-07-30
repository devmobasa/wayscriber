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
        let config = self.presenter_mode_config.clone();
        if self.presenter_mode {
            self.presenter_mode = false;
            if let Some(restore) = self.presenter_restore.take() {
                if let Some(value) = restore.show_status_bar {
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
                if let Some(value) = restore.toolbar_top_display_mode {
                    self.toolbar_top_display_mode = value;
                }
                if let Some(value) = restore.toolbar_top_minimized {
                    self.toolbar_top_minimized = value;
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
            return self.presenter_mode;
        }

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
            toolbar_visible: None,
            toolbar_top_visible: None,
            toolbar_side_visible: None,
            toolbar_top_display_mode: None,
            toolbar_top_minimized: None,
            click_highlight_enabled: None,
            input_hud_enabled: None,
            tool_override: None,
        };

        if config.close_help_overlay && self.show_help {
            self.toggle_help_overlay();
        }

        self.cancel_active_interaction();
        if config.hide_status_bar {
            restore.show_status_bar = Some(self.show_status_bar);
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
            match config.toolbar_mode {
                crate::config::PresenterToolbarMode::Hidden => {
                    self.toolbar_visible = false;
                    self.toolbar_top_visible = false;
                    self.toolbar_side_visible = false;
                }
                crate::config::PresenterToolbarMode::Micro => {
                    // The top strip stays up as the micro chip; side (and
                    // bottom) toolbars keep the hidden behavior.
                    restore.toolbar_top_display_mode = Some(self.toolbar_top_display_mode);
                    restore.toolbar_top_minimized = Some(self.toolbar_top_minimized);
                    self.toolbar_side_visible = false;
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
