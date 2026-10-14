use wayscriber::i18n::tr;

use relm4::ComponentSender;

use crate::messages::Message;
use crate::models::{InputHudModeOption, InputHudPositionOption, TabId, TextField, ToggleField};

use super::super::super::state::ConfiguratorApp;
use super::super::{BuiltPage, PageBuilder, validate_u32_range};
use super::{note, options, validate_f64_range};


pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (modes, mode_labels) = options(InputHudModeOption::list(), |value| value.label());
    let (positions, position_labels) =
        options(InputHudPositionOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group(tr("Input HUD"))
        .custom(&note(
            tr("Show a live row of keystroke and click chips for demos and screencasts."),
        ))
        .switch_row(
            tr("Enable input HUD"),
            "",
            |app| app.draft.input_hud_enabled,
            |value| Message::ToggleChanged(ToggleField::UiInputHudEnabled, value),
        )
        .combo_row(
            tr("Input source"),
            tr("\"Overlay only\" shows what Wayscriber itself receives. \"System-wide\" also shows input that goes to other apps; it needs a build with the input-monitor feature and read access to /dev/input (usually `input` group membership), and it sees every keystroke on the seat - including passwords typed elsewhere."),
            modes,
            mode_labels,
            |app| app.draft.input_hud_mode,
            Message::InputHudModeChanged,
        )
        .combo_row(
            tr("Screen position"),
            "",
            positions,
            position_labels,
            |app| app.draft.input_hud_position,
            Message::InputHudPositionChanged,
        )
        .switch_row(
            tr("Show mouse buttons and scroll"),
            "",
            |app| app.draft.input_hud_show_mouse,
            |value| Message::ToggleChanged(ToggleField::UiInputHudShowMouse, value),
        )
        .switch_row(
            tr("Show bare modifier taps"),
            "",
            |app| app.draft.input_hud_show_bare_modifiers,
            |value| Message::ToggleChanged(ToggleField::UiInputHudShowBareModifiers, value),
        )
        .switch_row(
            tr("Combine repeats into a counter"),
            "",
            |app| app.draft.input_hud_combine_repeats,
            |value| Message::ToggleChanged(ToggleField::UiInputHudCombineRepeats, value),
        );

    page.group(tr("Chips"))
        .entry_row_validated(
            tr("Hold (ms)"),
            |app| app.draft.input_hud_display_ms.clone(),
            |value| Message::TextChanged(TextField::InputHudDisplayMs, value),
            |app| validate_u32_range(&app.draft.input_hud_display_ms, 200, 30_000),
        )
        .entry_row_validated(
            tr("Fade (ms)"),
            |app| app.draft.input_hud_fade_ms.clone(),
            |value| Message::TextChanged(TextField::InputHudFadeMs, value),
            |app| validate_u32_range(&app.draft.input_hud_fade_ms, 0, 5_000),
        )
        .entry_row_validated(
            tr("Max chips"),
            |app| app.draft.input_hud_max_entries.clone(),
            |value| Message::TextChanged(TextField::InputHudMaxEntries, value),
            |app| validate_u32_range(&app.draft.input_hud_max_entries, 1, 16),
        )
        .entry_row_validated(
            tr("Font size"),
            |app| app.draft.input_hud_font_size.clone(),
            |value| Message::TextChanged(TextField::InputHudFontSize, value),
            |app| validate_f64_range(&app.draft.input_hud_font_size, 6.0, 72.0),
        );

    page.finish()
}
