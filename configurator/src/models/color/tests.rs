use super::*;
use wayscriber::config::enums::ColorSpec;

#[test]
fn color_input_named_round_trip_and_preview() {
    let spec = ColorSpec::Name("red".to_string());
    let input = ColorInput::from_color(&spec);

    assert_eq!(input.mode, ColorMode::Named);
    assert_eq!(input.selected_named, NamedColorOption::Red);
    assert_eq!(input.summary(), "Red");

    let preview = input.preview_color().expect("preview should resolve");
    assert!((preview.r - 1.0).abs() < f32::EPSILON);
    assert!((preview.g - 0.0).abs() < f32::EPSILON);
    assert!((preview.b - 0.0).abs() < f32::EPSILON);

    let round_trip = input.to_color_spec().expect("to_color_spec should succeed");
    match round_trip {
        ColorSpec::Name(name) => assert_eq!(name, "red"),
        _ => panic!("expected named color"),
    }
}

#[test]
fn color_input_custom_name_requires_value() {
    let input = ColorInput {
        mode: ColorMode::Named,
        name: "   ".to_string(),
        rgb: ["0".to_string(), "0".to_string(), "0".to_string()],
        selected_named: NamedColorOption::Custom,
    };

    let err = input.to_color_spec().expect_err("expected error");
    assert_eq!(err.field, "drawing.default_color");
}

#[test]
fn color_input_rgb_rejects_out_of_range_component() {
    let input = ColorInput {
        mode: ColorMode::Rgb,
        name: String::new(),
        rgb: ["255".to_string(), "0".to_string(), "300".to_string()],
        selected_named: NamedColorOption::Custom,
    };

    let err = input.to_color_spec().expect_err("expected error");
    assert_eq!(err.field, "drawing.default_color[2]");
    assert!(err.message.contains("between 0 and 255"));
}

#[test]
fn color_input_rgb_rejects_negative_component() {
    let input = ColorInput {
        mode: ColorMode::Rgb,
        name: String::new(),
        rgb: ["-1".to_string(), "10".to_string(), "20".to_string()],
        selected_named: NamedColorOption::Custom,
    };

    let err = input.to_color_spec().expect_err("expected error");
    assert_eq!(err.field, "drawing.default_color[0]");
    assert!(err.message.contains("between 0 and 255"));
}

#[test]
fn color_input_rgb_rejects_non_integer_component() {
    let input = ColorInput {
        mode: ColorMode::Rgb,
        name: String::new(),
        rgb: ["12.5".to_string(), "10".to_string(), "20".to_string()],
        selected_named: NamedColorOption::Custom,
    };

    let err = input.to_color_spec().expect_err("expected error");
    assert_eq!(err.field, "drawing.default_color[0]");
    assert!(err.message.contains("Expected integer"));
}

#[test]
fn color_input_preview_rgb_rejects_out_of_range() {
    let input = ColorInput {
        mode: ColorMode::Rgb,
        name: String::new(),
        rgb: ["256".to_string(), "0".to_string(), "0".to_string()],
        selected_named: NamedColorOption::Custom,
    };

    assert!(
        input.preview_color().is_none(),
        "preview should be None for out-of-range component"
    );
}

#[test]
fn color_triplet_input_reports_invalid_component() {
    let input = ColorTripletInput {
        components: ["0.1".to_string(), "oops".to_string(), "0.3".to_string()],
    };

    let err = input
        .to_array("board.whiteboard_color")
        .expect_err("expected error");
    assert_eq!(err.field, "board.whiteboard_color[1]");
}

#[test]
fn color_quad_input_summary_trims_components() {
    let input = ColorQuadInput {
        components: [
            " 0.1 ".to_string(),
            "0.2".to_string(),
            " 0.3".to_string(),
            "0.4 ".to_string(),
        ],
    };

    assert_eq!(input.summary(), "0.1, 0.2, 0.3, 0.4");
}
