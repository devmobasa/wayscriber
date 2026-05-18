use super::super::base::{InputState, LightModeRestore, UiToastKind};
use crate::config::keybindings::Action;
use crate::input::tool::Tool;

impl InputState {
    pub fn light_mode_supported(&self) -> bool {
        self.compositor_capabilities.layer_shell
    }

    pub fn light_mode_passthrough(&self) -> bool {
        self.light_mode_supported() && self.light_mode && !self.light_mode_drawing
    }

    pub(crate) fn session_tool_override(&self) -> Option<Tool> {
        self.light_mode_restore
            .map_or_else(|| self.tool_override(), |restore| restore.tool_override)
    }

    pub(crate) fn session_active_tool(&self) -> Tool {
        self.session_tool_override()
            .unwrap_or_else(|| self.active_tool())
    }

    pub(crate) fn session_show_status_bar(&self) -> bool {
        self.light_mode_restore
            .map_or(self.show_status_bar, |restore| restore.show_status_bar)
    }

    pub(crate) fn toggle_light_mode(&mut self) -> bool {
        if self.light_mode {
            self.exit_light_mode();
        } else {
            if !self.light_mode_supported() {
                self.set_ui_toast(
                    UiToastKind::Warning,
                    "Light Mode requires layer-shell support",
                );
                self.needs_redraw = true;
                return false;
            }
            self.enter_light_mode(false);
        }
        self.light_mode
    }

    pub fn toggle_light_mode_drawing(&mut self) -> bool {
        let drawing = if self.light_mode {
            !self.light_mode_drawing
        } else {
            true
        };
        self.set_light_mode_drawing(drawing)
    }

    pub fn set_light_mode_drawing(&mut self, drawing: bool) -> bool {
        if !self.light_mode {
            if drawing {
                if !self.light_mode_supported() {
                    self.set_ui_toast(
                        UiToastKind::Warning,
                        "Light Mode requires layer-shell support",
                    );
                    self.needs_redraw = true;
                    return false;
                }
                self.enter_light_mode(true);
            }
            return self.light_mode_drawing;
        }

        if self.light_mode_drawing == drawing {
            return self.light_mode_drawing;
        }

        self.cancel_active_interaction();
        self.light_mode_drawing = drawing;
        let message = if drawing {
            "Light Mode drawing"
        } else {
            "Light Mode passthrough"
        };
        self.set_ui_toast(UiToastKind::Info, message);
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.light_mode_drawing
    }

    pub(crate) fn exit_light_mode(&mut self) {
        if !self.light_mode {
            return;
        }

        self.cancel_active_interaction();
        self.light_mode = false;
        self.light_mode_drawing = false;

        if let Some(restore) = self.light_mode_restore.take() {
            self.show_status_bar = restore.show_status_bar;
            self.show_tool_preview = restore.show_tool_preview;
            self.toolbar_visible = restore.toolbar_visible;
            self.toolbar_top_visible = restore.toolbar_top_visible;
            self.toolbar_side_visible = restore.toolbar_side_visible;
            self.set_tool_override(restore.tool_override);
            if self.click_highlight_enabled() != restore.click_highlight_enabled {
                self.toggle_click_highlight();
            }
        }

        self.set_ui_toast(UiToastKind::Info, "Stopping Light Mode");
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    fn enter_light_mode(&mut self, drawing: bool) {
        if self.presenter_mode {
            self.toggle_presenter_mode();
        }

        self.cancel_active_interaction();
        self.close_context_menu();
        self.close_properties_panel();
        self.close_radial_menu();
        self.close_board_picker();
        self.close_color_picker_popup(false);
        if self.show_help {
            self.toggle_help_overlay();
        }

        self.light_mode_restore = Some(LightModeRestore {
            show_status_bar: self.show_status_bar,
            show_tool_preview: self.show_tool_preview,
            toolbar_visible: self.toolbar_visible,
            toolbar_top_visible: self.toolbar_top_visible,
            toolbar_side_visible: self.toolbar_side_visible,
            click_highlight_enabled: self.click_highlight_enabled(),
            tool_override: self.tool_override(),
        });

        self.show_status_bar = false;
        self.show_tool_preview = false;
        self.toolbar_visible = false;
        self.toolbar_top_visible = false;
        self.toolbar_side_visible = false;
        self.set_tool_override(Some(Tool::Pen));
        if self.click_highlight_forced_in_light_mode() && !self.click_highlight_enabled() {
            self.toggle_click_highlight();
        }

        self.light_mode = true;
        self.light_mode_drawing = drawing;
        let message = if drawing {
            "Light Mode drawing"
        } else {
            "Light Mode passthrough"
        };
        self.set_ui_toast_with_action(UiToastKind::Info, message, "Exit", Action::ToggleLightMode);
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }
}
