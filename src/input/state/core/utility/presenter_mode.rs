//! Presenter mode: a snapshot-and-restore switch over the chrome, the tool
//! override, the click highlight, and the input HUD.
//!
//! Only the tool override is session content; the chrome it hides is a
//! this-run preference that `ToolStateSnapshot` deliberately excludes, so
//! entering and leaving redraw without marking the session dirty. The tool
//! override marks it through `set_tool_override`, where the change actually
//! reaches the snapshot.

use super::super::base::InputState;
use super::super::modes::PresenterRestore;
use crate::domain::Action;
use crate::input::state::{Toast, ToastPriority};
use crate::input::tool::Tool;

impl InputState {
    pub(crate) fn presenter_mode_active(&self) -> bool {
        self.modes.presenter_active()
    }

    pub(crate) fn presenter_mode_config(&self) -> &crate::config::PresenterModeConfig {
        self.modes.presenter_config()
    }

    pub(crate) fn presenter_hides_toolbars(&self) -> bool {
        self.modes.presenter_hides_toolbars()
    }

    pub(crate) fn presenter_restored_tool_preview(&self) -> Option<bool> {
        self.modes.presenter_restored_tool_preview()
    }

    pub(crate) fn presenter_restored_click_highlight(&self) -> Option<bool> {
        self.modes.presenter_restored_click_highlight()
    }

    pub(crate) fn presenter_restored_top_display_mode(
        &self,
    ) -> Option<crate::config::TopDisplayMode> {
        self.modes.presenter_restored_top_display_mode()
    }

    pub(crate) fn retarget_presenter_toolbar_display_mode(
        &mut self,
        mode: crate::config::TopDisplayMode,
    ) -> bool {
        self.modes.retarget_presenter_toolbar_display_mode(mode)
    }

    #[cfg(test)]
    pub(crate) fn presenter_restore_pending(&self) -> bool {
        self.modes.presenter_restore_pending()
    }

    #[cfg(test)]
    pub(crate) fn presenter_mode_config_mut_for_test(
        &mut self,
    ) -> &mut crate::config::PresenterModeConfig {
        self.modes.presenter_config_mut_for_test()
    }

    #[cfg(test)]
    pub(crate) fn override_presenter_mode_for_test(&mut self, active: bool) {
        self.modes.override_presenter_for_test(active);
    }

    pub(crate) fn toggle_presenter_mode(&mut self) -> bool {
        if self.presenter_mode_active() {
            self.stop_presenter_mode()
        } else {
            self.start_presenter_mode()
        }
    }

    fn stop_presenter_mode(&mut self) -> bool {
        let config = self.presenter_mode_config().clone();
        if let Some(restore) = self.modes.end_presenter() {
            if let Some(value) = restore.show_status_bar() {
                self.ui_visibility.show_status_bar = value;
            }
            if let Some(value) = restore.show_tool_preview() {
                self.ui_visibility.show_tool_preview = value;
            }
            if let Some(snapshot) = restore.toolbar_visibility() {
                self.restore_toolbar_visibility(snapshot);
            }
            if let Some(value) = restore.tool_override() {
                self.set_tool_override(value);
            }
            if let Some(value) = restore.click_highlight_enabled()
                && self.click_highlight_enabled() != value
            {
                self.toggle_click_highlight();
            }
            if let Some(value) = restore.input_hud_enabled() {
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
        self.presenter_mode_active()
    }

    fn start_presenter_mode(&mut self) -> bool {
        let config = self.presenter_mode_config().clone();
        if self.light_mode_active() {
            self.exit_light_mode();
        }
        if self.focus_mode_active() {
            // Restore Focus Mode's snapshot before Presenter Mode captures its
            // own chrome baseline. This keeps the two transient owners from
            // nesting and lets micro-toolbar presenter policy operate on the
            // real pre-Focus visibility.
            self.toggle_focus_mode();
        }

        if config.close_help_overlay && self.help_overlay.visible {
            self.toggle_help_overlay();
        }

        self.cancel_active_interaction();
        let restore = PresenterRestore::capture(
            &config,
            self.ui_visibility.show_status_bar,
            self.ui_visibility.show_tool_preview,
            self.toolbar_visibility_snapshot(),
            self.click_highlight_enabled(),
            self.input_hud_enabled(),
            self.tool_override(),
        );
        if config.hide_status_bar {
            self.ui_visibility.show_status_bar = false;
        }
        if config.hide_tool_preview {
            self.ui_visibility.show_tool_preview = false;
        }
        if config.hide_toolbars {
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
            self.set_tool_override(Some(Tool::Highlight));
        }
        if config.enable_click_highlight && !self.click_highlight_enabled() {
            self.toggle_click_highlight();
        }
        if config.enable_input_hud {
            self.set_input_hud_enabled(true);
        }

        self.modes.begin_presenter(restore);
        if config.show_toast {
            self.push_toast(
                ToastPriority::Action,
                "presenter",
                Toast::info("Presenter Mode active").action("Exit", Action::TogglePresenterMode),
            );
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.presenter_mode_active()
    }
}
