use super::*;
use crate::draw::{
    ArrowLabel, ArrowStyle, BLACK, BlurStyle, EmbeddedImage, EraserBrush, EraserKind,
    FontDescriptor, PolygonKind, StepMarkerLabel, WHITE,
};
use std::sync::Arc;

fn font() -> FontDescriptor {
    FontDescriptor::new(
        "Test Sans".to_string(),
        "bold".to_string(),
        "italic".to_string(),
    )
}

#[test]
fn every_shape_variant_translates_its_bounds() {
    let font = font();
    let image = EmbeddedImage {
        mime_type: "image/png".to_string(),
        width: 2,
        height: 3,
        bytes: Arc::from([1, 2, 3, 4]),
    };
    let mut shapes = vec![
        Shape::Freehand {
            points: vec![(10, 20), (30, 40)],
            color: WHITE,
            thick: 2.0,
        },
        Shape::FreehandPressure {
            points: vec![(10, 20, 1.5), (30, 40, 2.5)],
            color: WHITE,
        },
        Shape::Line {
            x1: 10,
            y1: 20,
            x2: 30,
            y2: 40,
            color: WHITE,
            thick: 2.0,
        },
        Shape::Rect {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
            fill: false,
            color: WHITE,
            thick: 2.0,
        },
        Shape::Ellipse {
            cx: 20,
            cy: 30,
            rx: 10,
            ry: 15,
            fill: false,
            color: WHITE,
            thick: 2.0,
        },
        Shape::Polygon {
            kind: PolygonKind::Freeform,
            points: vec![(10, 20), (30, 20), (20, 40)],
            fill: true,
            color: WHITE,
            thick: 2.0,
        },
        Shape::Arrow {
            x1: 10,
            y1: 20,
            x2: 80,
            y2: 40,
            color: WHITE,
            thick: 2.0,
            arrow_length: 12.0,
            arrow_angle: 30.0,
            head_at_end: true,
            style: ArrowStyle::Curved,
            bend: 0.2,
            label: None,
        },
        Shape::BlurRect {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
            strength: 8.0,
            style: BlurStyle::Secure,
        },
        Shape::Spotlight {
            cx: 20,
            cy: 30,
            rx: 10,
            ry: 15,
            magnification: 1.5,
        },
        Shape::StepMarker {
            x: 20,
            y: 30,
            color: WHITE,
            label: StepMarkerLabel {
                value: 2,
                size: 14.0,
                font_descriptor: font.clone(),
            },
        },
        Shape::Text {
            x: 10,
            y: 40,
            text: "text".to_string(),
            color: WHITE,
            size: 14.0,
            font_descriptor: font.clone(),
            background_enabled: true,
            wrap_width: Some(80),
        },
        Shape::StickyNote {
            x: 10,
            y: 40,
            text: "note".to_string(),
            background: BLACK,
            size: 14.0,
            font_descriptor: font,
            wrap_width: Some(80),
        },
        Shape::MarkerStroke {
            points: vec![(10, 20), (30, 40)],
            color: WHITE,
            thick: 8.0,
        },
        Shape::EraserStroke {
            points: vec![(10, 20), (30, 40)],
            brush: EraserBrush {
                size: 8.0,
                kind: EraserKind::Rect,
            },
        },
        Shape::Image {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
            data: image,
        },
    ];

    for shape in &mut shapes {
        let before = shape
            .bounding_box()
            .unwrap_or_else(|| panic!("{} should have bounds", shape.kind_name()));
        shape.translate(17, -9);
        let after = shape
            .bounding_box()
            .unwrap_or_else(|| panic!("{} should keep bounds", shape.kind_name()));
        assert_eq!(
            after,
            crate::util::Rect::new(before.x + 17, before.y - 9, before.width, before.height)
                .expect("translated bounds"),
            "{} translated incorrectly",
            shape.kind_name()
        );
    }
}

#[test]
fn non_uniform_scale_preserves_arrow_metadata_and_scales_curvature() {
    let original = Shape::Arrow {
        x1: 10,
        y1: 20,
        x2: 110,
        y2: 20,
        color: WHITE,
        thick: 4.0,
        arrow_length: 18.0,
        arrow_angle: 35.0,
        head_at_end: false,
        style: ArrowStyle::Curved,
        bend: 0.25,
        label: Some(ArrowLabel {
            value: 7,
            size: 15.0,
            font_descriptor: font(),
        }),
    };

    let scaled = original.scaled(2.0, 3.0, 10.0, 20.0);
    match scaled {
        Shape::Arrow {
            x1,
            y1,
            x2,
            y2,
            color,
            thick,
            arrow_length,
            arrow_angle,
            head_at_end,
            style,
            bend,
            label: Some(label),
        } => {
            assert_eq!((x1, y1, x2, y2), (10, 20, 210, 20));
            assert_eq!(color, WHITE);
            assert_eq!(thick, 4.0);
            assert_eq!(arrow_length, 18.0);
            assert_eq!(arrow_angle, 35.0);
            assert!(!head_at_end);
            assert_eq!(style, ArrowStyle::Curved);
            assert!(bend > 0.25, "vertical scaling should increase the arc");
            assert_eq!(label.value, 7);
            assert_eq!(label.size, 15.0);
            assert_eq!(label.font_descriptor, font());
        }
        other => panic!("expected curved arrow, got {other:?}"),
    }
}

#[test]
fn scale_preserves_embedded_and_text_payloads() {
    let bytes: Arc<[u8]> = Arc::from([9, 8, 7, 6]);
    let image = Shape::Image {
        x: 10,
        y: 20,
        w: 30,
        h: 40,
        data: EmbeddedImage {
            mime_type: "image/png".to_string(),
            width: 2,
            height: 2,
            bytes: bytes.clone(),
        },
    }
    .scaled(2.0, 0.5, 0.0, 0.0);
    match image {
        Shape::Image { x, y, w, h, data } => {
            assert_eq!((x, y, w, h), (20, 10, 60, 20));
            assert_eq!(data.mime_type, "image/png");
            assert_eq!(data.bytes, bytes);
        }
        other => panic!("expected image, got {other:?}"),
    }

    let descriptor = font();
    let text = Shape::Text {
        x: 10,
        y: 20,
        text: "keep me".to_string(),
        color: WHITE,
        size: 14.0,
        font_descriptor: descriptor.clone(),
        background_enabled: true,
        wrap_width: Some(90),
    }
    .scaled(2.0, 0.5, 0.0, 0.0);
    match text {
        Shape::Text {
            x,
            y,
            text,
            size,
            font_descriptor,
            background_enabled,
            wrap_width,
            ..
        } => {
            assert_eq!((x, y), (20, 10));
            assert_eq!(text, "keep me");
            assert_eq!(size, 14.0);
            assert_eq!(font_descriptor, descriptor);
            assert!(background_enabled);
            assert_eq!(wrap_width, Some(90));
        }
        other => panic!("expected text, got {other:?}"),
    }
}
