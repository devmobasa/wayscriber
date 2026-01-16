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

pub(crate) fn text_layout(
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
    text: &str,
    wrap_width: Option<f64>,
) -> UiTextLayout {
    let layout = pangocairo::functions::create_layout(ctx);
    let font_desc = font_description(style);
    layout.set_font_description(Some(&font_desc));
    layout.set_text(text);
    if let Some(width) = wrap_width {
        let width = width.max(1.0);
        layout.set_width(to_pango_units(width));
        layout.set_wrap(pango::WrapMode::WordChar);
    }
    let (ink_rect, logical_rect) = layout.extents();
    let baseline = layout.baseline() as f64 / pango::SCALE as f64;
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
