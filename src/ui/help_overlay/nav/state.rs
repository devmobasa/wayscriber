use crate::ui::primitives::text_extents_for;

use super::super::search::ellipsize_to_fit;

const BULLET: &str = "\u{2022}";

#[derive(Clone)]
pub(crate) struct NavState {
    pub(crate) nav_text_primary: String,
    pub(crate) nav_secondary_segments: Vec<(String, [f64; 4])>,
    pub(crate) nav_primary_width: f64,
    pub(crate) nav_secondary_width: f64,
    pub(crate) nav_font_size: f64,
    pub(crate) nav_block_height: f64,
    pub(crate) extra_line_text: Option<String>,
    pub(crate) extra_line_width: Option<f64>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_nav_state(
    ctx: &cairo::Context,
    help_font_family: &str,
    nav_title: &str,
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
    let nav_text_primary = if !search_active && page_count > 1 {
        format!(
            "{}  {}  Page {}/{}",
            nav_title,
            BULLET,
            page_index + 1,
            page_count
        )
    } else {
        nav_title.to_string()
    };
    let nav_separator = format!("   {}   ", BULLET);
    let nav_secondary_segments: Vec<(String, [f64; 4])> = if search_active {
        vec![
            ("Esc".to_string(), nav_key_color),
            (": Close".to_string(), subtitle_color),
            (nav_separator.clone(), subtitle_color),
            ("Backspace".to_string(), nav_key_color),
            (": Remove".to_string(), subtitle_color),
        ]
    } else if page_count > 1 {
        vec![
            ("Switch pages:".to_string(), subtitle_color),
            (
                "  Left/Right, PageUp/PageDown, Home/End".to_string(),
                nav_key_color,
            ),
        ]
    } else {
        vec![
            ("Esc".to_string(), nav_key_color),
            (": Close".to_string(), subtitle_color),
        ]
    };
    let nav_text_secondary: String = nav_secondary_segments
        .iter()
        .map(|(text, _)| text.as_str())
        .collect();

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

    let nav_block_height = if extra_line_text.is_some() {
        nav_font_size * 2.0
            + nav_line_gap
            + extra_line_gap
            + nav_font_size
            + extra_line_bottom_spacing
    } else {
        nav_font_size * 2.0 + nav_line_gap + nav_bottom_spacing
    };

    NavState {
        nav_text_primary,
        nav_secondary_segments,
        nav_primary_width,
        nav_secondary_width,
        nav_font_size,
        nav_block_height,
        extra_line_text,
        extra_line_width,
    }
}
