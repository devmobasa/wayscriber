use wayscriber::i18n::tr;

use relm4::ComponentSender;

use crate::messages::Message;
use crate::models::{ColorPickerId, StatusPositionOption, TabId, TextField, ToggleField};

use super::super::super::state::ConfiguratorApp;
use super::super::color_rows::color_row;
use super::super::{BuiltPage, PageBuilder};
use super::{options, quad_color};

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (positions, position_labels) = options(StatusPositionOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group(tr("Status Bar"))
        .switch_row(
            tr("Show status bar"),
            "",
            |app| app.draft.ui_show_status_bar,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusBar, value),
        )
        .switch_row(
            tr("Clickable status bar segments"),
            "",
            |app| app.draft.ui_status_bar_interactive,
            |value| Message::ToggleChanged(ToggleField::UiStatusBarInteractive, value),
        );

    page.group(tr("Contents"))
        .switch_row(
            tr("Show active output"),
            tr("Appears while two or more outputs are connected"),
            |app| app.draft.ui_active_output_badge,
            |value| Message::ToggleChanged(ToggleField::UiActiveOutputBadge, value),
        )
        .switch_row(
            tr("Show active output with a single output"),
            "",
            |app| app.draft.ui_active_output_badge_always,
            |value| Message::ToggleChanged(ToggleField::UiActiveOutputBadgeAlways, value),
        )
        .switch_row(
            tr("Show selection dimensions"),
            "",
            |app| app.draft.ui_show_status_selection_info,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusSelectionInfo, value),
        )
        .switch_row(
            tr("Show board label"),
            "",
            |app| app.draft.ui_show_status_board_badge,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusBoardBadge, value),
        )
        .switch_row(
            tr("Show page counter"),
            "",
            |app| app.draft.ui_show_status_page_badge,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusPageBadge, value),
        )
        .switch_row(
            tr("Show current color"),
            "",
            |app| app.draft.ui_show_status_color,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusColor, value),
        )
        .switch_row(
            tr("Show active tool"),
            "",
            |app| app.draft.ui_show_status_tool,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusTool, value),
        )
        .switch_row(
            tr("Show tool size"),
            "",
            |app| app.draft.ui_show_status_size,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusSize, value),
        )
        .switch_row(
            tr("Show context indicators"),
            "",
            |app| app.draft.ui_show_status_context_indicators,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusContextIndicators, value),
        )
        .switch_row(
            tr("Show toolbar hint while toolbars are hidden"),
            "",
            |app| app.draft.ui_show_toolbar_hint,
            |value| Message::ToggleChanged(ToggleField::UiShowToolbarHint, value),
        )
        .switch_row(
            tr("Show Help shortcut"),
            "",
            |app| app.draft.ui_show_status_help,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusHelp, value),
        )
        .switch_row(
            tr("Show About and version"),
            "",
            |app| app.draft.ui_show_status_about,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusAbout, value),
        );

    page.group(tr("Additional Badges"))
        .switch_row(
            tr("Show board/page badge"),
            "",
            |app| app.draft.ui_show_floating_badge,
            |value| Message::ToggleChanged(ToggleField::UiShowFloatingBadge, value),
        )
        .switch_row(
            tr("Also show badge with status bar"),
            "",
            |app| app.draft.ui_show_page_badge_with_status_bar,
            |value| Message::ToggleChanged(ToggleField::UiShowPageBadgeWithStatusBar, value),
        )
        .switch_row(
            tr("Show frozen badge"),
            "",
            |app| app.draft.ui_show_frozen_badge,
            |value| Message::ToggleChanged(ToggleField::UiShowFrozenBadge, value),
        )
        .combo_row(
            tr("Status bar position"),
            "",
            positions,
            &position_labels,
            |app| app.draft.ui_status_position,
            Message::StatusPositionChanged,
        );

    page.group(tr("Status Bar Style"));
    color_row(
        &mut page,
        tr("Background (hex)"),
        ColorPickerId::StatusBarBg,
        |app| quad_color(&app.draft.status_bar_bg_color.components),
    );
    color_row(
        &mut page,
        tr("Text (hex)"),
        ColorPickerId::StatusBarText,
        |app| quad_color(&app.draft.status_bar_text_color.components),
    );
    page.entry_row(
        tr("Font size"),
        |app| app.draft.status_font_size.clone(),
        |value| Message::TextChanged(TextField::StatusFontSize, value),
    )
    .entry_row(
        tr("Padding"),
        |app| app.draft.status_padding.clone(),
        |value| Message::TextChanged(TextField::StatusPadding, value),
    )
    .entry_row(
        tr("Dot radius"),
        |app| app.draft.status_dot_radius.clone(),
        |value| Message::TextChanged(TextField::StatusDotRadius, value),
    );

    page.finish()
}
