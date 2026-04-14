use iced::Element;
use iced::theme;
use iced::widget::{button, column, horizontal_rule, row, scrollable, text, text_input};

use crate::messages::Message;
use crate::models::{DaemonAction, ShortcutApplyCapability};

use super::super::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(super) fn daemon_tab(&self) -> Element<'_, Message> {
        let busy = self.daemon_busy;
        let status_loading = self.daemon_status.is_none();
        let service_installed = self
            .daemon_status
            .as_ref()
            .is_some_and(|status| status.service_installed);

        let mut content = column![].spacing(16);

        // ── Title and explanation ──
        content = content.push(text("Background Mode").size(20)).push(text(
            "Run wayscriber in the background and toggle it with a keyboard shortcut.",
        ));

        // ── Overall status summary ──
        content = content.push(self.daemon_overall_status(busy));

        // ── Feedback banner ──
        if let Some(feedback) = self.daemon_feedback.as_deref() {
            let styled = if feedback.to_ascii_lowercase().contains("failed")
                || feedback.to_ascii_lowercase().contains("error")
            {
                text(feedback).style(theme::Text::Color(iced::Color::from_rgb(1.0, 0.5, 0.5)))
            } else {
                text(feedback).style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.9, 0.6)))
            };
            content = content.push(styled);
        }

        if busy {
            content = content.push(text("Working...").size(12));
        }

        if status_loading {
            content = content.push(
                text("Checking your system and background service status...")
                    .size(14)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            );
            content = content.push(self.daemon_technical_details(busy));
            return scrollable(content).into();
        }

        content = content.push(horizontal_rule(1));

        // ── Step 1: Install the service ──
        content = content.push(self.daemon_step_install(busy));
        content = content.push(horizontal_rule(1));

        // ── Step 2: Set your shortcut ──
        content = content.push(self.daemon_step_shortcut(busy, service_installed));
        content = content.push(horizontal_rule(1));

        // ── Step 3: Start the service ──
        content = content.push(self.daemon_step_start(busy, service_installed));
        content = content.push(horizontal_rule(1));

        // ── Technical details (bottom) ──
        content = content.push(self.daemon_technical_details(busy));

        scrollable(content).into()
    }

    fn daemon_overall_status(&self, busy: bool) -> Element<'_, Message> {
        let (label, color) = match self.daemon_status.as_ref() {
            None => ("Status: Detecting...", iced::Color::from_rgb(0.6, 0.6, 0.6)),
            Some(status) => {
                if status.service_active {
                    ("Status: Running", iced::Color::from_rgb(0.5, 0.9, 0.5))
                } else if status.service_installed {
                    (
                        "Status: Installed, not running",
                        iced::Color::from_rgb(0.95, 0.8, 0.3),
                    )
                } else {
                    (
                        "Status: Not installed",
                        iced::Color::from_rgb(0.6, 0.6, 0.6),
                    )
                }
            }
        };

        let mut refresh_button = button("Refresh").style(theme::Button::Secondary);
        if !busy {
            refresh_button = refresh_button
                .on_press(Message::DaemonActionRequested(DaemonAction::RefreshStatus));
        }

        row![
            text(label).size(16).style(theme::Text::Color(color)),
            refresh_button
        ]
        .spacing(12)
        .into()
    }

    fn daemon_step_install(&self, busy: bool) -> Element<'_, Message> {
        let installed = self
            .daemon_status
            .as_ref()
            .is_some_and(|s| s.service_installed);

        let status_indicator = if installed {
            text("Installed \u{2713}")
                .size(14)
                .style(theme::Text::Color(iced::Color::from_rgb(0.5, 0.9, 0.5)))
        } else {
            text("Not installed")
                .size(14)
                .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6)))
        };

        let button_label = if installed {
            "Update Service"
        } else {
            "Install Service"
        };
        let mut install_button = button(button_label).style(theme::Button::Secondary);
        if !busy {
            install_button = install_button.on_press(Message::DaemonActionRequested(
                DaemonAction::InstallOrUpdateService,
            ));
        }

        column![
            text("Step 1 — Install the service").size(16),
            text("Install wayscriber as a background service.").size(14),
            row![status_indicator, install_button].spacing(12),
        ]
        .spacing(8)
        .into()
    }

    fn daemon_step_shortcut(&self, busy: bool, service_installed: bool) -> Element<'_, Message> {
        if !service_installed {
            return column![
                text("Step 2 — Set your shortcut")
                    .size(16)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.55, 0.55, 0.55))),
                text("Install the background service first, then set your shortcut.")
                    .size(14)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.55, 0.55, 0.55))),
            ]
            .spacing(8)
            .into();
        }

        let apply_capability = self
            .daemon_status
            .as_ref()
            .map(|s| s.shortcut_apply_capability);
        let placeholder = match apply_capability {
            Some(ShortcutApplyCapability::GnomeCustomShortcut) => "e.g. Super+G or <Super>g",
            Some(ShortcutApplyCapability::PortalServiceDropIn) => {
                "e.g. Ctrl+Shift+G or <Ctrl><Shift>g"
            }
            _ => "e.g. Ctrl+Shift+G",
        };

        let mut shortcut_button = button("Apply Shortcut").style(theme::Button::Primary);
        if !busy && apply_capability != Some(ShortcutApplyCapability::Manual) {
            shortcut_button = shortcut_button
                .on_press(Message::DaemonActionRequested(DaemonAction::ApplyShortcut));
        }

        let mut step = column![
            text("Step 2 — Set your shortcut").size(16),
            text("Choose a keyboard shortcut to toggle drawing on/off.").size(14),
            text(
                "The shortcut takes effect after the background service is installed and running."
            )
            .size(12)
            .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
        ]
        .spacing(8);

        if let Some(configured) = self
            .daemon_status
            .as_ref()
            .and_then(|s| s.configured_shortcut.as_deref())
        {
            step = step.push(
                text(format!("Current shortcut: {configured}"))
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            );
        }

        if apply_capability == Some(ShortcutApplyCapability::Manual) {
            step = step.push(
                text("Automatic shortcut setup is unavailable here. Add a manual keybind for `wayscriber --daemon-toggle`.")
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.8, 0.3))),
            );
        }

        step = step.push(
            text_input(placeholder, &self.daemon_shortcut_input)
                .on_input(Message::DaemonShortcutInputChanged)
                .padding(8),
        );
        step = step.push(shortcut_button);

        step.into()
    }

    fn daemon_step_start(&self, busy: bool, service_installed: bool) -> Element<'_, Message> {
        if !service_installed {
            return column![
                text("Step 3 — Start the service")
                    .size(16)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.55, 0.55, 0.55))),
                text("Install the background service first.")
                    .size(14)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.55, 0.55, 0.55))),
            ]
            .spacing(8)
            .into();
        }

        let enabled = self
            .daemon_status
            .as_ref()
            .is_some_and(|status| status.service_enabled);
        let running = self
            .daemon_status
            .as_ref()
            .is_some_and(|s| s.service_active);

        let status_indicator = if running && enabled {
            text("Running \u{2713}")
                .size(14)
                .style(theme::Text::Color(iced::Color::from_rgb(0.5, 0.9, 0.5)))
        } else if running {
            text("Running (not enabled)")
                .size(14)
                .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.8, 0.3)))
        } else if enabled {
            text("Enabled, not running")
                .size(14)
                .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.8, 0.3)))
        } else {
            text("Stopped and disabled")
                .size(14)
                .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6)))
        };

        let mut step = column![
            text("Step 3 — Start the service").size(16),
            text("Enable and start the background service.").size(14),
            status_indicator,
        ]
        .spacing(8);

        if running {
            // Show Restart and Stop when running
            let mut restart_button = button("Restart").style(theme::Button::Secondary);
            if !busy {
                restart_button = restart_button
                    .on_press(Message::DaemonActionRequested(DaemonAction::RestartService));
            }
            let mut stop_button = button("Stop & Disable").style(theme::Button::Secondary);
            if !busy {
                stop_button = stop_button.on_press(Message::DaemonActionRequested(
                    DaemonAction::StopAndDisableService,
                ));
            }
            step = step.push(row![restart_button, stop_button].spacing(12));
        } else {
            // Show Start when not running
            let mut start_button = button("Start").style(theme::Button::Primary);
            if !busy {
                start_button = start_button.on_press(Message::DaemonActionRequested(
                    DaemonAction::EnableAndStartService,
                ));
            }
            step = step.push(start_button);
        }

        step.into()
    }

    fn daemon_technical_details(&self, busy: bool) -> Element<'_, Message> {
        let mut details = column![text("Details").size(14)].spacing(4);

        if let Some(status) = self.daemon_status.as_ref() {
            details = details.push(
                text(format!("Desktop: {}", status.desktop.label()))
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            );

            details = details.push(
                text(status.shortcut_backend.friendly_label())
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            );
            details = details.push(
                text(status.shortcut_apply_capability.friendly_label())
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            );

            if let Some(path) = status.service_unit_path.as_deref() {
                details = details.push(
                    text(format!("Service file: {path}"))
                        .size(12)
                        .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
                );
            }

            // Only show tool availability when something is missing
            if !status.systemctl_available || !status.gsettings_available {
                let mut missing = Vec::new();
                if !status.systemctl_available {
                    missing.push("systemctl");
                }
                if !status.gsettings_available {
                    missing.push("gsettings");
                }
                details = details.push(
                    text(format!("Missing tools: {}", missing.join(", ")))
                        .size(12)
                        .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.8, 0.3))),
                );
            }
        } else {
            details = details.push(
                text("Detecting environment...")
                    .size(12)
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            );
        }

        let mut refresh_button = button("Refresh").style(theme::Button::Secondary);
        if !busy {
            refresh_button = refresh_button
                .on_press(Message::DaemonActionRequested(DaemonAction::RefreshStatus));
        }
        details = details.push(refresh_button);

        details.into()
    }
}
