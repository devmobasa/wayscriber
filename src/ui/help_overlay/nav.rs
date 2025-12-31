use crate::input::HelpOverlayView;

use super::super::primitives::{draw_rounded_rect, text_extents_for};
use super::search::{draw_segmented_text, ellipsize_to_fit};

const BULLET: &str = "\u{2022}";

pub(crate) struct NavState {
    pub(crate) nav_text_primary: String,
    pub(crate) nav_secondary_segments: Vec<(String, [f64; 4])>,
    pub(crate) nav_tertiary_segments: Option<Vec<(String, [f64; 4])>>,
    pub(crate) nav_primary_width: f64,
    pub(crate) nav_secondary_width: f64,
    pub(crate) nav_tertiary_width: Option<f64>,
    pub(crate) nav_font_size: f64,
    pub(crate) nav_block_height: f64,
    pub(crate) extra_line_text: Option<String>,
    pub(crate) extra_line_width: Option<f64>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_nav_state(
    ctx: &cairo::Context,
    help_font_family: &str,
    view_label: &str,
    view: HelpOverlayView,
    search_active: bool,
    page_index: usize,
    page_count: usize,
    search_query: &str,
    nav_key_color: [f64; 4],
    subtitle_color: [f64; 4],
    nav_font_size: f64,
    nav_line_gap: f64,
    nav_bottom_spacing: f64,
    extra_line_gap: f64,
    extra_line_bottom_spacing: f64,
    max_search_width: f64,
) -> NavState {
    let nav_text_primary = if !search_active && matches!(view, HelpOverlayView::Full) {
        format!(
            "{} view  {}  Page {}/{}",
            view_label,
            BULLET,
            page_index + 1,
            page_count
        )
    } else {
        format!("{} view", view_label)
    };
    let nav_separator = format!("   {}   ", BULLET);
    let nav_secondary_segments: Vec<(String, [f64; 4])> = if search_active {
        vec![
            ("Esc".to_string(), nav_key_color),
            (": Close".to_string(), subtitle_color),
            (nav_separator.clone(), subtitle_color),
            ("Backspace".to_string(), nav_key_color),
            (": Remove".to_string(), subtitle_color),
            (nav_separator.clone(), subtitle_color),
            ("Tab".to_string(), nav_key_color),
            (": Toggle view".to_string(), subtitle_color),
        ]
    } else if page_count > 1 {
        vec![
            ("Switch pages:  ".to_string(), subtitle_color),
            (
                "Left/Right, PageUp/PageDown, Home/End".to_string(),
                nav_key_color,
            ),
        ]
    } else {
        vec![
            ("Tab".to_string(), nav_key_color),
            (": Toggle view".to_string(), subtitle_color),
        ]
    };
    // Third nav line for multi-page view (separate from switch pages)
    let nav_tertiary_segments: Option<Vec<(String, [f64; 4])>> = if !search_active && page_count > 1
    {
        Some(vec![
            ("Tab".to_string(), nav_key_color),
            (": Toggle view".to_string(), subtitle_color),
        ])
    } else {
        None
    };
    let nav_text_secondary: String = nav_secondary_segments
        .iter()
        .map(|(text, _)| text.as_str())
        .collect();
    let nav_tertiary_text: String = nav_tertiary_segments
        .as_ref()
        .map(|segs| segs.iter().map(|(t, _)| t.as_str()).collect())
        .unwrap_or_default();

    let nav_primary_width = text_extents_for(
        ctx,
        help_font_family,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        nav_font_size,
        &nav_text_primary,
    )
    .width();
    let nav_secondary_width = text_extents_for(
        ctx,
        help_font_family,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        nav_font_size,
        &nav_text_secondary,
    )
    .width();
    let nav_tertiary_width = if nav_tertiary_segments.is_some() {
        Some(
            text_extents_for(
                ctx,
                help_font_family,
                cairo::FontSlant::Normal,
                cairo::FontWeight::Normal,
                nav_font_size,
                &nav_tertiary_text,
            )
            .width(),
        )
    } else {
        None
    };

    let search_text = if search_active {
        let prefix = "Search: ";
        let prefix_extents = text_extents_for(
            ctx,
            help_font_family,
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
            nav_font_size,
            prefix,
        );
        let max_query_width = (max_search_width - prefix_extents.width()).max(0.0);
        let query_display = ellipsize_to_fit(
            ctx,
            search_query,
            help_font_family,
            nav_font_size,
            cairo::FontWeight::Normal,
            max_query_width,
        );
        Some(format!("{}{}", prefix, query_display))
    } else {
        None
    };
    let search_hint_text = (!search_active).then(|| "Type to search".to_string());
    let extra_line_text = search_text.or(search_hint_text);
    let extra_line_width = extra_line_text.as_ref().map(|text| {
        text_extents_for(
            ctx,
            help_font_family,
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
            nav_font_size,
            text,
        )
        .width()
    });

    let nav_tertiary_height = if nav_tertiary_segments.is_some() {
        nav_line_gap + nav_font_size
    } else {
        0.0
    };
    let nav_block_height = if extra_line_text.is_some() {
        nav_font_size * 2.0
            + nav_line_gap
            + nav_tertiary_height
            + extra_line_gap
            + nav_font_size
            + extra_line_bottom_spacing
    } else {
        nav_font_size * 2.0 + nav_line_gap + nav_tertiary_height + nav_bottom_spacing
    };

    NavState {
        nav_text_primary,
        nav_secondary_segments,
        nav_tertiary_segments,
        nav_primary_width,
        nav_secondary_width,
        nav_tertiary_width,
        nav_font_size,
        nav_block_height,
        extra_line_text,
        extra_line_width,
    }
}

pub(crate) struct NavDrawStyle<'a> {
    pub(crate) font_family: &'a str,
    pub(crate) subtitle_color: [f64; 4],
    pub(crate) search_color: [f64; 4],
    pub(crate) nav_line_gap: f64,
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
    ctx.select_font_face(
        style.font_family,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    ctx.set_font_size(nav.nav_font_size);
    ctx.set_source_rgba(
        style.subtitle_color[0],
        style.subtitle_color[1],
        style.subtitle_color[2],
        style.subtitle_color[3],
    );
    let nav_baseline = cursor_y + nav.nav_font_size;
    ctx.move_to(inner_x, nav_baseline);
    let _ = ctx.show_text(&nav.nav_text_primary);
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

    // Draw tertiary nav line (for multi-page Complete view)
    if let Some(ref tertiary_segments) = nav.nav_tertiary_segments {
        cursor_y += style.nav_line_gap;
        let nav_tertiary_baseline = cursor_y + nav.nav_font_size;
        draw_segmented_text(
            ctx,
            inner_x,
            nav_tertiary_baseline,
            nav.nav_font_size,
            cairo::FontWeight::Normal,
            style.font_family,
            tertiary_segments,
        );
        cursor_y += nav.nav_font_size;
    }

    if let Some(ref extra_line_text) = nav.extra_line_text {
        cursor_y += style.extra_line_gap;

        // Draw search input field style
        let search_padding_x = 12.0;
        let search_padding_y = 6.0;
        let search_box_height = nav.nav_font_size + search_padding_y * 2.0;
        // Clamp search box to available width
        let search_box_width = inner_width.min(if let Some(width) = nav.extra_line_width {
            (width + search_padding_x * 2.0 + 20.0).min(inner_width)
        } else {
            200.0
        });
        let search_box_radius = 6.0;

        // Search box background
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

        // Search text with clipping
        let extra_line_baseline = cursor_y + search_padding_y + nav.nav_font_size;
        let max_text_width = search_box_width - search_padding_x * 2.0;

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
        ctx.move_to(inner_x + search_padding_x, extra_line_baseline);
        let _ = ctx.show_text(&display_text);
        cursor_y += search_box_height + style.extra_line_bottom_spacing;
    } else {
        cursor_y += style.nav_bottom_spacing;
    }

    cursor_y
}
