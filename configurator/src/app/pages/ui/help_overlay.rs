use wayscriber::i18n::tr;

use relm4::ComponentSender;

use crate::messages::Message;
use crate::models::{ColorPickerId, TabId, TextField, ToggleField};

use super::super::super::state::ConfiguratorApp;
use super::super::color_rows::color_row;
use super::super::{BuiltPage, PageBuilder};
use super::quad_color;


pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::Ui);

    page.group(tr("Help Overlay")).switch_row(
        tr("Filter sections by enabled features"),
        "",
        |app| app.draft.help_context_filter,
        |value| Message::ToggleChanged(ToggleField::UiHelpOverlayContextFilter, value),
    );

    page.group(tr("Help Overlay Style"));
    color_row(
        &mut page,
        tr("Background (hex)"),
        ColorPickerId::HelpBg,
        |app| quad_color(&app.draft.help_bg_color.components),
    );
    color_row(
        &mut page,
        tr("Border (hex)"),
        ColorPickerId::HelpBorder,
        |app| quad_color(&app.draft.help_border_color.components),
    );
    color_row(&mut page, "Text (hex)", ColorPickerId::HelpText, |app| {
        quad_color(&app.draft.help_text_color.components)
    });
    page.entry_row(
        tr("Font family"),
        |app| app.draft.help_font_family.clone(),
        |value| Message::TextChanged(TextField::HelpFontFamily, value),
    )
    .entry_row(
        tr("Font size"),
        |app| app.draft.help_font_size.clone(),
        |value| Message::TextChanged(TextField::HelpFontSize, value),
    )
    .entry_row(
        tr("Line height"),
        |app| app.draft.help_line_height.clone(),
        |value| Message::TextChanged(TextField::HelpLineHeight, value),
    )
    .entry_row(
        tr("Padding"),
        |app| app.draft.help_padding.clone(),
        |value| Message::TextChanged(TextField::HelpPadding, value),
    )
    .entry_row(
        tr("Border width"),
        |app| app.draft.help_border_width.clone(),
        |value| Message::TextChanged(TextField::HelpBorderWidth, value),
    );

    page.finish()
}
