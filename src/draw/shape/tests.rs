use super::types::Shape;
use super::{EmbeddedImage, EraserBrush};
use crate::draw::{
    ArrowLabel, ArrowStyle, EraserKind, FontDescriptor, PolygonKind, StepMarkerLabel,
    color::{BLACK, WHITE},
};
use crate::util;

#[test]
fn freehand_bounding_box_expands_with_thickness() {
    let shape = Shape::Freehand {
        points: vec![(10, 20), (30, 40)],
        color: WHITE,
        thick: 6.0,
    };

    let rect = shape.bounding_box().expect("freehand should have bounds");
    assert_eq!(rect.x, 7);
    assert_eq!(rect.y, 17);
    assert_eq!(rect.width, 26);
    assert_eq!(rect.height, 26);
}

#[test]
fn line_bounding_box_covers_stroke() {
    let shape = Shape::Line {
        x1: 50,
        y1: 40,
        x2: 70,
        y2: 90,
        color: WHITE,
        thick: 4.0,
    };

    let rect = shape.bounding_box().expect("line should have bounds");
    assert_eq!(rect.x, 48);
    assert_eq!(rect.y, 38);
    assert_eq!(rect.width, 24);
    assert_eq!(rect.height, 54);
}

#[test]
fn arrow_bounding_box_includes_head() {
    let shape = Shape::Arrow {
        x1: 100,
        y1: 100,
        x2: 50,
        y2: 120,
        color: WHITE,
        thick: 3.0,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        head_at_end: false,
        style: ArrowStyle::Standard,
        bend: 0.0,
        label: None,
    };

    let rect = shape.bounding_box().expect("arrow should have bounds");
    let x_min = rect.x;
    let x_max = rect.x + rect.width;
    let y_min = rect.y;
    let y_max = rect.y + rect.height;

    assert!(x_min <= 50 && x_max >= 100);
    assert!(y_min <= 100 && y_max >= 120);

    let skeleton = util::calculate_arrow_skeleton(
        100,
        100,
        50,
        120,
        3.0,
        20.0,
        30.0,
        ArrowStyle::Standard,
        0.0,
    )
    .expect("arrow geometry should exist");
    for (px, py) in [skeleton.head.left, skeleton.head.right] {
        assert!(px >= x_min as f64 && px <= x_max as f64);
        assert!(py >= y_min as f64 && py <= y_max as f64);
    }
}

#[test]
fn arrow_without_a_style_field_loads_as_standard_and_straight() {
    // The back-compat guarantee: every session written before styles existed
    // has arrows with no `style` and no `bend`. Drop `#[serde(default)]` from
    // either field and this stops deserializing at all.
    let shape: Shape = serde_json::from_str(
        r#"{"Arrow":{"x1":0,"y1":0,"x2":100,"y2":0,"color":{"r":1.0,"g":1.0,"b":1.0,"a":1.0},
            "thick":2.0,"arrow_length":20.0,"arrow_angle":30.0,"head_at_end":true}}"#,
    )
    .expect("historical arrow should deserialize");

    match shape {
        Shape::Arrow { style, bend, .. } => {
            assert_eq!(style, ArrowStyle::Standard);
            assert_eq!(bend, 0.0);
        }
        other => panic!("expected arrow shape, got {other:?}"),
    }
}

#[test]
fn arrow_style_and_bend_survive_a_serde_round_trip() {
    for style in ArrowStyle::ALL {
        let shape = Shape::Arrow {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 0,
            color: WHITE,
            thick: 2.0,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            head_at_end: true,
            style,
            bend: -0.35,
            label: None,
        };

        let json = serde_json::to_string(&shape).expect("serialize arrow");
        let restored: Shape = serde_json::from_str(&json).expect("deserialize arrow");
        match restored {
            Shape::Arrow {
                style: restored_style,
                bend,
                ..
            } => {
                assert_eq!(restored_style, style);
                assert_eq!(bend, -0.35);
            }
            other => panic!("expected arrow shape, got {other:?}"),
        }
    }
}

