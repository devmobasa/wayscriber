use super::*;

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (themes, theme_labels) = options(UiThemeOption::list(), |value| value.label());
    let (motions, motion_labels) = options(ReducedMotionOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);
    page.group_in_area("General UI", SearchArea::UiGeneral)
        .combo_row(
            "Theme",
            "\"Auto\" currently uses the dark theme; \"Light\" takes effect as overlay surfaces adopt the runtime theme.",
            themes,
            theme_labels,
            |app| app.draft.ui_theme,
            Message::UiThemeChanged,
        )
        .combo_row(
            "Reduced motion",
            "\"On\" disables UI animations. \"Auto\" follows the system preference in a future release and keeps full motion for now.",
            motions,
            motion_labels,
            |app| app.draft.ui_reduced_motion,
            Message::UiReducedMotionChanged,
        )
        .entry_row(
            "Preferred output (GNOME fallback)",
            |app| app.draft.ui_preferred_output.clone(),
            |value| Message::TextChanged(TextField::UiPreferredOutput, value),
        )
        .switch_row(
            "Use fullscreen xdg fallback",
            "Applies to the GNOME xdg-shell fallback overlay.",
            |app| app.draft.ui_xdg_fullscreen,
            |value| Message::ToggleChanged(ToggleField::UiXdgFullscreen, value),
        )
        .switch_row(
            "Keep open on xdg focus loss",
            "",
            |app| app.draft.ui_xdg_keep_on_focus_loss,
            |value| Message::ToggleChanged(ToggleField::UiXdgKeepOnFocusLoss, value),
        )
        .switch_row(
            "Enable context menu",
            "",
            |app| app.draft.ui_context_menu_enabled,
            |value| Message::ToggleChanged(ToggleField::UiContextMenuEnabled, value),
        )
        .switch_row(
            "Show capabilities warning toast",
            "",
            |app| app.draft.ui_show_capabilities_warning,
            |value| Message::ToggleChanged(ToggleField::UiShowCapabilitiesWarning, value),
        )
        .entry_row(
            "Command palette toast (ms)",
            |app| app.draft.ui_command_palette_toast_duration_ms.clone(),
            |value| Message::TextChanged(TextField::UiCommandPaletteToastDurationMs, value),
        );

    page.finish()
}
