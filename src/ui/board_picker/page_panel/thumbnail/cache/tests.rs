use super::*;
use crate::draw::frame::UndoAction;
use crate::draw::{Color, FontDescriptor, Frame, Shape};

fn rect(x: i32, color: Color) -> Shape {
    Shape::Rect {
        x,
        y: 60,
        w: 130,
        h: 95,
        fill: true,
        color,
        thick: 3.0,
    }
}

pub(super) fn sample_frame() -> Frame {
    let mut frame = Frame::new();
    frame.add_shape(rect(30, Color::new(1.0, 0.1, 0.2, 0.65)));
    frame.add_shape(Shape::Text {
        x: 40,
        y: 220,
        text: "Page α".into(),
        color: Color::new(0.9, 0.8, 0.1, 1.0),
        size: 32.0,
        font_descriptor: FontDescriptor::default(),
        background_enabled: false,
        wrap_width: None,
    });
    frame
}

fn pixels(
    frame: &Frame,
    background: &BoardBackground,
    density: f64,
    halo: bool,
    cache: Option<&mut ThumbnailCache>,
) -> Vec<u8> {
    let mut surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        (160.0 * density).ceil() as i32,
        (120.0 * density).ceil() as i32,
    )
    .unwrap();
    {
        let ctx = cairo::Context::new(&surface).unwrap();
        ctx.set_source_rgba(0.15, 0.23, 0.35, 0.9);
        ctx.paint().unwrap();
        ctx.scale(density, density);
        let engine = UiTextEngine::default();
        let measurer = TextMeasurer::default();
        let mut draw = crate::draw::RenderCaches::default();
        let mut render = RenderCtx::new(&ctx, &mut draw);
        let args = PageContentArgs {
            render: &mut render,
            frame,
            background,
            grid: Default::default(),
            x: 8.25,
            y: 10.5,
            width: 128.0,
            height: 96.0,
            screen_width: 400,
            screen_height: 300,
            text_halo_enabled: halo,
        };
        if let Some(cache) = cache {
            render_cached_page_content(&engine, &measurer, cache, args);
        } else {
            render_page_content(&engine, &measurer, args);
        }
    }
    surface.data().unwrap().to_vec()
}

fn assert_parity(frame: &Frame, background: &BoardBackground, cache: &mut ThumbnailCache) {
    for density in [1.0, 1.25, 2.0] {
        let expected = pixels(frame, background, density, true, None);
        let actual = pixels(frame, background, density, true, Some(cache));
        let mismatches = actual.iter().zip(&expected).filter(|(a, b)| a != b).count();
        let max_delta = actual
            .iter()
            .zip(&expected)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        assert!(
            max_delta <= 1 && mismatches <= expected.len() / 1000,
            "fresh raster at density {density}: {mismatches} bytes differ, maximum {max_delta}"
        );
        let hits = cache.hits;
        let actual = pixels(frame, background, density, true, Some(cache));
        assert_eq!(cache.hits, hits + 1, "unchanged content must reuse pixels");
        assert_eq!(
            actual.iter().zip(&expected).filter(|(a, b)| a != b).count(),
            mismatches,
            "cached raster at density {density}"
        );
    }
}