#[test]
fn curved_arrow_bounds_contain_the_arc_not_just_the_chord() {
    // The arc bulges outside the chord's box. An under-sized box leaves repaint
    // trails behind the shaft, so break this by unioning only the endpoints and
    // the head and the assertion below fails on the very first pixel of arc.
    let curved = Shape::Arrow {
        x1: 0,
        y1: 100,
        x2: 400,
        y2: 100,
        color: WHITE,
        thick: 4.0,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        head_at_end: true,
        style: ArrowStyle::Curved,
        bend: 0.5,
        label: None,
    };

    let rect = curved
        .bounding_box()
        .expect("curved arrow should have bounds");
    // Bend 0.5 over a 400px chord puts the arc's furthest point 100px off it,
    // at y = 0. The chord itself never leaves y = 100.
    assert!(
        rect.y <= 0,
        "bounds start at y = {} and clip the arc's bulge",
        rect.y
    );
    assert!(
        rect.y + rect.height >= 100,
        "bounds stop at y = {} and clip the chord",
        rect.y + rect.height
    );
}

#[test]
fn double_arrow_bounds_contain_the_second_head() {
    fn arrow(style: ArrowStyle) -> Shape {
        // Diagonal on purpose: on a horizontal arrow both heads project the
        // same vertical extent and the boxes coincide. Off-axis, the tail
        // head's barbs reach past the tail point the chord's box stops at.
        Shape::Arrow {
            x1: 20,
            y1: 20,
            x2: 200,
            y2: 200,
            color: WHITE,
            thick: 10.0,
            arrow_length: 20.0,
            arrow_angle: 60.0,
            head_at_end: true,
            style,
            bend: 0.0,
            label: None,
        }
    }

    let double_rect = arrow(ArrowStyle::Double)
        .bounding_box()
        .expect("double arrow bounds");
    let standard_rect = arrow(ArrowStyle::Standard)
        .bounding_box()
        .expect("standard arrow bounds");
    // The tail head's barbs stick out past the tapered tail a standard arrow
    // ends in, so the box has to grow in both directions.
    assert!(
        double_rect.x < standard_rect.x && double_rect.y < standard_rect.y,
        "double bounds {double_rect:?} do not cover the tail head; standard was {standard_rect:?}"
    );
}

#[test]
fn arrow_label_layout_offsets_from_line() {
    let font = FontDescriptor::default();
    let layout = super::arrow_label_layout(100, 0, 0, 0, 2.0, 0.0, "1", 12.0, &font)
        .expect("label layout should exist");
    let center_x = layout.bounds.x + layout.bounds.width / 2;
    let center_y = layout.bounds.y + layout.bounds.height / 2;

    assert!(center_y > 0);
    assert!((center_x - 50).abs() <= 20);

    let layout = super::arrow_label_layout(0, 100, 0, 0, 2.0, 0.0, "1", 12.0, &font)
        .expect("label layout should exist");
    let center_x = layout.bounds.x + layout.bounds.width / 2;
    let center_y = layout.bounds.y + layout.bounds.height / 2;

    assert!(center_x < 0);
    assert!((center_y - 50).abs() <= 20);
}

#[test]
fn ellipse_bounding_box_handles_radii_and_stroke() {
    let shape = Shape::Ellipse {
        cx: 200,
        cy: 150,
        rx: 40,
        ry: 20,
        fill: false,
        color: WHITE,
        thick: 2.0,
    };

    let rect = shape.bounding_box().expect("ellipse should have bounds");
    assert_eq!(rect.x, 159);
    assert_eq!(rect.y, 129);
    assert_eq!(rect.width, 82);
    assert_eq!(rect.height, 42);
}

