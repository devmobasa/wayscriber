//! Load-policy contracts, inventoried independently against the declaration-derived schema.
use super::{Config, ConfigDocument};
use toml::Value;

#[derive(Clone)]
struct Case {
    path: String,
    fallback: Option<f64>,
    bounds: Option<(f64, f64)>,
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (path, default, min, max) in [
        ("drawing.default_thickness", 3.0, 1.0, 50.0),
        ("drawing.default_eraser_size", 12.0, 1.0, 50.0),
        ("drawing.marker_opacity", 0.32, 0.05, 0.9),
        ("drawing.default_font_size", 32.0, 8.0, 72.0),
        ("drawing.hit_test_tolerance", 6.0, 1.0, 20.0),
        ("arrow.length", 20.0, 5.0, 50.0),
        ("arrow.angle_degrees", 26.0, 15.0, 60.0),
        ("spotlight.dim_opacity", 0.6, 0.1, 0.95),
        ("spotlight.feather", 0.35, 0.0, 0.9),
        ("spotlight.magnification", 1.0, 1.0, 4.0),
        ("laser.width", 6.0, 2.0, 30.0),
        ("ui.toolbar.scale", 1.0, 0.5, 3.0),
        ("ui.click_highlight.radius", 24.0, 16.0, 160.0),
        ("ui.click_highlight.outline_thickness", 4.0, 1.0, 12.0),
        ("ui.input_hud.font_size", 18.0, 6.0, 72.0),
        ("export.pdf.custom_width", 800.0, 1.0, 14_400.0),
        ("export.pdf.custom_height", 600.0, 1.0, 14_400.0),
        ("export.pdf.content_source_padding", 24.0, 0.0, 4_096.0),
        ("export.pdf.labels.font_size", 10.0, 1.0, 72.0),
        ("export.pdf.labels.margin", 12.0, 0.0, 240.0),
        ("export.pdf.labels.padding_x", 6.0, 0.0, 120.0),
        ("export.pdf.labels.padding_y", 3.0, 0.0, 120.0),
    ] {
        cases.push(Case {
            path: path.into(),
            fallback: Some(default),
            bounds: Some((min, max)),
        });
    }
    #[cfg(feature = "tablet-input")]
    for (field, default, min, max) in [
        ("min_thickness", 1.0, 1.0, 50.0),
        ("max_thickness", 8.0, 1.0, 50.0),
        ("pressure_variation_threshold", 0.1, 0.0, f64::MAX),
        ("pressure_thickness_scale_step", 0.1, 0.0, 1.0),
    ] {
        cases.push(Case {
            path: format!("tablet.{field}"),
            fallback: Some(default),
            bounds: Some((min, max)),
        });
    }
    for (path, default) in [
        ("ui.toolbar.top_offset", 0.0),
        ("ui.toolbar.top_offset_y", 0.0),
        ("ui.status_bar_style.font_size", 15.0),
        ("ui.status_bar_style.padding", 11.0),
        ("ui.status_bar_style.dot_radius", 6.0),
        ("ui.help_overlay_style.font_size", 14.0),
        ("ui.help_overlay_style.line_height", 22.0),
        ("ui.help_overlay_style.padding", 32.0),
        ("ui.help_overlay_style.border_width", 2.0),
    ] {
        cases.push(Case {
            path: path.into(),
            fallback: Some(default),
            bounds: None,
        });
    }
    for (path, defaults, bounded) in [
        ("board.whiteboard_color", vec![0.992; 3], true),
        ("board.blackboard_color", vec![0.067; 3], true),
        (
            "board.whiteboard_pen_color",
            vec![36.0 / 255.0, 31.0 / 255.0, 49.0 / 255.0],
            true,
        ),
        ("board.blackboard_pen_color", vec![1.0; 3], true),
        ("boards.items.0.background", vec![0.0; 3], true),
        ("boards.items.0.background.rgb", vec![0.0; 3], true),
        ("boards.items.0.default_pen_color", vec![0.0; 3], true),
        ("boards.items.0.default_pen_color.rgb", vec![0.0; 3], true),
        ("laser.color", vec![1.0, 0.16, 0.12, 1.0], true),
        (
            "ui.click_highlight.fill_color",
            vec![1.0, 0.8, 0.0, 0.35],
            true,
        ),
        (
            "ui.click_highlight.outline_color",
            vec![1.0, 0.6, 0.0, 0.9],
            true,
        ),
        (
            "ui.status_bar_style.bg_color",
            vec![0.0, 0.0, 0.0, 0.85],
            false,
        ),
        ("ui.status_bar_style.text_color", vec![1.0; 4], false),
        (
            "ui.help_overlay_style.bg_color",
            vec![0.09, 0.1, 0.13, 1.0],
            false,
        ),
        (
            "ui.help_overlay_style.border_color",
            vec![0.33, 0.39, 0.52, 0.88],
            false,
        ),
        (
            "ui.help_overlay_style.text_color",
            vec![0.95, 0.96, 0.98, 1.0],
            false,
        ),
        (
            "export.pdf.labels.text_color",
            vec![0.1, 0.1, 0.1, 1.0],
            true,
        ),
        (
            "export.pdf.labels.background_color",
            vec![1.0, 1.0, 1.0, 0.85],
            true,
        ),
    ] {
        for (index, default) in defaults.into_iter().enumerate() {
            cases.push(Case {
                path: format!("{path}.{index}"),
                fallback: Some(default),
                bounds: bounded.then_some((0.0, 1.0)),
            });
        }
    }
    for slot in 1..=5 {
        let prefix = format!("presets.slot_{slot}");
        for field in [
            "size",
            "tool_settings.pen.size",
            "tool_settings.line.size",
            "tool_settings.rect.size",
            "tool_settings.ellipse.size",
            "tool_settings.arrow.size",
            "tool_settings.blur.size",
            "tool_settings.marker.size",
            "tool_settings.step_marker.size",
            "tool_settings.eraser_size",
        ] {
            cases.push(Case {
                path: format!("{prefix}.{field}"),
                fallback: Some(1.0),
                bounds: Some((1.0, 50.0)),
            });
        }
        for (field, min, max) in [
            ("marker_opacity", 0.05, 0.9),
            ("font_size", 8.0, 72.0),
            ("arrow_length", 5.0, 50.0),
            ("arrow_angle", 15.0, 60.0),
        ] {
            cases.push(Case {
                path: format!("{prefix}.{field}"),
                fallback: None,
                bounds: Some((min, max)),
            });
        }
    }
    cases
}

