use super::*;

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (modes, mode_labels) = options(InputHudModeOption::list(), |value| value.label());
    let (positions, position_labels) =
        options(InputHudPositionOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group("Input HUD")
        .custom(&note(
            "Show a live row of keystroke and click chips for demos and screencasts.",
        ))
        .switch_row(
            "Enable input HUD",
            "",
            |app| app.draft.input_hud_enabled,
            |value| Message::ToggleChanged(ToggleField::UiInputHudEnabled, value),
        )
        .combo_row(
            "Input source",
            "\"Overlay only\" shows what Wayscriber itself receives. \"System-wide\" also shows input that goes to other apps; it needs a build with the input-monitor feature and read access to /dev/input (usually `input` group membership), and it sees every keystroke on the seat - including passwords typed elsewhere.",
            modes,
            mode_labels,
            |app| app.draft.input_hud_mode,
            Message::InputHudModeChanged,
        )
        .combo_row(
            "Screen position",
            "",
            positions,
            position_labels,
            |app| app.draft.input_hud_position,
            Message::InputHudPositionChanged,
        )
        .switch_row(
            "Show mouse buttons and scroll",
            "",
            |app| app.draft.input_hud_show_mouse,
            |value| Message::ToggleChanged(ToggleField::UiInputHudShowMouse, value),
        )
        .switch_row(
            "Show bare modifier taps",
            "",
            |app| app.draft.input_hud_show_bare_modifiers,
            |value| Message::ToggleChanged(ToggleField::UiInputHudShowBareModifiers, value),
        )
        .switch_row(
            "Combine repeats into a counter",
            "",
            |app| app.draft.input_hud_combine_repeats,
            |value| Message::ToggleChanged(ToggleField::UiInputHudCombineRepeats, value),
        );

    page.group("Chips")
        .entry_row_validated(
            "Hold (ms)",
            |app| app.draft.input_hud_display_ms.clone(),
            |value| Message::TextChanged(TextField::InputHudDisplayMs, value),
            |app| validate_u32_range(&app.draft.input_hud_display_ms, 200, 30_000),
        )
        .entry_row_validated(
            "Fade (ms)",
            |app| app.draft.input_hud_fade_ms.clone(),
            |value| Message::TextChanged(TextField::InputHudFadeMs, value),
            |app| validate_u32_range(&app.draft.input_hud_fade_ms, 0, 5_000),
        )
        .entry_row_validated(
            "Max chips",
            |app| app.draft.input_hud_max_entries.clone(),
            |value| Message::TextChanged(TextField::InputHudMaxEntries, value),
            |app| validate_u32_range(&app.draft.input_hud_max_entries, 1, 16),
        )
        .entry_row_validated(
            "Font size",
            |app| app.draft.input_hud_font_size.clone(),
            |value| Message::TextChanged(TextField::InputHudFontSize, value),
            |app| validate_f64_range(&app.draft.input_hud_font_size, 6.0, 72.0),
        );

    page.finish()
}
