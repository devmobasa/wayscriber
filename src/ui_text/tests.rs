use super::*;

fn style(size: f64) -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size,
    }
}

fn uncached_text_extents(
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    text: &str,
    wrap_width: Option<f64>,
) -> UiTextExtents {
    let layout = pangocairo::functions::create_layout(ctx);
    let font_desc = font_description(style);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);
    if let Some(width) = wrap_width {
        layout.set_width(to_pango_units(width.max(1.0)));
        layout.set_wrap(pango::WrapMode::WordChar);
    }
    let (ink_rect, logical_rect, baseline) = layout_metrics(&layout);
    rect_to_extents(ink_rect, logical_rect, baseline)
}

fn assert_extents_eq(actual: UiTextExtents, expected: UiTextExtents) {
    assert_eq!(actual.width(), expected.width());
    assert_eq!(actual.height(), expected.height());
    assert_eq!(actual.x_bearing(), expected.x_bearing());
    assert_eq!(actual.y_bearing(), expected.y_bearing());
    assert_eq!(actual.x_advance(), expected.x_advance());
    assert_eq!(actual.y_advance, expected.y_advance);
}

#[test]
fn cached_layout_returns_identical_extents() {
    let engine = UiTextEngine::default();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();

    let first = engine
        .layout(&ctx, style(15.0), "cache me", None)
        .ink_extents();
    let second = engine
        .layout(&ctx, style(15.0), "cache me", None)
        .ink_extents();

    assert_eq!(first.width(), second.width());
    assert_eq!(first.height(), second.height());
    assert_eq!(first.x_bearing(), second.x_bearing());
    assert_eq!(first.y_bearing(), second.y_bearing());
}

#[test]
fn cached_layout_recomputes_extents_after_context_update() {
    let engine = UiTextEngine::default();
    let first_surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 64, 64).unwrap();
    let first_ctx = cairo::Context::new(&first_surface).unwrap();
    let scaled_surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 64, 64).unwrap();
    let scaled_ctx = cairo::Context::new(&scaled_surface).unwrap();
    scaled_ctx.scale(2.0, 2.0);

    let style = style(18.0);
    let text = "cache scale";
    let _ = engine.layout(&first_ctx, style, text, None).ink_extents();
    let expected_scaled = uncached_text_extents(&scaled_ctx, style, text, None);

    let cached_scaled = engine.layout(&scaled_ctx, style, text, None).ink_extents();
    assert_extents_eq(cached_scaled, expected_scaled);
}

#[test]
fn measure_text_matches_rendered_layout_extents() {
    let engine = UiTextEngine::default();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();

    let measured = engine
        .measure(style(16.0), "toast body", None)
        .expect("measurement");
    let rendered = engine
        .layout(&ctx, style(16.0), "toast body", None)
        .ink_extents();

    assert_eq!(measured.width(), rendered.width());
    assert_eq!(measured.height(), rendered.height());
}

#[test]
fn different_styles_produce_distinct_cache_entries() {
    let engine = UiTextEngine::default();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();

    let small = engine
        .layout(&ctx, style(10.0), "sized", None)
        .ink_extents();
    let large = engine
        .layout(&ctx, style(30.0), "sized", None)
        .ink_extents();
    assert!(large.width() > small.width());

    // Wrap width participates in the key: same text, different layouts.
    let unwrapped = engine
        .layout(&ctx, style(12.0), "wrap wrap wrap wrap", None)
        .ink_extents();
    let wrapped = engine
        .layout(&ctx, style(12.0), "wrap wrap wrap wrap", Some(40.0))
        .ink_extents();
    assert!(wrapped.height() >= unwrapped.height());
}

#[test]
fn owners_create_measurement_contexts_lazily_and_keep_layouts_independent() {
    let first = UiTextEngine::default();
    let second = UiTextEngine::default();
    assert!(first.measurement.borrow().is_none());
    assert!(second.measurement.borrow().is_none());
    assert!(first.layouts.borrow().entries.is_empty());
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let a = first.layout(&ctx, style(16.0), "owner", None);
    let hit = first.layout(&ctx, style(16.0), "owner", None);
    let b = second.layout(&ctx, style(16.0), "owner", None);
    assert_eq!(
        a.layout, hit.layout,
        "one owner reuses its shaped allocation"
    );
    assert_ne!(
        a.layout, b.layout,
        "independent owners must not alias layouts"
    );
    assert!(
        first.measurement.borrow().is_none(),
        "painting does not require dummy context"
    );
    first.measure(style(16.0), "owner", None).unwrap();
    assert!(first.measurement.borrow().is_some());
    assert!(second.measurement.borrow().is_none());
}

