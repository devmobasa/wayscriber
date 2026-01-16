use crate::ui::primitives::draw_rounded_rect;
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::super::search::{draw_segmented_text, ellipsize_to_fit};
use super::NavState;

pub(crate) struct NavDrawStyle<'a> {
    pub(crate) font_family: &'a str,
    pub(crate) subtitle_color: [f64; 4],
    pub(crate) search_color: [f64; 4],
    pub(crate) nav_line_gap: f64,
    #[allow(dead_code)]
    pub(crate) nav_bottom_spacing: f64,
    pub(crate) extra_line_gap: f64,
    pub(crate) extra_line_bottom_spacing: f64,
}

pub(crate) fn draw_nav(
    ctx: &cairo::Context,
    inner_x: f64,
    mut cursor_y: f64,
    inner_width: f64,
    nav: &NavState,
    style: &NavDrawStyle<'_>,
) -> f64 {
    let nav_style = UiTextStyle {
        family: style.font_family,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: nav.nav_font_size,
    };
    ctx.set_source_rgba(
        style.subtitle_color[0],
        style.subtitle_color[1],
        style.subtitle_color[2],
        style.subtitle_color[3],
    );
    let nav_baseline = cursor_y + nav.nav_font_size;
    draw_text_baseline(
        ctx,
        nav_style,
        &nav.nav_text_primary,
        inner_x,
        nav_baseline,
        None,
    );
    cursor_y += nav.nav_font_size + style.nav_line_gap;

    let nav_secondary_baseline = cursor_y + nav.nav_font_size;
    draw_segmented_text(
        ctx,
        inner_x,
        nav_secondary_baseline,
        nav.nav_font_size,
        cairo::FontWeight::Normal,
        style.font_family,
        &nav.nav_secondary_segments,
    );
    cursor_y += nav.nav_font_size;

    // Always show search box
    cursor_y += style.extra_line_gap;

    // Draw search input field style.
    let search_padding_x = 12.0;
    let search_padding_y = 6.0;
    let search_box_height = nav.nav_font_size + search_padding_y * 2.0;
    // Determine search box width - wider for placeholder, narrower for actual text
    let search_box_width = if nav.extra_line_text.is_some() {
        inner_width.min(if let Some(width) = nav.extra_line_width {
            (width + search_padding_x * 2.0 + 20.0).min(inner_width)
        } else {
            200.0
        })
    } else {
        // Width for placeholder text
        inner_width.min(250.0)
    };
    let search_box_radius = 6.0;

    // Search box background.
    draw_rounded_rect(
        ctx,
        inner_x,
        cursor_y,
        search_box_width,
        search_box_height,
        search_box_radius,
    );
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.3);
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(
        style.search_color[0],
        style.search_color[1],
        style.search_color[2],
        0.5,
    );
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    let extra_line_baseline = cursor_y + search_padding_y + nav.nav_font_size;
    let max_text_width = search_box_width - search_padding_x * 2.0;

    if let Some(ref extra_line_text) = nav.extra_line_text {
        // Search text with clipping.
        let display_text = ellipsize_to_fit(
            ctx,
            extra_line_text,
            style.font_family,
            nav.nav_font_size,
            cairo::FontWeight::Normal,
            max_text_width,
        );

        ctx.set_source_rgba(
            style.search_color[0],
            style.search_color[1],
            style.search_color[2],
            style.search_color[3],
        );
        draw_text_baseline(
            ctx,
            nav_style,
            &display_text,
            inner_x + search_padding_x,
            extra_line_baseline,
            None,
        );
    } else {
        // Show placeholder text
        ctx.set_source_rgba(
            style.search_color[0],
            style.search_color[1],
            style.search_color[2],
            0.5,
        );
        draw_text_baseline(
            ctx,
            nav_style,
            "Type to search... (Esc clears)",
            inner_x + search_padding_x,
            extra_line_baseline,
            None,
        );
    }
    cursor_y += search_box_height + style.extra_line_bottom_spacing;

    cursor_y
}
