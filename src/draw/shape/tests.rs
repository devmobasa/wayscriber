use super::types::Shape;
use super::{EmbeddedImage, EraserBrush};
use crate::draw::{EraserKind, FontDescriptor, PolygonKind, StepMarkerLabel, color::WHITE};
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
        label: None,
    };

    let rect = shape.bounding_box().expect("arrow should have bounds");
    let x_min = rect.x;
    let x_max = rect.x + rect.width;
    let y_min = rect.y;
    let y_max = rect.y + rect.height;

    assert!(x_min <= 50 && x_max >= 100);
    assert!(y_min <= 100 && y_max >= 120);

    let geometry = util::calculate_arrowhead_triangle_custom(100, 100, 50, 120, 3.0, 20.0, 30.0)
        .expect("arrow geometry should exist");
    for (px, py) in [geometry.left, geometry.right] {
        assert!(px >= x_min as f64 && px <= x_max as f64);
        assert!(py >= y_min as f64 && py <= y_max as f64);
    }
}

#[test]
fn arrow_label_layout_offsets_from_line() {
    let font = FontDescriptor::default();
    let layout = super::arrow_label_layout(100, 0, 0, 0, 2.0, "1", 12.0, &font)
        .expect("label layout should exist");
    let center_x = layout.bounds.x + layout.bounds.width / 2;
    let center_y = layout.bounds.y + layout.bounds.height / 2;

    assert!(center_y > 0);
    assert!((center_x - 50).abs() <= 20);

    let layout = super::arrow_label_layout(0, 100, 0, 0, 2.0, "1", 12.0, &font)
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
            bytes: vec![1, 2, 3],
        },
    };

    let rect = shape.bounding_box().expect("image should have bounds");
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (12, 24, 80, 45));
    assert_eq!(shape.kind_name(), "Image");
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
            bytes: vec![1, 2, 3, 4],
        },
    };

    let json = serde_json::to_string(&shape).expect("serialize image shape");
    assert!(json.contains("\"bytes\":\"AQIDBA==\""));

    let restored: Shape = serde_json::from_str(&json).expect("deserialize image shape");
    match restored {
        Shape::Image { data, .. } => {
            assert_eq!(data.mime_type, "image/jpeg");
            assert_eq!(data.bytes, vec![1, 2, 3, 4]);
        }
        other => panic!("expected image shape, got {:?}", other),
    }
}
