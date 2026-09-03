use super::super::base::{DesktopEnvironment, InputState, ShellMode};
use super::super::modes::LightModeRestore;
use crate::domain::Action;
use crate::input::state::{Toast, ToastPriority};
use crate::input::tool::Tool;

impl InputState {
    pub(crate) fn light_mode_active(&self) -> bool {
        self.modes.light_active()
    }

    pub(crate) fn light_mode_drawing_active(&self) -> bool {
        self.modes.light_drawing()
    }

    pub fn light_mode_supported(&self) -> bool {
        self.compositor_capabilities.layer_shell
    }

    fn light_mode_unsupported_message(&self) -> &'static str {
        let caps = self.compositor_capabilities;
        match (
            caps.desktop_environment,
            caps.shell_mode,
            caps.freeze_capture,
        ) {
            (DesktopEnvironment::Gnome, ShellMode::XdgFallback, true) => {
                "Light Mode passthrough is not supported in this GNOME Wayland session."
            }
            (DesktopEnvironment::Gnome, ShellMode::XdgFallback, false) => {
                "Light Mode passthrough is not supported in this GNOME Wayland session. Screen capture is also unavailable."
            }
            (_, ShellMode::XdgFallback, _) => {
                "Light Mode passthrough is not supported in this desktop session."
            }
            _ => "Light Mode passthrough is not supported by this compositor.",
        }
    }

    pub fn light_mode_passthrough(&self) -> bool {
        self.light_mode_supported() && self.light_mode_active() && !self.light_mode_drawing_active()
    }

    pub(crate) fn session_tool_override(&self) -> Option<Tool> {
        self.modes
            .light_restored_tool_override()
            .unwrap_or_else(|| self.tool_override())
    }

    pub(crate) fn session_active_tool(&self) -> Tool {
        self.session_tool_override()
            .unwrap_or_else(|| self.active_tool())
    }

    pub(crate) fn toggle_light_mode(&mut self) -> bool {
        if self.light_mode_active() {
            self.exit_light_mode();
        } else {
            if !self.light_mode_supported() {
                self.push_toast(
                    ToastPriority::Info,
                    "light_mode",
                    Toast::warning(self.light_mode_unsupported_message()),
                );
                self.needs_redraw = true;
                return false;
            }
            self.enter_light_mode(false);
        }
        self.light_mode_active()
    }

    pub fn toggle_light_mode_drawing(&mut self) -> bool {
        let drawing = if self.light_mode_active() {
            !self.light_mode_drawing_active()
        } else {
            true
        };
        self.set_light_mode_drawing(drawing)
    }

    pub fn set_light_mode_drawing(&mut self, drawing: bool) -> bool {
        if !self.light_mode_active() {
            if drawing {
                if !self.light_mode_supported() {
                    self.push_toast(
                        ToastPriority::Info,
                        "light_mode",
                        Toast::warning(self.light_mode_unsupported_message()),
                    );
                    self.needs_redraw = true;
                    return false;
                }
                self.enter_light_mode(true);
            }
            return self.light_mode_drawing_active();
        }

        if self.light_mode_drawing_active() == drawing {
            return self.light_mode_drawing_active();
        }

        self.cancel_active_interaction();
        self.modes.set_light_drawing(drawing);
        let message = if drawing {
            "Light Mode drawing"
        } else {
            "Light Mode passthrough"
        };
        self.push_toast(ToastPriority::Info, "light_mode", Toast::info(message));
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.light_mode_drawing_active()
    }

    pub(crate) fn exit_light_mode(&mut self) {
        if !self.light_mode_active() {
            return;
        }

        self.cancel_active_interaction();

        if let Some(restore) = self.modes.end_light() {
            self.ui_visibility.show_status_bar = restore.show_status_bar();
            self.ui_visibility.show_tool_preview = restore.show_tool_preview();
            self.restore_toolbar_visibility(restore.toolbar_visibility());
            self.set_tool_override(restore.tool_override());
            if self.click_highlight_enabled() != restore.click_highlight_enabled() {
                self.toggle_click_highlight();
            }
        }

        self.push_toast(
            ToastPriority::Info,
            "light_mode",
            Toast::info("Stopping Light Mode"),
        );
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    fn enter_light_mode(&mut self, drawing: bool) {
        if self.focus_mode_active() {
            self.toggle_focus_mode();
        }
        if self.presenter_mode_active() {
            self.toggle_presenter_mode();
        }

        self.cancel_active_interaction();
        self.close_context_menu();
        self.close_properties_panel();
        self.close_radial_menu();
        self.close_board_picker();
        self.close_color_picker_popup(false);
        if self.help_overlay.visible {
            self.toggle_help_overlay();
        }

        let restore = LightModeRestore::capture(
            self.ui_visibility.show_status_bar,
            self.ui_visibility.show_tool_preview,
            self.toolbar_visibility_snapshot(),
            self.click_highlight_enabled(),
            self.tool_override(),
        );

        self.ui_visibility.show_status_bar = false;
        self.ui_visibility.show_tool_preview = false;
        self.hide_toolbar_visibility();
        self.set_tool_override(Some(Tool::Pen));
        if self.click_highlight_forced_in_light_mode() && !self.click_highlight_enabled() {
            self.toggle_click_highlight();
        }

        self.modes.begin_light(drawing, restore);
        let message = if drawing {
            "Light Mode drawing"
        } else {
            "Light Mode passthrough"
        };
        self.push_toast(
            ToastPriority::Action,
            "light_mode",
            Toast::info(message).action("Exit", Action::ToggleLightMode),
        );
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }
}
