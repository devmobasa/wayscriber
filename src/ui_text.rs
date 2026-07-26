#[derive(Clone, Copy, Debug)]
pub(crate) struct UiTextStyle<'a> {
    pub family: &'a str,
    pub slant: cairo::FontSlant,
    pub weight: cairo::FontWeight,
    pub size: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct UiTextExtents {
    x_bearing: f64,
    y_bearing: f64,
    width: f64,
    height: f64,
    x_advance: f64,
    y_advance: f64,
}

impl UiTextExtents {
    pub(crate) fn x_bearing(self) -> f64 {
        self.x_bearing
    }

    pub(crate) fn y_bearing(self) -> f64 {
        self.y_bearing
    }

    pub(crate) fn width(self) -> f64 {
        self.width
    }

    pub(crate) fn height(self) -> f64 {
        self.height
    }

    pub(crate) fn x_advance(self) -> f64 {
        self.x_advance
    }

    pub(crate) fn to_cairo(self) -> cairo::TextExtents {
        cairo::TextExtents::new(
            self.x_bearing,
            self.y_bearing,
            self.width,
            self.height,
            self.x_advance,
            self.y_advance,
        )
    }
}

pub(crate) struct UiTextLayout {
    layout: pango::Layout,
    ink_rect: pango::Rectangle,
    logical_rect: pango::Rectangle,
    baseline: f64,
}

impl UiTextLayout {
    pub(crate) fn ink_extents(&self) -> UiTextExtents {
        rect_to_extents(self.ink_rect, self.logical_rect, self.baseline)
    }

    pub(crate) fn show_at_baseline(&self, ctx: &cairo::Context, x: f64, y: f64) {
        ctx.move_to(x, y - self.baseline);
        pangocairo::functions::show_layout(ctx, &self.layout);
    }
}

/// Measure UI text without a rendering context (e.g. for damage computation
/// before a frame buffer exists). The measurement surface belongs to this
/// call, so independent UI roots cannot share mutable Pango state.
pub(crate) fn measure_text(
    style: UiTextStyle<'_>,
    text: &str,
    wrap_width: Option<f64>,
) -> Option<UiTextExtents> {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).ok()?;
    let ctx = cairo::Context::new(&surface).ok()?;
    Some(text_layout(&ctx, style, text, wrap_width).ink_extents())
}

pub(crate) fn text_layout(
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    text: &str,
    wrap_width: Option<f64>,
) -> UiTextLayout {
    let wrap_units = wrap_width.map_or(-1, |width| to_pango_units(width.max(1.0)));
    let layout = pangocairo::functions::create_layout(ctx);
    let font_desc = font_description(style);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);
    if wrap_units >= 0 {
        layout.set_width(wrap_units);
        layout.set_wrap(pango::WrapMode::WordChar);
    }
    let (ink_rect, logical_rect, baseline) = layout_metrics(&layout);

    UiTextLayout {
        layout,
        ink_rect,
        logical_rect,
        baseline,
    }
}

pub(crate) fn draw_text_baseline(
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    text: &str,
    x: f64,
    y: f64,
    wrap_width: Option<f64>,
) -> UiTextExtents {
    let layout = text_layout(ctx, style, text, wrap_width);
    let extents = layout.ink_extents();
    layout.show_at_baseline(ctx, x, y);
    extents
}

fn rect_to_extents(
    rect: pango::Rectangle,
    logical: pango::Rectangle,
    baseline: f64,
) -> UiTextExtents {
    let scale = pango::SCALE as f64;
    UiTextExtents {
        x_bearing: rect.x() as f64 / scale,
        y_bearing: rect.y() as f64 / scale - baseline,
        width: rect.width() as f64 / scale,
        height: rect.height() as f64 / scale,
        x_advance: logical.width() as f64 / scale,
        y_advance: 0.0,
    }
}

