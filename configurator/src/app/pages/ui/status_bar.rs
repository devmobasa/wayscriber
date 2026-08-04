use super::*;

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (positions, position_labels) = options(StatusPositionOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group("Status Bar")
        .switch_row(
            "Show status bar",
            "",
            |app| app.draft.ui_show_status_bar,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusBar, value),
        )
        .switch_row(
            "Clickable status bar segments",
            "",
            |app| app.draft.ui_status_bar_interactive,
            |value| Message::ToggleChanged(ToggleField::UiStatusBarInteractive, value),
        );

    page.group("Contents")
        .switch_row(
            "Show active output",
            "",
            |app| app.draft.ui_active_output_badge,
            |value| Message::ToggleChanged(ToggleField::UiActiveOutputBadge, value),
        )
        .switch_row(
            "Show selection dimensions",
            "",
            |app| app.draft.ui_show_status_selection_info,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusSelectionInfo, value),
        )
        .switch_row(
            "Show board label",
            "",
            |app| app.draft.ui_show_status_board_badge,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusBoardBadge, value),
        )
        .switch_row(
            "Show page counter",
            "",
            |app| app.draft.ui_show_status_page_badge,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusPageBadge, value),
        )
        .switch_row(
            "Show current color",
            "",
            |app| app.draft.ui_show_status_color,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusColor, value),
        )
        .switch_row(
            "Show active tool",
            "",
            |app| app.draft.ui_show_status_tool,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusTool, value),
        )
        .switch_row(
            "Show tool size",
            "",
            |app| app.draft.ui_show_status_size,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusSize, value),
        )
        .switch_row(
            "Show context indicators",
            "",
            |app| app.draft.ui_show_status_context_indicators,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusContextIndicators, value),
        )
        .switch_row(
            "Show toolbar hint while toolbars are hidden",
            "",
            |app| app.draft.ui_show_toolbar_hint,
            |value| Message::ToggleChanged(ToggleField::UiShowToolbarHint, value),
        )
        .switch_row(
            "Show Help shortcut",
            "",
            |app| app.draft.ui_show_status_help,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusHelp, value),
        )
        .switch_row(
            "Show About and version",
            "",
            |app| app.draft.ui_show_status_about,
            |value| Message::ToggleChanged(ToggleField::UiShowStatusAbout, value),
        );

    page.group("Additional Badges")
        .switch_row(
            "Show board/page badge",
            "",
            |app| app.draft.ui_show_floating_badge,
            |value| Message::ToggleChanged(ToggleField::UiShowFloatingBadge, value),
        )
        .switch_row(
            "Also show badge with status bar",
            "",
            |app| app.draft.ui_show_page_badge_with_status_bar,
            |value| Message::ToggleChanged(ToggleField::UiShowPageBadgeWithStatusBar, value),
        )
        .switch_row(
            "Show frozen badge",
            "",
            |app| app.draft.ui_show_frozen_badge,
            |value| Message::ToggleChanged(ToggleField::UiShowFrozenBadge, value),
        )
        .combo_row(
            "Status bar position",
            "",
            positions,
            position_labels,
            |app| app.draft.ui_status_position,
            Message::StatusPositionChanged,
        );

    page.group("Status Bar Style");
    color_row(
        &mut page,
        "Background (hex)",
        ColorPickerId::StatusBarBg,
        |app| quad_color(&app.draft.status_bar_bg_color.components),
    );
    color_row(
        &mut page,
        "Text (hex)",
        ColorPickerId::StatusBarText,
        |app| quad_color(&app.draft.status_bar_text_color.components),
    );
    page.entry_row(
        "Font size",
        |app| app.draft.status_font_size.clone(),
        |value| Message::TextChanged(TextField::StatusFontSize, value),
    )
    .entry_row(
        "Padding",
        |app| app.draft.status_padding.clone(),
        |value| Message::TextChanged(TextField::StatusPadding, value),
    )
    .entry_row(
        "Dot radius",
        |app| app.draft.status_dot_radius.clone(),
        |value| Message::TextChanged(TextField::StatusDotRadius, value),
    );

    page.finish()
}
