use super::*;

pub(super) fn build(page: &mut PageBuilder) {
    page.group_in_area("Font", SearchArea::DrawingFont)
        .entry_row(
            "Font family",
            |app| app.draft.drawing_font_family.clone(),
            |value| Message::TextChanged(TextField::DrawingFontFamily, value),
        )
        .combo_row(
            "Font weight",
            "",
            FontWeightOption::list(),
            FontWeightOption::list()
                .iter()
                .map(|option| option.label().to_string())
                .collect(),
            |app| app.draft.drawing_font_weight_option,
            Message::FontWeightOptionSelected,
        )
        .entry_row(
            "Custom or numeric weight",
            |app| app.draft.drawing_font_weight.clone(),
            |value| Message::TextChanged(TextField::DrawingFontWeight, value),
        )
        .combo_row(
            "Font style",
            "",
            FontStyleOption::list(),
            FontStyleOption::list()
                .iter()
                .map(|option| option.label().to_string())
                .collect(),
            |app| app.draft.drawing_font_style_option,
            Message::FontStyleOptionSelected,
        );

    let custom_style = conditional_section(page, |app| {
        app.draft.drawing_font_style_option == FontStyleOption::Custom
    });
    section_entry_row(
        page,
        &custom_style,
        "Custom style",
        |app| app.draft.drawing_font_style.clone(),
        |value| Message::TextChanged(TextField::DrawingFontStyle, value),
        |_app| None,
    );
}
