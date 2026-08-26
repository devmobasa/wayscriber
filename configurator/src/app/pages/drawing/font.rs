use crate::messages::Message;
use crate::models::{FontStyleOption, FontWeightOption, TextField};
use wayscriber::draw::family_is_installed;

use super::super::super::search::SearchArea;
use super::super::PageBuilder;
use super::{conditional_section, section_entry_row};

pub(super) fn build(page: &mut PageBuilder) {
    page.group_in_area("Font", SearchArea::DrawingFont)
        .entry_row_validated(
            "Font family",
            |app| app.draft.drawing_font_family.clone(),
            |value| Message::TextChanged(TextField::DrawingFontFamily, value),
            |app| validate_installed_family(&app.draft.drawing_font_family),
        )
        .entry_row_validated(
            "Font cycle list (comma separated)",
            |app| app.draft.drawing_font_cycle.clone(),
            |value| Message::TextChanged(TextField::DrawingFontCycle, value),
            |app| validate_installed_family_list(&app.draft.drawing_font_cycle),
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

/// Warn about a family the font system cannot find.
///
/// Pango resolves an unknown family to whatever fontconfig substitutes, with no
/// error anywhere, so a typo renders in a different face and looks like the
/// setting was ignored. Naming it here is the only place the user finds out.
///
/// A blank field is not an error: it means "leave the built-in default".
fn validate_installed_family(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || family_is_installed(trimmed) {
        return None;
    }
    Some(format!(
        "\"{trimmed}\" is not installed; text falls back to another font"
    ))
}

/// The same check across a comma-separated list, naming every missing family.
fn validate_installed_family_list(value: &str) -> Option<String> {
    let missing: Vec<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|family| !family.is_empty() && !family_is_installed(family))
        .collect();
    match missing.len() {
        0 => None,
        1 => Some(format!("\"{}\" is not installed", missing[0])),
        _ => Some(format!("Not installed: {}", missing.join(", "))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed() -> String {
        wayscriber::draw::system_font_families()
            .first()
            .expect("at least one family")
            .clone()
    }

    #[test]
    fn an_installed_family_raises_no_warning_and_a_blank_field_is_allowed() {
        assert_eq!(validate_installed_family(&installed()), None);
        assert_eq!(validate_installed_family(""), None);
        assert_eq!(validate_installed_family("   "), None);
    }

    #[test]
    fn a_missing_family_is_named_so_a_typo_is_findable() {
        let message = validate_installed_family("Wayscriber No Such Font 9000")
            .expect("a missing family warns");

        assert!(message.contains("Wayscriber No Such Font 9000"));
    }

    #[test]
    fn the_list_check_names_every_missing_family_and_ignores_the_present_ones() {
        let present = installed();

        assert_eq!(
            validate_installed_family_list(&format!("{present}, {present}")),
            None
        );

        let message = validate_installed_family_list(&format!("{present}, Nope One, Nope Two"))
            .expect("missing families warn");
        assert!(message.contains("Nope One"));
        assert!(message.contains("Nope Two"));
        assert!(!message.contains(&present));
    }

    #[test]
    fn blank_and_trailing_separators_in_the_list_are_not_treated_as_missing_fonts() {
        assert_eq!(validate_installed_family_list(""), None);
        assert_eq!(validate_installed_family_list(" , , "), None);
    }
}
