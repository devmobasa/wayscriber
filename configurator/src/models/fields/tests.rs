use super::*;
use wayscriber::config::{SessionCompression, SessionStorageMode, StatusPosition};

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
fn board_mode_option_parses_known_values() {
    assert_eq!(
        BoardModeOption::from_str("whiteboard"),
        Some(BoardModeOption::Whiteboard)
    );
    assert_eq!(
        BoardModeOption::from_str("blackboard"),
        Some(BoardModeOption::Blackboard)
    );
    assert_eq!(BoardModeOption::from_str("invalid"), None);
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
