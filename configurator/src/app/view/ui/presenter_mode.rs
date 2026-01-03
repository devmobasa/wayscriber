use iced::widget::{column, pick_list, scrollable, text};
use iced::{Element, Length};

use crate::app::state::ConfiguratorApp;
use crate::messages::Message;
use crate::models::{PresenterToolBehaviorOption, ToggleField};

use super::super::widgets::{labeled_control, toggle_row};

impl ConfiguratorApp {
    pub(super) fn ui_presenter_mode_tab(&self) -> Element<'_, Message> {
        let tool_behavior_pick = pick_list(
            PresenterToolBehaviorOption::list(),
            Some(self.draft.presenter_tool_behavior),
            Message::PresenterToolBehaviorChanged,
        );

        let column = column![
            text("Presenter Mode").size(18),
            text("Customize what presenter mode changes when toggled.")
                .size(12)
                .style(iced::theme::Text::Color(iced::Color::from_rgb(
                    0.6, 0.6, 0.6
                ))),
            toggle_row(
                "Hide status bar",
                self.draft.presenter_hide_status_bar,
                self.defaults.presenter_hide_status_bar,
                ToggleField::PresenterHideStatusBar,
            ),
            toggle_row(
                "Hide toolbars",
                self.draft.presenter_hide_toolbars,
                self.defaults.presenter_hide_toolbars,
                ToggleField::PresenterHideToolbars,
            ),
            toggle_row(
                "Hide tool preview",
                self.draft.presenter_hide_tool_preview,
                self.defaults.presenter_hide_tool_preview,
                ToggleField::PresenterHideToolPreview,
            ),
            toggle_row(
                "Close help overlay on entry",
                self.draft.presenter_close_help_overlay,
                self.defaults.presenter_close_help_overlay,
                ToggleField::PresenterCloseHelpOverlay,
            ),
            toggle_row(
                "Force click highlights on",
                self.draft.presenter_enable_click_highlight,
                self.defaults.presenter_enable_click_highlight,
                ToggleField::PresenterEnableClickHighlight,
            ),
            labeled_control(
                "Tool behavior",
                tool_behavior_pick.width(Length::Fill).into(),
                self.defaults.presenter_tool_behavior.label().to_string(),
                self.draft.presenter_tool_behavior != self.defaults.presenter_tool_behavior,
            ),
            toggle_row(
                "Show enter/exit toast",
                self.draft.presenter_show_toast,
                self.defaults.presenter_show_toast,
                ToggleField::PresenterShowToast,
            ),
        ]
        .spacing(12);

        scrollable(column).into()
    }
}
