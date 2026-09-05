use super::{RenderCaches, RenderCtx};
use crate::draw::{
    ArrowLabel, ArrowStyle, FontDescriptor, Frame, RED, Shape, StepMarkerLabel, TextMeasurer,
    YELLOW,
};

fn scene() -> Frame {
    let font = FontDescriptor::default();
    let mut frame = Frame::new();
    frame.add_shape(Shape::Text {
        x: 20,
        y: 45,
        text: "Wrapped العربية 測試 words across lines".into(),
        color: RED,
        size: 20.0,
        font_descriptor: font.clone(),
        background_enabled: true,
        wrap_width: Some(150),
    });
    frame.add_shape(Shape::StickyNote {
        x: 200,
        y: 45,
        text: "Note 測試 words".into(),
        background: YELLOW,
        size: 18.0,
        font_descriptor: font.clone(),
        wrap_width: Some(120),
    });
    frame.add_shape(Shape::Arrow {
        x1: 30,
        y1: 210,
        x2: 330,
        y2: 210,
        color: RED,
        thick: 4.0,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        head_at_end: true,
        style: ArrowStyle::Standard,
        bend: 0.2,
        label: Some(ArrowLabel {
            value: 72,
            size: 24.0,
            font_descriptor: font.clone(),
        }),
    });
    frame.add_shape(Shape::StepMarker {
        x: 220,
        y: 290,
        color: RED,
        label: StepMarkerLabel {
            value: 108,
            size: 22.0,
            font_descriptor: font,
        },
    });
    frame
}

fn pixels(
    measurer: &TextMeasurer,
    caches: &mut RenderCaches,
    frame: &Frame,
    density: i32,
    halo: bool,
    legacy: bool,
) -> Vec<u8> {
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, 400 * density, 380 * density).unwrap();
    surface.set_device_scale(density as f64, density as f64);
    {
        let ctx = cairo::Context::new(&surface).unwrap();
        ctx.set_source_rgb(1.0, 1.0, 1.0);
        ctx.paint().unwrap();
        ctx.translate(3.0, 4.0);
        let before = ctx.matrix();
        let mut render = RenderCtx::new(&ctx, caches);
        for shape in &frame.shapes {
            if legacy {
                render.render_shape_with_halo(&shape.shape, halo);
                crate::draw::render_selection_halo(&ctx, shape);
            } else {
                render.render_shape_with_halo_with_measurer(measurer, &shape.shape, halo);
                crate::draw::render_selection_halo_with_measurer(measurer, &ctx, shape);
            }
        }
        assert_eq!(ctx.matrix(), before);
    }
    surface.data().unwrap().to_vec()
}

#[test]
fn retained_shape_measurement_matches_fresh_and_legacy_paint_and_preserves_bounds() {
    let frame = scene();
    let measurer = TextMeasurer::default();
    let mut caches = RenderCaches::default();
    let bounds: Vec<_> = frame
        .shapes
        .iter()
        .map(|shape| shape.bounding_box_with(&measurer))
        .collect();
    for density in [1, 2, 1] {
        for halo in [true, false] {
            let actual = pixels(&measurer, &mut caches, &frame, density, halo, false);
            let fresh = pixels(
                &TextMeasurer::default(),
                &mut RenderCaches::default(),
                &frame,
                density,
                halo,
                false,
            );
            assert!(
                actual == fresh,
                "fresh shape pixels: density {density}, halo {halo}"
            );
            let legacy = pixels(
                &measurer,
                &mut RenderCaches::default(),
                &frame,
                density,
                halo,
                true,
            );
            assert!(
                actual == legacy,
                "legacy shape pixels: density {density}, halo {halo}"
            );
            assert_eq!(
                frame
                    .shapes
                    .iter()
                    .map(|shape| shape.bounding_box_with(&measurer))
                    .collect::<Vec<_>>(),
                bounds
            );
        }
    }
}

#[test]
fn empty_sticky_note_preview_keeps_its_background_with_explicit_measurement() {
    let paint = |measurer: &TextMeasurer| {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 100).unwrap();
        {
            let ctx = cairo::Context::new(&surface).unwrap();
            super::super::text::render_sticky_note_preview_with_measurer(
                measurer,
                &ctx,
                20,
                40,
                "",
                YELLOW,
                20.0,
                &FontDescriptor::default(),
                Some(100),
            );
        }
        surface.data().unwrap().to_vec()
    };
    let measurer = TextMeasurer::default();
    let first = paint(&measurer);
    assert!(first.iter().any(|byte| *byte != 0));
    assert!(first == paint(&measurer));
    assert!(first == paint(&TextMeasurer::default()));
}