#[test]
fn historical_spotlight_without_magnification_loads_as_one_x() {
    let shape: Shape = serde_json::from_str(r#"{"Spotlight":{"cx":120,"cy":80,"rx":40,"ry":20}}"#)
        .expect("historical spotlight should deserialize");

    match shape {
        Shape::Spotlight { magnification, .. } => assert_eq!(magnification, 1.0),
        other => panic!("expected spotlight shape, got {other:?}"),
    }
}

#[test]
fn persisted_spotlight_magnification_is_normalized_on_load() {
    for (json, expected) in [
        (
            r#"{"Spotlight":{"cx":120,"cy":80,"rx":40,"ry":20,"magnification":9.0}}"#,
            4.0,
        ),
        (
            r#"{"Spotlight":{"cx":120,"cy":80,"rx":40,"ry":20,"magnification":0.5}}"#,
            1.0,
        ),
    ] {
        let shape: Shape = serde_json::from_str(json).expect("spotlight should deserialize");
        match shape {
            Shape::Spotlight { magnification, .. } => assert_eq!(magnification, expected),
            other => panic!("expected spotlight shape, got {other:?}"),
        }
    }
}

#[test]
fn polygon_bounding_box_covers_vertices_and_stroke() {
    let shape = Shape::Polygon {
        kind: PolygonKind::Triangle,
        points: vec![(10, 20), (30, 40), (5, 35)],
        fill: false,
        color: WHITE,
        thick: 6.0,
    };

    let rect = shape.bounding_box().expect("polygon should have bounds");
    assert_eq!(rect.x, 2);
    assert_eq!(rect.y, 17);
    assert_eq!(rect.width, 31);
    assert_eq!(rect.height, 26);
}

#[test]
fn polygon_shape_serializes_and_deserializes_with_points() {
    let shape = Shape::Polygon {
        kind: PolygonKind::Regular { sides: 6 },
        points: vec![(10, 20), (30, 20), (40, 35), (30, 50), (10, 50), (0, 35)],
        fill: true,
        color: WHITE,
        thick: 4.0,
    };

    let json = serde_json::to_string(&shape).expect("serialize polygon shape");
    let restored: Shape = serde_json::from_str(&json).expect("deserialize polygon shape");

    match restored {
        Shape::Polygon {
            kind,
            points,
            fill,
            color,
            thick,
        } => {
            assert_eq!(kind, PolygonKind::Regular { sides: 6 });
            assert_eq!(
                points,
                vec![(10, 20), (30, 20), (40, 35), (30, 50), (10, 50), (0, 35)]
            );
            assert!(fill);
            assert_eq!(color, WHITE);
            assert_eq!(thick, 4.0);
        }
        other => panic!("expected polygon shape, got {other:?}"),
    }
}

#[test]
fn invalid_polygon_has_no_bounds() {
    let shape = Shape::Polygon {
        kind: PolygonKind::Freeform,
        points: vec![(10, 20), (10, 20), (30, 40)],
        fill: false,
        color: WHITE,
        thick: 6.0,
    };

    assert!(shape.bounding_box().is_none());
}

#[test]
fn text_bounding_box_is_non_zero() {
    let shape = Shape::Text {
        x: 10,
        y: 20,
        text: "Hello".to_string(),
        color: WHITE,
        size: 24.0,
        font_descriptor: FontDescriptor::default(),
        background_enabled: true,
        wrap_width: None,
    };

    let rect = shape.bounding_box().expect("text should have bounds");
    assert!(rect.width > 0);
    assert!(rect.height > 0);
    assert!(rect.x <= 10);
    assert!(rect.y <= 20);
}

#[test]
fn sticky_note_bounding_box_is_non_zero() {
    let shape = Shape::StickyNote {
        x: 10,
        y: 20,
        text: "Note".to_string(),
        background: WHITE,
        size: 24.0,
        font_descriptor: FontDescriptor::default(),
        wrap_width: None,
    };

    let rect = shape
        .bounding_box()
        .expect("sticky note should have bounds");
    assert!(rect.width > 0);
    assert!(rect.height > 0);
    assert!(rect.x <= 10);
    assert!(rect.y <= 20);
}

#[test]
fn step_marker_bounding_box_is_square_and_contains_center() {
    let font = FontDescriptor::default();
    let shape = Shape::StepMarker {
        x: 120,
        y: 80,
        color: WHITE,
        label: StepMarkerLabel {
            value: 7,
            size: 18.0,
            font_descriptor: font,
        },
    };

    let rect = shape
        .bounding_box()
        .expect("step marker should have bounds");
    assert!(rect.width > 0);
    assert_eq!(rect.width, rect.height);
    assert!(
        rect.contains(120, 80),
        "step marker bounds should include center point"
    );
}

#[test]
fn marker_bounding_box_uses_inflated_thickness() {
    let shape = Shape::MarkerStroke {
        points: vec![(0, 0), (10, 0)],
        color: WHITE,
        thick: 4.0,
    };

    let rect = shape.bounding_box().expect("marker should have bounds");
    assert_eq!(rect.x, -3);
    assert_eq!(rect.y, -3);
    assert_eq!(rect.width, 16);
    assert_eq!(rect.height, 6);
}

#[test]
fn eraser_bounding_box_tracks_diameter() {
    let shape = Shape::EraserStroke {
        points: vec![(5, 5), (5, 5)],
        brush: EraserBrush {
            size: 6.0,
            kind: EraserKind::Circle,
        },
    };

    let rect = shape.bounding_box().expect("eraser should have bounds");
    assert_eq!(rect.x, 2);
    assert_eq!(rect.y, 2);
    assert_eq!(rect.width, 6);
    assert_eq!(rect.height, 6);
}

#[test]
fn image_bounding_box_and_kind_name_use_display_bounds() {
    let shape = Shape::Image {
        x: 12,
        y: 24,
        w: 80,
        h: 45,
        data: EmbeddedImage {
            mime_type: "image/png".to_string(),
            width: 2,
            height: 1,
            bytes: vec![1, 2, 3].into(),
        },
    };

    let rect = shape.bounding_box().expect("image should have bounds");
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (12, 24, 80, 45));
    assert_eq!(shape.kind_name(), "Image");
}

