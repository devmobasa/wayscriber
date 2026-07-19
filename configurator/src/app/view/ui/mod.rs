mod click_highlight;
mod help_overlay;
mod presenter_mode;
mod status_bar;
mod toolbar;

use crate::app::view::theme;
use iced::widget::{Row, button, column, pick_list, text};
use iced::{Element, Length};

use crate::app::state::ConfiguratorApp;
use crate::messages::Message;
use crate::models::{ReducedMotionOption, TextField, ToggleField, UiTabId, UiThemeOption};

use super::super::search::{SearchArea, TabSearchSummary};
use super::widgets::{labeled_control, labeled_input, toggle_row};

impl ConfiguratorApp {
    pub(super) fn ui_tab(&self, search: Option<&TabSearchSummary>) -> Element<'_, Message> {
        let show_general = search.is_none_or(|search| search.area_matches(SearchArea::UiGeneral));
        let tabs = visible_ui_tabs(search);
        let active_tab = active_ui_tab(search, self.active_ui_tab);
        let tab_bar = tabs.iter().fold(
            Row::new().spacing(8).align_y(iced::Alignment::Center),
            |row, tab| {
                let label = tab.title();
                let button = button(label)
                    .padding([4, 10])
                    .style(if Some(*tab) == active_tab {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::UiTabSelected(*tab));
                row.push(button)
            },
        );

        let content = match active_tab {
            Some(UiTabId::Toolbar) => Some(self.ui_toolbar_tab()),
            Some(UiTabId::ToolbarVisibility) => Some(self.ui_toolbar_visibility_tab()),
            Some(UiTabId::StatusBar) => Some(self.ui_status_bar_tab()),
            Some(UiTabId::HelpOverlay) => Some(self.ui_help_overlay_tab()),
            Some(UiTabId::ClickHighlight) => Some(self.ui_click_highlight_tab()),
            Some(UiTabId::PresenterMode) => Some(self.ui_presenter_mode_tab()),
            None => None,
        };

        let ui_theme = pick_list(
            UiThemeOption::list(),
            Some(self.draft.ui_theme),
            Message::UiThemeChanged,
        );
        let reduced_motion = pick_list(
            ReducedMotionOption::list(),
            Some(self.draft.ui_reduced_motion),
            Message::UiReducedMotionChanged,
        );

        let general = column![
            text("General UI").size(18),
            labeled_control(
                "Theme",
                ui_theme.width(Length::Fill).into(),
                self.defaults.ui_theme.label().to_string(),
                self.draft.ui_theme != self.defaults.ui_theme,
            ),
            text("\"Auto\" currently uses the dark theme; \"Light\" takes effect as overlay surfaces adopt the runtime theme.")
                .size(12)
                .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            labeled_control(
                "Reduced motion",
                reduced_motion.width(Length::Fill).into(),
                self.defaults.ui_reduced_motion.label().to_string(),
                self.draft.ui_reduced_motion != self.defaults.ui_reduced_motion,
            ),
            text("\"On\" disables UI animations. \"Auto\" follows the system preference in a future release and keeps full motion for now.")
                .size(12)
                .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            labeled_input(
                "Preferred output (GNOME fallback)",
                &self.draft.ui_preferred_output,
                &self.defaults.ui_preferred_output,
                TextField::UiPreferredOutput,
            ),
            text("Used for the GNOME xdg-shell fallback overlay.")
                .size(12)
                .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            toggle_row(
                "Use fullscreen xdg fallback",
                self.draft.ui_xdg_fullscreen,
                self.defaults.ui_xdg_fullscreen,
                ToggleField::UiXdgFullscreen,
            ),
            toggle_row(
                "Keep open on xdg focus loss",
                self.draft.ui_xdg_keep_on_focus_loss,
                self.defaults.ui_xdg_keep_on_focus_loss,
                ToggleField::UiXdgKeepOnFocusLoss,
            ),
            toggle_row(
                "Enable context menu",
                self.draft.ui_context_menu_enabled,
                self.defaults.ui_context_menu_enabled,
                ToggleField::UiContextMenuEnabled,
            ),
            toggle_row(
                "Show capabilities warning toast",
                self.draft.ui_show_capabilities_warning,
                self.defaults.ui_show_capabilities_warning,
                ToggleField::UiShowCapabilitiesWarning,
            ),
            labeled_input(
                "Command palette toast (ms)",
                &self.draft.ui_command_palette_toast_duration_ms,
                &self.defaults.ui_command_palette_toast_duration_ms,
                TextField::UiCommandPaletteToastDurationMs,
            )
        ]
        .spacing(12);

        let mut page = column![text("UI Settings").size(20)].spacing(12);
        if show_general {
            page = page.push(general);
        }
        if !tabs.is_empty() {
            page = page.push(tab_bar);
        }
        if let Some(content) = content {
            page = page.push(content);
        }
        page.into()
    }
}

fn visible_ui_tabs(search: Option<&TabSearchSummary>) -> Vec<UiTabId> {
    match search {
        Some(summary) if !summary.show_all() => summary.ui_tabs().to_vec(),
        _ => UiTabId::ALL.to_vec(),
    }
}

fn active_ui_tab(search: Option<&TabSearchSummary>, preferred: UiTabId) -> Option<UiTabId> {
    match search {
        Some(summary) if summary.show_all() || summary.ui_tab_visible(preferred) => Some(preferred),
        Some(summary) => summary.ui_tabs().first().copied(),
        None => Some(preferred),
    }
}
