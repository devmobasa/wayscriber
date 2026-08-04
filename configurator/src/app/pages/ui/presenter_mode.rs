use super::*;

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (toolbar_modes, toolbar_mode_labels) =
        options(PresenterToolbarModeOption::list(), |value| value.label());
    let (behaviors, behavior_labels) =
        options(PresenterToolBehaviorOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group("Presenter Mode")
        .custom(&note("Customize what presenter mode changes when toggled."))
        .switch_row(
            "Hide status bar",
            "",
            |app| app.draft.presenter_hide_status_bar,
            |value| Message::ToggleChanged(ToggleField::PresenterHideStatusBar, value),
        )
        .switch_row(
            "Hide toolbars",
            "",
            |app| app.draft.presenter_hide_toolbars,
            |value| Message::ToggleChanged(ToggleField::PresenterHideToolbars, value),
        )
        .combo_row(
            "Top toolbar while presenting",
            "",
            toolbar_modes,
            toolbar_mode_labels,
            |app| app.draft.presenter_toolbar_mode,
            Message::PresenterToolbarModeChanged,
        )
        .switch_row(
            "Hide tool preview",
            "",
            |app| app.draft.presenter_hide_tool_preview,
            |value| Message::ToggleChanged(ToggleField::PresenterHideToolPreview, value),
        )
        .switch_row(
            "Close help overlay on entry",
            "",
            |app| app.draft.presenter_close_help_overlay,
            |value| Message::ToggleChanged(ToggleField::PresenterCloseHelpOverlay, value),
        )
        .switch_row(
            "Force click highlights on",
            "",
            |app| app.draft.presenter_enable_click_highlight,
            |value| Message::ToggleChanged(ToggleField::PresenterEnableClickHighlight, value),
        )
        .switch_row(
            "Force input HUD on",
            "",
            |app| app.draft.presenter_enable_input_hud,
            |value| Message::ToggleChanged(ToggleField::PresenterEnableInputHud, value),
        )
        .combo_row(
            "Tool behavior",
            "",
            behaviors,
            behavior_labels,
            |app| app.draft.presenter_tool_behavior,
            Message::PresenterToolBehaviorChanged,
        )
        .switch_row(
            "Show enter/exit toast",
            "",
            |app| app.draft.presenter_show_toast,
            |value| Message::ToggleChanged(ToggleField::PresenterShowToast, value),
        );

    page.finish()
}
