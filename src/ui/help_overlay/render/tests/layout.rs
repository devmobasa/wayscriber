use super::*;
use crate::config::{Action, HelpOverlayStyle, action_label};
use crate::label_format::NOT_BOUND_LABEL;
use crate::ui::HelpOverlayBindings;

fn build(query: &str, page: usize, show_unbound: bool, width: u32, height: u32) -> OverlayLayout {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let input = crate::input::state::test_support::make_test_input_state();
    let bindings = HelpOverlayBindings::from_input_state(&input);
    build_overlay_layout(
        &crate::ui_text::UiTextEngine::default(),
        &ctx,
        &HelpOverlayStyle::default(),
        width,
        height,
        page,
        &HelpContentSnapshot::from_bindings(&bindings, true, false, true, true),
        query,
        0.0,
        "Wayscriber Controls",
        &HeaderContent {
            version: "test",
            intro: None,
            hints: &[],
        },
        "Note",
        "Esc to close",
        false,
        show_unbound,
    )
}

fn rows(layout: &OverlayLayout) -> Vec<(&str, &str)> {
    layout
        .grid
        .rows
        .iter()
        .flatten()
        .flat_map(|measured| measured.section.rows.iter())
        .map(|row| (row.action, row.key.as_str()))
        .collect()
}

#[test]
fn unbound_rows_are_hidden_until_requested() {
    for page in [0, 1] {
        let hidden = build("", page, false, 1920, 1080);
        assert!(
            rows(&hidden).iter().all(|(_, key)| *key != NOT_BOUND_LABEL),
            "page {page} hides unbound rows by default"
        );

        let shown = build("", page, true, 1920, 1080);
        assert!(
            rows(&shown).len() > rows(&hidden).len(),
            "page {page}: the toggle brings unbound rows back"
        );
    }
    assert!(
        rows(&build("", 0, true, 1920, 1080))
            .iter()
            .any(|(_, key)| *key == NOT_BOUND_LABEL),
        "the default bindings leave some help actions unbound"
    );
}

#[test]
fn search_still_finds_unbound_actions() {
    let found = build("blur", 0, false, 1920, 1080);

    assert!(
        rows(&found).contains(&(action_label(Action::SelectBlurTool), NOT_BOUND_LABEL)),
        "search reaches actions hidden from the default view: {:?}",
        rows(&found)
    );
}

#[test]
fn key_column_starts_after_the_widest_label() {
    let layout = build("", 0, false, 1920, 1080);
    let metrics = layout.metrics;
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    let engine = crate::ui_text::UiTextEngine::default();

    for measured in layout.grid.rows.iter().flatten() {
        let widest_label = measured
            .section
            .rows
            .iter()
            .map(|row| {
                crate::ui::primitives::text_extents_for_with_engine(
                    &engine,
                    &ctx,
                    &layout.help_font_family,
                    cairo::FontSlant::Normal,
                    cairo::FontWeight::Normal,
                    metrics.body_font_size,
                    row.action,
                )
                .width()
            })
            .fold(0.0, f64::max);
        assert_eq!(measured.label_column_width, widest_label);

        // Keys sit one gap past the labels, and the card leaves no dead
        // space before them beyond that gap.
        let key_start = metrics.section_card_padding + widest_label + metrics.key_desc_gap;
        assert!(
            key_start < measured.width,
            "{}: keys start inside the card",
            measured.section.title
        );
    }
}

#[test]
fn default_help_fits_a_1080p_output_without_scrolling() {
    let first = build("", 0, false, 1920, 1080);
    assert_eq!(first.scroll_max, 0.0, "page 1 fits without scrolling");

    // Page 2 carries five sections; it may scroll, but never outgrows the
    // output.
    for page in [0, 1] {
        let layout = build("", page, false, 1920, 1080);
        assert!(
            layout.box_height <= 1080.0 * 0.92,
            "page {page}: box height {}",
            layout.box_height
        );
    }
}
