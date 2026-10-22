use crate::models::NamedColorOption;
use crate::test_env;

use super::*;

#[test]
fn arrow_style_options_cover_the_core_enum() {
    // `ArrowStyle::ALL` is what the runtime cycle steps through. If the two
    // lists ever disagree, a style is reachable from the overlay but not from
    // the configurator — or worse, the combo row offers one that no longer
    // exists and picks the wrong index for everything after it.
    let core = ArrowStyle::ALL;
    let options = ArrowStyleOption::list();
    assert_eq!(options.len(), core.len());
    for (option, style) in options.iter().zip(core.iter()) {
        assert_eq!(option.to_style(), *style);
        assert_eq!(ArrowStyleOption::from_style(*style), *option);
        // The combo row and the overlay's style pill name the same thing.
        assert_eq!(option.label(), style.label());
    }
}

#[test]
fn region_picker_options_cover_the_core_enum() {
    assert_eq!(
        RegionPickerOption::from_picker(RegionPicker::Native),
        RegionPickerOption::Native
    );
    assert_eq!(
        RegionPickerOption::from_picker(RegionPicker::Slurp),
        RegionPickerOption::Slurp
    );
    assert_eq!(RegionPickerOption::Slurp.to_picker(), RegionPicker::Slurp);
    assert_eq!(
        RegionPickerOption::list(),
        vec![RegionPickerOption::Native, RegionPickerOption::Slurp]
    );
}
use wayscriber::config::{
    Language, PdfFitMode, PdfLabelContentMode, PdfLabelPosition, PdfOrientation, PdfPageSize,
    PdfTransparentBackground, ReducedMotion, RegionPicker, SessionCompression, SessionStorageMode,
    StatusPosition, UiTheme,
};
use wayscriber::draw::ArrowStyle;
use wayscriber::i18n;

#[test]
fn font_style_option_handles_custom_values() {
    let (known, value) = FontStyleOption::from_value("italic");
    assert_eq!(known, FontStyleOption::Italic);
    assert_eq!(value, "italic");

    let (custom, value) = FontStyleOption::from_value("Fancy");
    assert_eq!(custom, FontStyleOption::Custom);
    assert_eq!(value, "Fancy");
}

#[test]
fn font_weight_option_handles_custom_values() {
    let (known, value) = FontWeightOption::from_value("bold");
    assert_eq!(known, FontWeightOption::Bold);
    assert_eq!(value, "bold");

    let (custom, value) = FontWeightOption::from_value("UltraHeavy");
    assert_eq!(custom, FontWeightOption::Custom);
    assert_eq!(value, "UltraHeavy");
}

#[test]
fn config_values_do_not_follow_the_interface_language() {
    test_env::in_language_child(
        concat!(
            module_path!(),
            "::config_values_do_not_follow_the_interface_language"
        ),
        || {
            use wayscriber::config::ColorSpec;
            use wayscriber::domain::Language;

            const STYLES: [&str; 4] = ["normal", "italic", "oblique", ""];
            const WEIGHTS: [&str; 7] = [
                "normal",
                "bold",
                "light",
                "ultralight",
                "heavy",
                "ultrabold",
                "",
            ];
            const COLORS: [&str; 8] = [
                "red", "green", "blue", "yellow", "orange", "pink", "white", "black",
            ];
            for language in [Language::English, Language::SimplifiedChinese] {
                i18n::set_language(language);
                // The overlay parses these words; a translated one is an
                // invalid font value or an unknown color.
                let styles: Vec<_> = FontStyleOption::list()
                    .iter()
                    .map(FontStyleOption::canonical_value)
                    .collect();
                assert_eq!(styles, STYLES, "{language:?}");
                let weights: Vec<_> = FontWeightOption::list()
                    .iter()
                    .map(FontWeightOption::canonical_value)
                    .collect();
                assert_eq!(weights, WEIGHTS, "{language:?}");
                for option in FontStyleOption::list() {
                    let value = option.canonical_value();
                    assert_eq!(FontStyleOption::from_value(value).1, value, "{language:?}");
                }
                for option in FontWeightOption::list() {
                    let value = option.canonical_value();
                    assert_eq!(FontWeightOption::from_value(value).1, value, "{language:?}");
                }

                let named: Vec<_> = NamedColorOption::list()
                    .iter()
                    .map(|option| option.as_value())
                    .collect();
                assert_eq!(named[..COLORS.len()], COLORS, "{language:?}");
                let drag: Vec<_> = DragColorOption::list()
                    .into_iter()
                    .filter_map(|option| option.to_color_spec(None))
                    .collect();
                assert_eq!(
                    drag,
                    COLORS.map(|name| ColorSpec::Name(name.to_string())),
                    "{language:?}"
                );
            }
        },
    );
}

#[test]
fn status_position_option_round_trips() {
    let option = StatusPositionOption::from_status_position(StatusPosition::BottomRight);
    assert_eq!(option, StatusPositionOption::BottomRight);
    assert!(matches!(
        option.to_status_position(),
        StatusPosition::BottomRight
    ));
}