fn set(value: &mut Value, path: &str, replacement: Value) {
    let (head, tail) = path.split_once('.').unwrap_or((path, ""));
    let child = if let Ok(index) = head.parse::<usize>() {
        &mut value.as_array_mut().unwrap()[index]
    } else {
        value
            .as_table_mut()
            .unwrap()
            .entry(head.to_string())
            .or_insert_with(|| Value::Table(Default::default()))
    };
    if tail.is_empty() {
        *child = replacement;
    } else {
        set(child, tail, replacement);
    }
}

fn get<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |value, part| {
        if let Ok(index) = part.parse::<usize>() {
            value.as_array()?.get(index)
        } else {
            value.get(part)
        }
    })
}

fn fixture(path: &str) -> Value {
    let mut value = Value::try_from(Config::default()).unwrap();
    if path.starts_with("presets.") {
        let slot = path.split('.').nth(1).unwrap();
        let preset: Value = toml::from_str("tool = 'pen'\ncolor = 'red'\nsize = 3.0").unwrap();
        set(&mut value, &format!("presets.{slot}"), preset);
        for tool in [
            "pen",
            "line",
            "rect",
            "ellipse",
            "arrow",
            "blur",
            "marker",
            "step_marker",
        ] {
            set(
                &mut value,
                &format!("presets.{slot}.tool_settings.{tool}"),
                toml::from_str("color = 'red'\nsize = 3.0").unwrap(),
            );
        }
        set(
            &mut value,
            &format!("presets.{slot}.tool_settings.eraser_size"),
            Value::Float(12.0),
        );
    }
    if path.starts_with("boards.") {
        let boards: Value = toml::from_str("[[items]]\nid = 'sample'\nname = 'Sample'\nbackground = [0.5, 0.5, 0.5]\ndefault_pen_color = [0.5, 0.5, 0.5]\n[[items]]\nid = 'transparent'\nname = 'Overlay'\nbackground = 'transparent'").unwrap();
        set(&mut value, "boards", boards);
        if let Some((base, _)) = path.split_once(".rgb.") {
            set(
                &mut value,
                base,
                toml::from_str("rgb = [0.5, 0.5, 0.5]").unwrap(),
            );
        }
    }
    // Isolate scalar bounds from the tablet min <= max relationship, tested separately.
    if path == "tablet.min_thickness" {
        set(&mut value, "tablet.max_thickness", Value::Float(50.0));
    }
    value
}