#[test]
fn lru_retains_512_layouts_and_hits_promote_before_eviction() {
    let engine = UiTextEngine::default();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let first = engine.layout(&ctx, style(12.0), "entry 0", None);
    let second = engine.layout(&ctx, style(12.0), "entry 1", None);
    for index in 2..512 {
        engine.layout(&ctx, style(12.0), &format!("entry {index}"), None);
    }
    assert_eq!(engine.layouts.borrow().entries.len(), 512);
    assert_eq!(
        engine.layout(&ctx, style(12.0), "entry 0", None).layout,
        first.layout
    );
    engine.layout(&ctx, style(12.0), "overflow", None);
    assert_eq!(engine.layouts.borrow().entries.len(), 512);
    assert_eq!(
        engine.layout(&ctx, style(12.0), "entry 0", None).layout,
        first.layout
    );
    assert_ne!(
        engine.layout(&ctx, style(12.0), "entry 1", None).layout,
        second.layout
    );
}

#[test]
fn measure_scaled_paint_measure_restores_canonical_context_metrics() {
    let engine = UiTextEngine::default();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 400, 160).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    ctx.scale(1.5, 2.0);
    let mut options = cairo::FontOptions::new().unwrap();
    options.set_hint_metrics(cairo::HintMetrics::Off);
    options.set_antialias(cairo::Antialias::Gray);
    ctx.set_font_options(&options);
    for text in [
        "",
        "Hello العربية עברית",
        "Wrap words across multiple visual lines",
    ] {
        for wrap in [None, Some(70.0)] {
            let before = engine.measure(style(18.0), text, wrap).unwrap();
            let painted = engine.draw_baseline(&ctx, style(18.0), text, 2.0, 30.0, wrap);
            assert_extents_eq(
                painted,
                uncached_text_extents(&ctx, style(18.0), text, wrap),
            );
            let after = engine.measure(style(18.0), text, wrap).unwrap();
            assert_extents_eq(after, before);
        }
    }
}

#[test]
fn cache_keys_keep_font_categories_quantized_size_and_wrap_units() {
    let engine = UiTextEngine::default();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let baseline = style(16.0);
    let first = engine.layout(&ctx, baseline, "key", None);
    let quantized = UiTextStyle {
        size: 16.001,
        ..baseline
    };
    assert_eq!(
        first.layout,
        engine.layout(&ctx, quantized, "key", None).layout
    );
    for changed in [
        UiTextStyle {
            family: "Monospace",
            ..baseline
        },
        UiTextStyle {
            slant: cairo::FontSlant::Italic,
            ..baseline
        },
        UiTextStyle {
            weight: cairo::FontWeight::Normal,
            ..baseline
        },
        UiTextStyle {
            size: 17.0,
            ..baseline
        },
    ] {
        assert_ne!(
            first.layout,
            engine.layout(&ctx, changed, "key", None).layout
        );
    }
    assert_ne!(
        first.layout,
        engine.layout(&ctx, baseline, "other", None).layout
    );
    let minimum = engine.layout(&ctx, baseline, "key", Some(1.0));
    assert_eq!(
        minimum.layout,
        engine.layout(&ctx, baseline, "key", Some(-10.0)).layout
    );
    assert_ne!(
        minimum.layout,
        engine.layout(&ctx, baseline, "key", Some(2.0)).layout
    );
}

#[test]
fn temporary_legacy_bridge_retains_layouts_and_matches_an_explicit_owner() {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let first = text_layout(&ctx, style(14.0), "legacy bridge", Some(50.0));
    let second = text_layout(&ctx, style(14.0), "legacy bridge", Some(50.0));
    assert_eq!(first.layout, second.layout);
    let engine = UiTextEngine::default();
    let explicit = engine.layout(&ctx, style(14.0), "legacy bridge", Some(50.0));
    assert_ne!(first.layout, explicit.layout);
    assert_extents_eq(first.ink_extents(), explicit.ink_extents());
}