#[test]
fn cached_pixels_follow_edits_rollback_history_and_render_inputs() {
    let mut frame = sample_frame();
    let mut cache = ThumbnailCache::default();
    let transparent = BoardBackground::Transparent;
    let solid = BoardBackground::Solid(Color::new(0.8, 0.9, 1.0, 1.0));
    assert_parity(&frame, &transparent, &mut cache);
    assert_parity(&frame, &solid, &mut cache);
    frame.add_shape(Shape::EraserStroke {
        points: vec![(50, 100), (180, 140)],
        brush: crate::draw::EraserBrush {
            size: 20.0,
            kind: crate::draw::EraserKind::Circle,
        },
    });
    assert_parity(&frame, &transparent, &mut cache);
    assert_parity(&frame, &solid, &mut cache);
    let first = frame.shapes[0].id;
    let before = frame.shape(first).unwrap().shape.clone();
    frame.shape_mut(first).unwrap().shape.translate(70, 30);
    assert_parity(&frame, &transparent, &mut cache);
    frame.shape_mut(first).unwrap().shape = before;
    assert_parity(&frame, &solid, &mut cache);
    let last = frame.add_shape(rect(140, Color::new(0.1, 0.9, 0.3, 1.0)));
    frame.push_undo_action(
        UndoAction::Create {
            shapes: vec![(frame.len() - 1, frame.shape(last).unwrap().clone())],
        },
        10,
    );
    assert_parity(&frame, &solid, &mut cache);
    frame.undo_last().unwrap();
    assert_parity(&frame, &solid, &mut cache);
    frame.redo_last().unwrap();
    assert_parity(&frame, &solid, &mut cache);
    frame.move_shape(2, 0).unwrap();
    assert_parity(&frame, &solid, &mut cache);
    frame.remove_shape_by_id(first).unwrap();
    assert_parity(&frame, &transparent, &mut cache);
    frame.set_view_offset(20, 10);
    assert_parity(&frame, &solid, &mut cache);
    assert_eq!(
        pixels(&frame, &solid, 1.0, false, None),
        pixels(&frame, &solid, 1.0, false, Some(&mut cache))
    );
    frame.clear();
    assert_parity(&frame, &transparent, &mut cache);
}

#[test]
fn cache_prunes_removed_pages_and_stays_within_entry_and_byte_budgets() {
    let mut cache = ThumbnailCache::default();
    for index in 0..MAX_ENTRIES + 5 {
        let mut frame = sample_frame();
        frame.shapes[0].shape.translate(index as i32, 0);
        pixels(
            &frame,
            &BoardBackground::Transparent,
            1.0,
            true,
            Some(&mut cache),
        );
    }
    assert_eq!(cache.entries.len(), MAX_ENTRIES);
    assert!(cache.bytes <= MAX_BYTES);
    cache.retain_revisions(std::iter::empty());
    assert!(cache.entries.is_empty());
    assert_eq!(cache.bytes, 0);
}

#[test]
fn transparent_spotlights_and_unmagnified_solid_spotlights_remain_cacheable() {
    let mut frame = sample_frame();
    frame.add_shape(Shape::Spotlight {
        cx: 140,
        cy: 120,
        rx: 85,
        ry: 65,
        magnification: 2.0,
    });
    let mut cache = ThumbnailCache::default();
    assert_parity(&frame, &BoardBackground::Transparent, &mut cache);
    if let Shape::Spotlight { magnification, .. } = &mut frame.shapes[2].shape {
        *magnification = 1.0;
    }
    assert_parity(
        &frame,
        &BoardBackground::Solid(Color::new(0.8, 0.9, 1.0, 1.0)),
        &mut cache,
    );
}

#[test]
#[ignore = "release timing workload; run with --release --ignored --nocapture"]
fn measure_cached_thumbnail_hover() {
    let mut frame = Frame::new();
    for index in 0..10_000 {
        frame.add_shape(Shape::Line {
            x1: index % 400,
            y1: index / 40,
            x2: index % 400 + 4,
            y2: index / 40 + 3,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            thick: 1.0,
        });
    }
    let background = BoardBackground::Transparent;
    let mut cache = ThumbnailCache::default();
    pixels(&frame, &background, 1.0, true, Some(&mut cache));
    for cached in [false, true] {
        let start = std::time::Instant::now();
        for _ in 0..100 {
            std::hint::black_box(pixels(
                &frame,
                &background,
                1.0,
                true,
                cached.then_some(&mut cache),
            ));
        }
        eprintln!(
            "10k shapes, 100 hover paints, cached={cached}: {:?}",
            start.elapsed()
        );
    }
}