#[test]
fn pressure_and_image_bounds_handle_extreme_coordinates() {
    let pressure = Shape::FreehandPressure {
        points: vec![(i32::MAX, i32::MAX, 2.0)],
        color: WHITE,
    };
    let pressure_bounds = pressure
        .bounding_box()
        .expect("edge pressure point should retain visible bounds");
    assert!(pressure_bounds.contains(i32::MAX, i32::MAX));

    let image = Shape::Image {
        x: i32::MAX,
        y: i32::MAX,
        w: i32::MAX,
        h: i32::MAX,
        data: EmbeddedImage {
            mime_type: "image/png".to_string(),
            width: 1,
            height: 1,
            bytes: vec![1].into(),
        },
    };
    let image_bounds = image
        .bounding_box()
        .expect("edge image should retain visible bounds");
    assert!(image_bounds.contains(i32::MAX, i32::MAX));
}

#[test]
fn curved_arrow_label_follows_the_arc_not_the_chord() {
    // Anchored to the chord, the label would sit in the gap the arrow was drawn
    // to route around - far from the shaft it numbers.
    let font = FontDescriptor::default();
    // Tail at (0, 100), tip at (400, 100), bulging up by 0.5 * 400 / 2 = 100px.
    let straight = super::arrow_label_layout(400, 100, 0, 100, 4.0, 0.0, "1", 12.0, &font)
        .expect("straight label layout");
    let curved = super::arrow_label_layout(400, 100, 0, 100, 4.0, 0.5, "1", 12.0, &font)
        .expect("curved label layout");

    // Both sit at the middle of the span horizontally.
    assert_eq!(straight.bounds.x, curved.bounds.x);
    // The curved one tracks the arc's midpoint at y = 0, on the outside of the
    // bend; the straight one stays beside the chord at y = 100.
    assert!(
        curved.bounds.y < straight.bounds.y - 90,
        "curved label at y = {} did not follow the arc from y = {}",
        curved.bounds.y,
        straight.bounds.y
    );
}

