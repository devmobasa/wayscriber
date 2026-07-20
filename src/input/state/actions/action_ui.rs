use crate::domain::Action;
use crate::input::state::{Toast, ToastPriority};
use log::info;

use super::super::{DrawingState, InputState, PendingBackendAction};

impl InputState {
    pub(in crate::input::state) fn handle_ui_action(&mut self, action: Action) -> bool {
        match action {
            Action::ToggleHelp => {
                self.toggle_help_overlay();
                true
            }
            Action::ToggleQuickHelp => {
                self.toggle_quick_help();
                true
            }
            Action::ToggleStatusBar => {
                if self.presenter_mode && self.presenter_mode_config.hide_status_bar {
                    return true;
                }
                self.show_status_bar = !self.show_status_bar;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                self.mark_session_dirty();
                true
            }
            Action::ToggleClickHighlight => {
                if self.presenter_mode && self.presenter_mode_config.enable_click_highlight {
                    return true;
                }
                let enabled = self.toggle_click_highlight();
                let message = if enabled {
                    "Click highlight enabled"
                } else {
                    "Click highlight disabled"
                };
                info!("{}", message);
                true
            }
            Action::ToggleToolbar => {
                if self.presenter_mode && self.presenter_mode_config.hide_toolbars {
                    return true;
                }
                let now_visible = !self.toolbar_visible();
                let changed = self.set_toolbar_visible(now_visible);
                if changed {
                    self.pending_onboarding_usage.used_toolbar_toggle = true;
                    info!(
                        "Toolbar visibility {}",
                        if now_visible { "enabled" } else { "disabled" }
                    );
                }
                true
            }
            Action::CycleToolbarDisplay => {
                // Same presenter gate as ToggleToolbar: while presenter mode
                // owns toolbar visibility, the cycle must not fight it.
                if self.presenter_mode && self.presenter_mode_config.hide_toolbars {
                    return true;
                }
                let mode = self.cycle_top_toolbar_display();
                self.pending_onboarding_usage.used_toolbar_toggle = true;
                self.push_toast(
                    ToastPriority::Info,
                    "ui",
                    Toast::info(match mode {
                        crate::config::TopDisplayMode::Full => "Toolbar: full",
                        crate::config::TopDisplayMode::Micro => "Toolbar: micro",
                        crate::config::TopDisplayMode::Hidden => "Toolbar: hidden",
                    }),
                );
                self.set_pending_backend_action(PendingBackendAction::PersistToolbarConfig);
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                info!("Toolbar display mode cycled to {mode:?}");
                true
            }
            Action::TogglePresenterMode => {
                let enabled = self.toggle_presenter_mode();
                info!(
                    "Presenter mode {}",
                    if enabled { "enabled" } else { "disabled" }
                );
                true
            }
            Action::ToggleLightMode => {
                let enabled = self.toggle_light_mode();
                info!(
                    "Light mode {}",
                    if enabled { "enabled" } else { "disabled" }
                );
                true
            }
            Action::ToggleLightModeDrawing => {
                let drawing = self.toggle_light_mode_drawing();
                info!(
                    "Light mode drawing {}",
                    if drawing { "enabled" } else { "disabled" }
                );
                true
            }
            Action::RenderProfileNext => {
                let changed = self.activate_next_render_profile();
                if changed {
                    info!(
                        "Render profile {}",
                        self.active_render_profile()
                            .map(|profile| profile.name())
                            .unwrap_or("off")
                    );
                }
                true
            }
            Action::RenderProfilePrevious => {
                let changed = self.activate_previous_render_profile();
                if changed {
                    info!(
                        "Render profile {}",
                        self.active_render_profile()
                            .map(|profile| profile.name())
                            .unwrap_or("off")
                    );
                }
                true
            }
            Action::RenderProfileOff => {
                let changed = self.deactivate_render_profile();
                if changed {
                    info!("Render profile off");
                }
                true
            }
            Action::ToggleRadialMenu => {
                if self.is_radial_menu_open() {
                    self.close_radial_menu();
                } else if !self.zoom_active() && matches!(self.state, DrawingState::Idle) {
                    let (x, y) = self.pointer_position();
                    self.open_radial_menu(x as f64, y as f64);
                }
                true
            }
            Action::OpenContextMenu => {
                if !self.zoom_active() {
                    self.toggle_context_menu_via_keyboard();
                }
                true
            }
            Action::ToggleSelectionProperties => {
                if matches!(self.state, DrawingState::Idle) {
                    if self.properties_panel().is_some() {
                        self.close_properties_panel();
                    } else if self.show_properties_panel() {
                        self.close_context_menu();
                    } else {
                        self.push_toast(
                            ToastPriority::Info,
                            "ui",
                            Toast::warning("No selection to edit."),
                        );
                    }
                }
                true
            }
            Action::OpenConfigurator => {
                self.launch_configurator();
                true
            }
            Action::ClearSavedToolState => {
                self.set_pending_backend_action(PendingBackendAction::ClearSavedToolState);
                true
            }
            Action::OpenCaptureFolder => {
                self.open_capture_folder();
                true
            }
            Action::ReplayTour => {
                self.start_tour_replay();
                true
            }
            Action::ToggleCommandPalette => {
                self.toggle_command_palette();
                true
            }
            _ => false,
        }
    }
}
