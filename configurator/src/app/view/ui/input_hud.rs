use iced::widget::{column, pick_list, row, scrollable, text};
use iced::{Element, Length};

use crate::app::scroll::CONTENT_SCROLL_ID;
use crate::app::state::ConfiguratorApp;
use crate::messages::Message;
use crate::models::{InputHudModeOption, InputHudPositionOption, TextField, ToggleField};

use super::super::widgets::{
    labeled_control, labeled_input_with_feedback, toggle_row, validate_f64_range,
    validate_u64_range, validate_usize_range,
};

impl ConfiguratorApp {
    pub(super) fn ui_input_hud_tab(&self) -> Element<'_, Message> {
        let mode_pick = pick_list(
            InputHudModeOption::list(),
            Some(self.draft.input_hud_mode),
            Message::InputHudModeChanged,
        );
        let position_pick = pick_list(
            InputHudPositionOption::list(),
            Some(self.draft.input_hud_position),
            Message::InputHudPositionChanged,
        );

        let column = column![
            text("Input HUD").size(18),
            text("Show a live row of keystroke and click chips for demos and screencasts.")
                .size(12)
                .color(iced::Color::from_rgb(0.6, 0.6, 0.6)),
            toggle_row(
                "Enable input HUD",
                self.draft.input_hud_enabled,
                self.defaults.input_hud_enabled,
                ToggleField::UiInputHudEnabled,
            ),
            labeled_control(
                "Input source",
                mode_pick.width(Length::Fill).into(),
                self.defaults.input_hud_mode.label().to_string(),
                self.draft.input_hud_mode != self.defaults.input_hud_mode,
            ),
            text(
                "\"Overlay only\" shows what Wayscriber itself receives. \"System-wide\" also \
                 shows input that goes to other apps; it needs a build with the input-monitor \
                 feature and read access to /dev/input (usually `input` group membership), and \
                 it sees every keystroke on the seat - including passwords typed elsewhere."
            )
            .size(12)
            .color(iced::Color::from_rgb(0.6, 0.6, 0.6)),
            labeled_control(
                "Screen position",
                position_pick.width(Length::Fill).into(),
                self.defaults.input_hud_position.label().to_string(),
                self.draft.input_hud_position != self.defaults.input_hud_position,
            ),
            toggle_row(
                "Show mouse buttons and scroll",
                self.draft.input_hud_show_mouse,
                self.defaults.input_hud_show_mouse,
                ToggleField::UiInputHudShowMouse,
            ),
            toggle_row(
                "Show bare modifier taps",
                self.draft.input_hud_show_bare_modifiers,
                self.defaults.input_hud_show_bare_modifiers,
                ToggleField::UiInputHudShowBareModifiers,
            ),
            toggle_row(
                "Combine repeats into a counter",
                self.draft.input_hud_combine_repeats,
                self.defaults.input_hud_combine_repeats,
                ToggleField::UiInputHudCombineRepeats,
            ),
            row![
                labeled_input_with_feedback(
                    "Hold (ms)",
                    &self.draft.input_hud_display_ms,
                    &self.defaults.input_hud_display_ms,
                    TextField::InputHudDisplayMs,
                    Some("Range: 200-30000 ms"),
                    validate_u64_range(&self.draft.input_hud_display_ms, 200, 30_000),
                ),
                labeled_input_with_feedback(
                    "Fade (ms)",
                    &self.draft.input_hud_fade_ms,
                    &self.defaults.input_hud_fade_ms,
                    TextField::InputHudFadeMs,
                    Some("Range: 0-5000 ms"),
                    validate_u64_range(&self.draft.input_hud_fade_ms, 0, 5_000),
                ),
            ]
            .spacing(12),
            row![
                labeled_input_with_feedback(
                    "Max chips",
                    &self.draft.input_hud_max_entries,
                    &self.defaults.input_hud_max_entries,
                    TextField::InputHudMaxEntries,
                    Some("Range: 1-16"),
                    validate_usize_range(&self.draft.input_hud_max_entries, 1, 16),
                ),
                labeled_input_with_feedback(
                    "Font size",
                    &self.draft.input_hud_font_size,
                    &self.defaults.input_hud_font_size,
                    TextField::InputHudFontSize,
                    Some("Range: 6-72"),
                    validate_f64_range(&self.draft.input_hud_font_size, 6.0, 72.0),
                ),
            ]
            .spacing(12),
        ]
        .spacing(12);

        scrollable(column).id(CONTENT_SCROLL_ID).into()
    }
}