/// A labelled arrow along y = 100 from (0, 100) to (400, 100).
fn labelled_arrow(style: ArrowStyle, head_at_end: bool) -> Shape {
    Shape::Arrow {
        x1: 0,
        y1: 100,
        x2: 400,
        y2: 100,
        color: BLACK,
        thick: 4.0,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        head_at_end,
        style,
        bend: 0.0,
        label: Some(ArrowLabel {
            value: 7,
            size: 12.0,
            font_descriptor: FontDescriptor::default(),
        }),
    }
}

#[test]
fn flipping_the_head_does_not_move_a_double_arrow_label() {
    // docs/CONFIG.md states head_at_end has no effect on Double, and the
    // outline honours that — the polygon is the same either way. The label was
    // the hole in the contract: anchored to the head's reading of the
    // endpoints, toggling Arrow Head mirrored the number across a shaft that
    // had not moved, and took its bounds and hit area with it.
    let at_end = labelled_arrow(ArrowStyle::Double, true).bounding_box();
    let at_start = labelled_arrow(ArrowStyle::Double, false).bounding_box();
    assert_eq!(
        at_end, at_start,
        "flipping the head moved a double arrow's label"
    );
}

#[test]
fn flipping_the_head_still_moves_a_single_headed_arrow_label() {
    // The normalization is Double-only. For every other style the head really
    // does pick an end, and the label follows it — flattening that would put
    // the number on the wrong side of a reversed arrow.
    for style in [ArrowStyle::Standard, ArrowStyle::Pointy, ArrowStyle::Curved] {
        let at_end = labelled_arrow(style, true).bounding_box();
        let at_start = labelled_arrow(style, false).bounding_box();
        assert_ne!(at_end, at_start, "{style:?} label ignored head_at_end");
    }
}

#[test]
fn arrow_label_layout_handles_full_span_endpoints() {
    let font = FontDescriptor::default();
    let layout = super::arrow_label_layout(
        i32::MAX,
        i32::MAX,
        i32::MIN,
        i32::MIN,
        2.0,
        0.0,
        "1",
        12.0,
        &font,
    )
    .expect("extreme arrow endpoints should not overflow label geometry");

    assert!(layout.bounds.is_valid());
}

#[test]
fn image_serialization_uses_base64_bytes() {
    let shape = Shape::Image {
        x: 1,
        y: 2,
        w: 3,
        h: 4,
        data: EmbeddedImage {
            mime_type: "image/jpeg".to_string(),
            width: 3,
            height: 4,
            bytes: vec![1, 2, 3, 4].into(),
        },
    };

    let json = serde_json::to_string(&shape).expect("serialize image shape");
    assert!(json.contains("\"bytes\":\"AQIDBA==\""));

    let restored: Shape = serde_json::from_str(&json).expect("deserialize image shape");
    match restored {
        Shape::Image { data, .. } => {
            assert_eq!(data.mime_type, "image/jpeg");
            assert_eq!(data.bytes.as_ref(), [1, 2, 3, 4]);
        }
        other => panic!("expected image shape, got {:?}", other),
    }
}

#[test]
fn embedded_image_clones_share_the_encoded_payload() {
    let image = EmbeddedImage {
        mime_type: "image/png".to_string(),
        width: 1,
        height: 1,
        bytes: vec![1, 2, 3, 4].into(),
    };

    let cloned = image.clone();

    assert!(std::sync::Arc::ptr_eq(&image.bytes, &cloned.bytes));
}