#[test]
fn document_load_enforces_finite_policy_for_every_float_and_array_component() {
    let temp = crate::test_temp::tempdir().unwrap();
    let file = temp.path().join("config.toml");
    for case in cases() {
        let mut inputs = vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
        if let Some((min, max)) = case.bounds {
            inputs.extend([min.next_down(), min, min.next_up(), max.next_down(), max]);
            if max < f64::MAX {
                inputs.push(max.next_up());
            }
        } else {
            inputs.extend([-1.0, 0.0, 1.0, 1e9]);
        }
        for input in inputs {
            let mut raw = fixture(&case.path);
            set(&mut raw, &case.path, Value::Float(input));
            let source = toml::to_string(&raw).unwrap();
            std::fs::write(&file, &source).unwrap();
            let document = ConfigDocument::load_from_path(&file).unwrap();
            assert!(
                document.section_errors().is_empty(),
                "{}: {:?}",
                case.path,
                document.source()
            );
            let loaded = Value::try_from(document.config()).unwrap();
            // Board RGB maps normalize to the canonical array representation.
            let output_path = case.path.replace(".rgb.", ".");
            let actual = get(&loaded, &output_path).and_then(Value::as_float);
            let expected = if !input.is_finite() {
                case.fallback
            } else {
                Some(match case.bounds {
                    Some((min, _)) if input < min => min,
                    Some((_, max)) if input > max => max,
                    _ => input,
                })
            };
            assert_eq!(actual, expected, "{} input={input}", case.path);
            assert_eq!(
                std::fs::read_to_string(&file).unwrap(),
                source,
                "load must not rewrite authored non-finite values"
            );
        }
    }
}

#[cfg(feature = "tablet-input")]
#[test]
fn tablet_normalizes_nonfinite_values_before_ordering_and_clamping() {
    let temp = crate::test_temp::tempdir().unwrap();
    let file = temp.path().join("config.toml");
    for (min, max, expected) in [
        ("nan", "inf", (1.0, 8.0)),
        ("60.0", "-10.0", (1.0, 50.0)),
        ("45.0", "nan", (8.0, 45.0)),
    ] {
        std::fs::write(
            &file,
            format!("[tablet]\nmin_thickness = {min}\nmax_thickness = {max}\n"),
        )
        .unwrap();
        let document = ConfigDocument::load_from_path(&file).unwrap();
        let tablet = &document.config().tablet;
        assert_eq!((tablet.min_thickness, tablet.max_thickness), expected);
    }
}

#[test]
fn authored_save_rejects_nonfinite_float_with_a_field_level_correction() {
    let mut edited = Config::default();
    edited.drawing.default_thickness = f64::NAN;
    let Err(crate::config::SaveValidationError::CorrectedValues(corrections)) =
        edited.validate_for_save()
    else {
        panic!("authored save must report the invalid numeric input");
    };
    assert_eq!(corrections.len(), 1);
    assert_eq!(corrections[0].path, "drawing.default_thickness");
    assert!(
        corrections[0]
            .authored
            .as_ref()
            .and_then(Value::as_float)
            .unwrap()
            .is_nan()
    );
    assert_eq!(corrections[0].corrected, Some(Value::Float(3.0)));
}

#[cfg(feature = "config-schema")]
#[test]
fn float_load_cases_cover_the_independent_schema_inventory() {
    use serde_json::Value as Json;
    use std::collections::BTreeSet;

    fn inventory(root: &Json, node: &Json, path: &str, found: &mut BTreeSet<String>) {
        if let Some(reference) = node.get("$ref").and_then(Json::as_str) {
            inventory(
                root,
                root.pointer(reference.strip_prefix('#').unwrap()).unwrap(),
                path,
                found,
            );
            return;
        }
        let kind = node.get("type");
        if kind == Some(&Json::String("number".into()))
            || kind
                .and_then(Json::as_array)
                .is_some_and(|types| types.iter().any(|kind| kind == "number"))
        {
            found.insert(path.into());
        }
        if let Some(properties) = node.get("properties").and_then(Json::as_object) {
            for (key, child) in properties {
                let next = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                inventory(root, child, &next, found);
            }
        }
        // A newly introduced map can be empty in defaults and fixtures too.
        if let Some(values) = node
            .get("additionalProperties")
            .filter(|value| value.is_object())
        {
            inventory(root, values, &format!("{path}.{{key}}"), found);
        }
        if let Some(patterns) = node.get("patternProperties").and_then(Json::as_object) {
            for values in patterns.values() {
                inventory(root, values, &format!("{path}.{{key}}"), found);
            }
        }
        if let Some(items) = node.get("prefixItems").and_then(Json::as_array) {
            for (index, child) in items.iter().enumerate() {
                inventory(root, child, &format!("{path}.{index}"), found);
            }
        } else if let Some(items) = node.get("items") {
            let count = node.get("minItems").and_then(Json::as_u64).unwrap_or(1);
            for index in 0..count {
                inventory(root, items, &format!("{path}.{index}"), found);
            }
        }
        for branch in ["anyOf", "oneOf", "allOf"] {
            if let Some(children) = node.get(branch).and_then(Json::as_array) {
                for child in children {
                    inventory(root, child, path, found);
                }
            }
        }
    }

    let schema = Config::json_schema();
    let mut declared = BTreeSet::new();
    inventory(&schema, &schema, "", &mut declared);
    let exercised: BTreeSet<_> = cases().into_iter().map(|case| case.path).collect();
    assert!(!declared.is_empty());
    assert_eq!(
        declared, exercised,
        "new floats (including absent optionals and collections) require load-policy cases"
    );
}