#[test]
fn ui_theme_option_round_trips() {
    for (config_value, option) in [
        (UiTheme::Auto, UiThemeOption::Auto),
        (UiTheme::Dark, UiThemeOption::Dark),
        (UiTheme::Light, UiThemeOption::Light),
    ] {
        assert_eq!(UiThemeOption::from_theme(config_value), option);
        assert_eq!(option.to_theme(), config_value);
    }
}

#[test]
fn reduced_motion_option_round_trips() {
    for (config_value, option) in [
        (ReducedMotion::Auto, ReducedMotionOption::Auto),
        (ReducedMotion::On, ReducedMotionOption::On),
        (ReducedMotion::Off, ReducedMotionOption::Off),
    ] {
        assert_eq!(
            ReducedMotionOption::from_reduced_motion(config_value),
            option
        );
        assert_eq!(option.to_reduced_motion(), config_value);
    }
}

#[test]
fn language_option_round_trips() {
    for (config_value, option) in [
        (Language::Auto, LanguageOption::Auto),
        (Language::English, LanguageOption::English),
        (
            Language::SimplifiedChinese,
            LanguageOption::SimplifiedChinese,
        ),
    ] {
        assert_eq!(LanguageOption::from_language(config_value), option);
        assert_eq!(option.to_language(), config_value);
    }
}

#[test]
fn language_option_labels_name_each_language_in_itself() {
    test_env::in_language_child(
        concat!(
            module_path!(),
            "::language_option_labels_name_each_language_in_itself"
        ),
        || {
            use wayscriber::domain::Language;

            for language in [Language::English, Language::SimplifiedChinese] {
                i18n::set_language(language);
                // The way back from a preview reads the same in every UI language.
                assert_eq!(LanguageOption::English.label(), "English");
                assert_eq!(LanguageOption::SimplifiedChinese.label(), "简体中文");
                assert_eq!(
                    LanguageOption::Auto.label(),
                    i18n::tr("Automatic (system locale)")
                );
            }
        },
    );
}

#[test]
fn language_row_title_names_the_setting_in_every_language() {
    test_env::in_language_child(
        concat!(
            module_path!(),
            "::language_row_title_names_the_setting_in_every_language"
        ),
        || {
            use wayscriber::domain::Language;

            // The interface language reads first, and the others follow, so
            // the row is recognizable from any of them.
            for (language, title) in [
                (Language::English, "Language / 语言"),
                (Language::SimplifiedChinese, "语言 / Language"),
            ] {
                i18n::set_language(language);
                assert_eq!(LanguageOption::row_title(), title);
            }
        },
    );
}

#[test]
fn session_storage_and_compression_round_trip() {
    let storage = SessionStorageModeOption::from_mode(SessionStorageMode::Custom);
    assert_eq!(storage, SessionStorageModeOption::Custom);
    assert!(matches!(storage.to_mode(), SessionStorageMode::Custom));

    let compression = SessionCompressionOption::from_compression(SessionCompression::On);
    assert_eq!(compression, SessionCompressionOption::On);
    assert!(matches!(
        compression.to_compression(),
        SessionCompression::On
    ));
}

#[test]
fn pdf_export_options_round_trip() {
    assert_eq!(
        PdfPageSizeOption::from_config(PdfPageSize::A4).to_config(),
        PdfPageSize::A4
    );
    assert_eq!(
        PdfOrientationOption::from_config(PdfOrientation::Landscape).to_config(),
        PdfOrientation::Landscape
    );
    assert_eq!(
        PdfFitModeOption::from_config(PdfFitMode::FitContentToPage).to_config(),
        PdfFitMode::FitContentToPage
    );
    assert_eq!(
        PdfTransparentBackgroundOption::from_config(PdfTransparentBackground::Desktop).to_config(),
        PdfTransparentBackground::Desktop
    );
    assert_eq!(
        PdfLabelPositionOption::from_config(PdfLabelPosition::TopRight).to_config(),
        PdfLabelPosition::TopRight
    );
    assert_eq!(
        PdfLabelContentModeOption::from_config(PdfLabelContentMode::DocumentPage).to_config(),
        PdfLabelContentMode::DocumentPage
    );
}

#[test]
fn drag_tool_options_match_button_capabilities() {
    let left = DragToolOption::list_for_button(DragMouseButton::Left);
    assert!(!left.contains(&DragToolOption::Default));
    assert!(left.contains(&DragToolOption::RegularPolygon));

    let right = DragToolOption::list_for_button(DragMouseButton::Right);
    assert!(right.contains(&DragToolOption::Default));
    assert_eq!(right, DragToolOption::list());
}

#[test]
fn tool_options_include_freeform_but_drag_options_do_not() {
    assert!(ToolOption::list().contains(&ToolOption::FreeformPolygon));
    assert!(ToolOption::list().contains(&ToolOption::RegularPolygon));
    assert!(
        !DragToolOption::list()
            .iter()
            .any(|option| option.to_tool_option() == Some(ToolOption::FreeformPolygon))
    );
}
