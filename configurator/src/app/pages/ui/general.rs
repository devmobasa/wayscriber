use relm4::ComponentSender;

use wayscriber::i18n::tr;

use crate::messages::Message;
use crate::models::{
    LanguageOption, ReducedMotionOption, TabId, TextField, ToggleField, UiThemeOption,
};

use super::super::super::search::SearchArea;
use super::super::super::state::ConfiguratorApp;
use super::super::{BuiltPage, PageBuilder};
use super::options;

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let (themes, theme_labels) = options(UiThemeOption::list(), |value| value.label());
    let (motions, motion_labels) = options(ReducedMotionOption::list(), |value| value.label());
    let (languages, language_labels) = options(LanguageOption::list(), |value| value.label());

    let mut page = PageBuilder::new(sender, TabId::Ui);
    page.group_in_area(tr("General UI"), SearchArea::UiGeneral)
        .combo_row(
            tr("Theme"),
            tr("\"Auto\" uses dark. \"Light\" supports the command palette, including shortcut editing and tooltips; other popups still use dark."),
            themes,
            theme_labels,
            |app| app.draft.ui_theme,
            Message::UiThemeChanged,
        )
        .combo_row(
            tr("Reduced motion"),
            tr("\"On\" disables UI animations. \"Auto\" follows the system preference in a future release and keeps full motion for now."),
            motions,
            motion_labels,
            |app| app.draft.ui_reduced_motion,
            Message::UiReducedMotionChanged,
        )
        .combo_row(
            tr("Language"),
            tr("Controls overlay UI language. \"Automatic\" follows your desktop locale."),
            languages,
            language_labels,
            |app| app.draft.ui_language,
            Message::UiLanguageChanged,
        )
        .entry_row(
            tr("Preferred output (GNOME fallback)"),
            |app| app.draft.ui_preferred_output.clone(),
            |value| Message::TextChanged(TextField::UiPreferredOutput, value),
        )
        .switch_row(
            tr("Use fullscreen xdg fallback"),
            tr("Applies to the GNOME xdg-shell fallback overlay."),
            |app| app.draft.ui_xdg_fullscreen,
            |value| Message::ToggleChanged(ToggleField::UiXdgFullscreen, value),
        )
        .switch_row(
            tr("Keep open on xdg focus loss"),
            "",
            |app| app.draft.ui_xdg_keep_on_focus_loss,
            |value| Message::ToggleChanged(ToggleField::UiXdgKeepOnFocusLoss, value),
        )
        .switch_row(
            tr("Enable context menu"),
            "",
            |app| app.draft.ui_context_menu_enabled,
            |value| Message::ToggleChanged(ToggleField::UiContextMenuEnabled, value),
        )
        .switch_row(
            tr("Show capabilities warning toast"),
            "",
            |app| app.draft.ui_show_capabilities_warning,
            |value| Message::ToggleChanged(ToggleField::UiShowCapabilitiesWarning, value),
        )
        .switch_row(
            tr("Show automatic guidance and tips"),
            tr("Controls first-run guidance, discovery tips, and shortcut coaching. Replay Tour still runs the first-run cards on request."),
            |app| app.draft.ui_show_onboarding_hints,
            |value| Message::ToggleChanged(ToggleField::UiShowOnboardingHints, value),
        )
        .switch_row(
            tr("Show shape size readout"),
            tr("Shows rectangle and ellipse preview dimensions in logical board pixels, and the shape Shape Pen has recognized. Separate from the region-capture size readout."),
            |app| app.draft.ui_show_shape_size_readout,
            |value| Message::ToggleChanged(ToggleField::UiShowShapeSizeReadout, value),
        )
        .entry_row(
            tr("Command palette toast (ms)"),
            |app| app.draft.ui_command_palette_toast_duration_ms.clone(),
            |value| Message::TextChanged(TextField::UiCommandPaletteToastDurationMs, value),
        );

    page.finish()
}
