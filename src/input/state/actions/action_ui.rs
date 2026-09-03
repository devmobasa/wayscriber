use crate::configurator_destination::{
    ConfiguratorDestination, ConfiguratorScreen, onboarding_hints_destination,
    quick_colors_destination,
};
use crate::domain::Action;
use crate::input::state::{Toast, ToastPriority};
use log::info;

use super::super::{DrawingState, InputState, PendingBackendAction, PendingToolbarPersistence};

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
            Action::ToggleFocusMode => {
                self.handle_toggle_focus_mode();
                true
            }
            Action::ToggleStatusBar => {
                self.handle_toggle_status_bar();
                true
            }
            Action::ToggleFloatingBadge => {
                self.handle_toggle_floating_badge();
                true
            }
            Action::ToggleZoomChip => {
                self.handle_toggle_zoom_chip();
                true
            }
            Action::ToggleClickHighlight => {
                self.handle_toggle_click_highlight();
                true
            }
            Action::ToggleInputHud => {
                self.handle_toggle_input_hud();
                true
            }
            Action::ToggleToolbar => {
                self.handle_toggle_toolbar();
                true
            }
            Action::CycleToolbarDisplay => {
                self.handle_cycle_toolbar_display();
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
                self.handle_toggle_radial_menu();
                true
            }
            Action::OpenContextMenu => {
                if !self.zoom_active() {
                    self.toggle_context_menu_via_keyboard();
                }
                true
            }
            Action::ToggleSelectionProperties => {
                self.handle_toggle_selection_properties();
                true
            }
            Action::OpenConfigurator => {
                self.launch_configurator(None);
                true
            }
            Action::OpenConfiguratorKeybindings => {
                self.launch_configurator(Some(ConfiguratorDestination::new(
                    ConfiguratorScreen::Keybindings(None),
                )));
                true
            }
            Action::OpenConfiguratorPresets => {
                self.launch_configurator(Some(ConfiguratorDestination::new(
                    ConfiguratorScreen::Presets,
                )));
                true
            }
            Action::OpenConfiguratorBoards => {
                self.launch_configurator(Some(ConfiguratorDestination::new(
                    ConfiguratorScreen::Boards,
                )));
                true
            }
            Action::OpenConfiguratorQuickColors => {
                self.launch_configurator(Some(quick_colors_destination()));
                true
            }
            Action::OpenConfiguratorOnboardingHints => {
                self.launch_configurator(Some(onboarding_hints_destination()));
                true
            }
            Action::OpenAbout => {
                self.launch_about();
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

    fn handle_toggle_focus_mode(&mut self) {
        // Presenter mode already owns chrome visibility and restores it on
        // exit; a second snapshot layer would fight it.
        if self.presenter_mode {
            return;
        }
        self.toggle_focus_mode();
        info!(
            "Focus mode {}",
            if self.focus_mode_active() {
                "on"
            } else {
                "off"
            }
        );
    }

    fn handle_toggle_status_bar(&mut self) {
        if self.presenter_mode && self.presenter_mode_config.hide_status_bar {
            return;
        }
        self.break_focus_mode();
        self.queue_toolbar_persistence(PendingToolbarPersistence::StatusBar {
            previous: self.ui_visibility.show_status_bar,
        });
        self.ui_visibility.show_status_bar = !self.ui_visibility.show_status_bar;
        // This run-only preference redraws without claiming persisted changes.
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        if !self.ui_visibility.show_status_bar {
            self.warn_if_all_chrome_hidden();
        }
    }

    fn handle_toggle_floating_badge(&mut self) {
        self.break_focus_mode();
        self.queue_toolbar_persistence(PendingToolbarPersistence::FloatingBadge {
            previous: self.ui_visibility.show_floating_badge,
        });
        self.ui_visibility.show_floating_badge = !self.ui_visibility.show_floating_badge;
        info!(
            "Floating board/page badge {}",
            if self.ui_visibility.show_floating_badge {
                "shown"
            } else {
                "hidden"
            }
        );
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    fn handle_toggle_zoom_chip(&mut self) {
        self.break_focus_mode();
        self.queue_toolbar_persistence(PendingToolbarPersistence::ZoomChip {
            previous: self.ui_visibility.show_zoom_chip,
        });
        self.ui_visibility.show_zoom_chip = !self.ui_visibility.show_zoom_chip;
        info!(
            "Zoom chip {}",
            if self.ui_visibility.show_zoom_chip {
                "shown"
            } else {
                "hidden"
            }
        );
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    fn handle_toggle_click_highlight(&mut self) {
        if self.presenter_mode && self.presenter_mode_config.enable_click_highlight {
            return;
        }
        let previous_enabled = self.click_highlight_enabled();
        let previous_tool_ring = self.highlight_tool_ring_enabled();
        let enabled = self.toggle_click_highlight();
        self.queue_toolbar_persistence(PendingToolbarPersistence::ClickHighlight {
            previous_enabled,
            previous_tool_ring,
        });
        info!(
            "Click highlight {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }

    fn handle_toggle_input_hud(&mut self) {
        if self.presenter_mode && self.presenter_mode_config.enable_input_hud {
            return;
        }
        let previous = self.input_hud_enabled();
        let enabled = self.toggle_input_hud();
        self.queue_toolbar_persistence(PendingToolbarPersistence::InputHud { previous });
        if enabled {
            info!("Input HUD enabled");
        } else {
            let message = "Input HUD disabled";
            self.push_toast(ToastPriority::Info, "ui", Toast::info(message));
            info!("{}", message);
        }
    }

    fn handle_toggle_toolbar(&mut self) {
        if self.presenter_mode && self.presenter_mode_config.hide_toolbars {
            return;
        }
        self.break_focus_mode();
        let now_visible = !self.toolbar_visible();
        if !self.set_toolbar_visible(now_visible) {
            return;
        }
        let previous_top_pinned = self.toolbar_top_pinned();
        self.set_toolbar_top_pinned(now_visible);
        if previous_top_pinned != now_visible {
            self.queue_toolbar_persistence(PendingToolbarPersistence::Visibility {
                previous_top_pinned,
            });
        }
        self.pending_onboarding_usage.used_toolbar_toggle = true;
        info!(
            "Toolbar visibility {}",
            if now_visible { "enabled" } else { "disabled" }
        );
        if !now_visible {
            self.warn_if_all_chrome_hidden();
        }
    }

    fn handle_cycle_toolbar_display(&mut self) {
        if self.presenter_mode && self.presenter_mode_config.hide_toolbars {
            return;
        }
        self.break_focus_mode();
        let previous_mode = self.toolbar_top_display_mode();
        let mode = self.cycle_top_toolbar_display();
        self.pending_onboarding_usage.used_toolbar_toggle = true;
        let toast = self.toolbar_display_toast(mode);
        self.push_toast(ToastPriority::Info, "ui", toast);
        if mode == crate::config::TopDisplayMode::Hidden {
            self.warn_if_all_chrome_hidden();
        }
        self.queue_toolbar_persistence(PendingToolbarPersistence::DisplayMode {
            previous: previous_mode,
        });
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        info!("Toolbar display mode cycled to {mode:?}");
    }

    fn toolbar_display_toast(&self, mode: crate::config::TopDisplayMode) -> Toast {
        match mode {
            crate::config::TopDisplayMode::Full => Toast::info("Toolbar: full"),
            crate::config::TopDisplayMode::Micro => Toast::info("Toolbar: micro"),
            crate::config::TopDisplayMode::Hidden => {
                let label = match self.action_binding_primary_label(Action::CycleToolbarDisplay) {
                    Some(binding) => format!("Show ({binding})"),
                    None => "Show".to_string(),
                };
                Toast::info("Toolbar: hidden").action(label, Action::CycleToolbarDisplay)
            }
        }
    }

    fn handle_toggle_radial_menu(&mut self) {
        if self.is_radial_menu_open() {
            self.close_radial_menu();
        } else if !self.zoom_active() && matches!(self.state, DrawingState::Idle) {
            let (x, y) = self.pointer_position();
            self.open_radial_menu(x as f64, y as f64);
        }
    }

    fn handle_toggle_selection_properties(&mut self) {
        if !matches!(self.state, DrawingState::Idle) {
            return;
        }
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
}