fn font_description(style: UiTextStyle<'_>) -> pango::FontDescription {
    let mut desc = pango::FontDescription::new();
    desc.set_family(style.family);
    desc.set_weight(pango_weight(style.weight));
    desc.set_style(pango_style(style.slant));
    desc.set_absolute_size(to_pango_units_f64(style.size));
    desc
}

fn pango_weight(weight: cairo::FontWeight) -> pango::Weight {
    match weight {
        cairo::FontWeight::Bold => pango::Weight::Bold,
        cairo::FontWeight::Normal => pango::Weight::Normal,
        _ => pango::Weight::Normal,
    }
}

fn pango_style(slant: cairo::FontSlant) -> pango::Style {
    match slant {
        cairo::FontSlant::Italic => pango::Style::Italic,
        cairo::FontSlant::Oblique => pango::Style::Oblique,
        cairo::FontSlant::Normal => pango::Style::Normal,
        _ => pango::Style::Normal,
    }
}

fn to_pango_units(value: f64) -> i32 {
    let scaled = (value * pango::SCALE as f64).round();
    scaled.clamp(i32::MIN as f64, i32::MAX as f64) as i32
}

fn to_pango_units_f64(value: f64) -> f64 {
    let scaled = value * pango::SCALE as f64;
    scaled.clamp(i32::MIN as f64, i32::MAX as f64)
}

fn layout_metrics(layout: &pango::Layout) -> (pango::Rectangle, pango::Rectangle, f64) {
    let (ink_rect, logical_rect) = layout.extents();
    let baseline = layout.baseline() as f64 / pango::SCALE as f64;
    (ink_rect, logical_rect, baseline)
}

#[cfg(test)]
mod tests {
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
    }

    #[test]
    fn cached_layout_returns_identical_extents() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();

        let first = text_layout(&ctx, style(15.0), "cache me", None).ink_extents();
        let second = text_layout(&ctx, style(15.0), "cache me", None).ink_extents();

        assert_eq!(first.width(), second.width());
        assert_eq!(first.height(), second.height());
        assert_eq!(first.x_bearing(), second.x_bearing());
        assert_eq!(first.y_bearing(), second.y_bearing());
    }

    #[test]
    fn cached_layout_recomputes_extents_after_context_update() {
        let first_surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 64, 64).unwrap();
        let first_ctx = cairo::Context::new(&first_surface).unwrap();
        let scaled_surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 64, 64).unwrap();
        let scaled_ctx = cairo::Context::new(&scaled_surface).unwrap();
        scaled_ctx.scale(2.0, 2.0);

        let style = style(18.0);
        let text = "cache scale";
        let _ = text_layout(&first_ctx, style, text, None).ink_extents();
        let expected_scaled = uncached_text_extents(&scaled_ctx, style, text, None);

        let cached_scaled = text_layout(&scaled_ctx, style, text, None).ink_extents();
        assert_extents_eq(cached_scaled, expected_scaled);
    }

    #[test]
    fn measure_text_matches_rendered_layout_extents() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();

        let measured = measure_text(style(16.0), "toast body", None).expect("measurement");
        let rendered = text_layout(&ctx, style(16.0), "toast body", None).ink_extents();

        assert_eq!(measured.width(), rendered.width());
        assert_eq!(measured.height(), rendered.height());
    }

    #[test]
    fn different_styles_produce_distinct_cache_entries() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 4).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();

        let small = text_layout(&ctx, style(10.0), "sized", None).ink_extents();
        let large = text_layout(&ctx, style(30.0), "sized", None).ink_extents();
        assert!(large.width() > small.width());

        // Wrap width participates in the key: same text, different layouts.
        let unwrapped = text_layout(&ctx, style(12.0), "wrap wrap wrap wrap", None).ink_extents();
        let wrapped =
            text_layout(&ctx, style(12.0), "wrap wrap wrap wrap", Some(40.0)).ink_extents();
        assert!(wrapped.height() >= unwrapped.height());
    }
}
