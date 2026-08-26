//! Drawing the system font picker.
//!
//! Each row is laid out in the family it names. That is the point of the whole
//! surface — nobody chooses a typeface by reading its name — and it is why the
//! visible window is capped: only the rows on screen are ever laid out, so a
//! system with 269 families costs the same as one with 12.

use crate::input::state::{
    FontPickerLayout, FontPickerRow, FontPickerTarget, InputState, font_picker_layout,
};
use crate::ui::primitives::draw_rounded_rect;
use crate::ui::theme::{self, overlay};

/// Point size the family names are drawn at.
const ROW_FONT_SIZE: f64 = 17.0;
/// Point size of the query line and the caption.
const CHROME_FONT_SIZE: f64 = 14.0;
/// Inset from a row's left edge to its text.
const ROW_TEXT_INSET: f64 = 12.0;
/// Width of the marker beside the family already in use.
const CURRENT_MARK_WIDTH: f64 = 3.0;
/// Clear space kept between two labels sharing one line.
const TEXT_GAP: f64 = 12.0;
/// Shortest the thumb gets, so a 269-family list still leaves something to see.
const SCROLL_THUMB_MIN_HEIGHT: f64 = 20.0;

/// Draw the picker. Does nothing when it is closed.
pub fn render_font_picker(ctx: &cairo::Context, state: &InputState, width: u32, height: u32) {
    if !state.is_font_picker_open() {
        return;
    }
    let families = state.font_picker_families();
    let layout = font_picker_layout(width, height, families.len());
    let current = state.font_picker_current_family();
    let rows = crate::input::state::font_picker_rows(
        layout,
        &families,
        state.font_picker_scroll(),
        state.font_picker_selected(),
        &current,
    );

    let _ = ctx.save();
    scrim(ctx, width, height);
    panel(ctx, layout);
    query_line(
        ctx,
        layout,
        state,
        (!state.font_picker_is_loading()).then_some(families.len()),
    );
    for row in &rows {
        draw_row(ctx, row);
    }
    scroll_indicator(ctx, layout, families.len(), state.font_picker_scroll());
    if state.font_picker_is_loading() {
        loading_note(ctx, layout);
    } else if state.font_picker_load_failed() {
        unavailable_note(ctx, layout);
    } else if families.is_empty() {
        empty_note(ctx, layout);
    }
    caption(ctx, layout, state);
    let _ = ctx.restore();
}

/// Dim the canvas so the panel reads as modal, matching the other pickers.
fn scrim(ctx: &cairo::Context, width: u32, height: u32) {
    ctx.set_source_rgba(0.0, 0.0, 0.0, overlay::OVERLAY_DIM_MEDIUM);
    ctx.rectangle(0.0, 0.0, f64::from(width), f64::from(height));
    let _ = ctx.fill();
}

fn panel(ctx: &cairo::Context, layout: FontPickerLayout) {
    let radius = overlay::RADIUS_LG;
    theme::set_color(ctx, overlay::SHADOW_DEEP);
    draw_rounded_rect(
        ctx,
        layout.panel_x + 1.0,
        layout.panel_y + 2.0,
        layout.panel_width,
        layout.panel_height,
        radius,
    );
    let _ = ctx.fill();
    theme::set_color(ctx, overlay::PANEL_BG_MODAL);
    draw_rounded_rect(
        ctx,
        layout.panel_x,
        layout.panel_y,
        layout.panel_width,
        layout.panel_height,
        radius,
    );
    let _ = ctx.fill();
    theme::set_color(ctx, overlay::BORDER_MODAL);
    ctx.set_line_width(1.0);
    draw_rounded_rect(
        ctx,
        layout.panel_x + 0.5,
        layout.panel_y + 0.5,
        layout.panel_width - 1.0,
        layout.panel_height - 1.0,
        radius - 0.5,
    );
    let _ = ctx.stroke();
}

fn query_line(
    ctx: &cairo::Context,
    layout: FontPickerLayout,
    state: &InputState,
    match_count: Option<usize>,
) {
    theme::set_color(ctx, overlay::INPUT_BG);
    draw_rounded_rect(
        ctx,
        layout.query_x,
        layout.query_y,
        layout.query_width,
        layout.query_height,
        overlay::RADIUS_MD,
    );
    let _ = ctx.fill();

    let query = state.font_picker_query();
    let (text, dim) = if query.is_empty() {
        ("Type to find a font".to_string(), true)
    } else {
        (query.to_string(), false)
    };
    let baseline = layout.query_y + layout.query_height / 2.0 + CHROME_FONT_SIZE / 2.5;

    // Match count on the right, so a query that narrows to nothing says so
    // before the empty list has to. Measured first: it is short and always
    // wanted, so it is the query that gives way when the two would collide.
    let count = match_count
        .map(|count| count.to_string())
        .unwrap_or_else(|| "…".to_string());
    let count_width = text_width(ctx, "Sans", CHROME_FONT_SIZE, &count);
    theme::set_color(ctx, overlay::TEXT_HINT);
    draw_text_right(
        ctx,
        "Sans",
        CHROME_FONT_SIZE,
        layout.query_x + layout.query_width - ROW_TEXT_INSET,
        baseline,
        count_width,
        &count,
    );

    theme::set_color(
        ctx,
        if dim {
            overlay::TEXT_HINT
        } else {
            overlay::TEXT_PRIMARY
        },
    );
    draw_text(
        ctx,
        "Sans",
        CHROME_FONT_SIZE,
        layout.query_x + ROW_TEXT_INSET,
        baseline,
        layout.query_width - ROW_TEXT_INSET * 2.0 - count_width - TEXT_GAP,
        &text,
    );
}

fn draw_row(ctx: &cairo::Context, row: &FontPickerRow) {
    if row.selected {
        theme::set_color(ctx, overlay::BG_SELECTION);
        draw_rounded_rect(ctx, row.x, row.y, row.width, row.height, overlay::RADIUS_SM);
        let _ = ctx.fill();
    }
    if row.current {
        theme::set_color(ctx, overlay::ACCENT_BRIGHT);
        ctx.rectangle(
            row.x,
            row.y + row.height * 0.2,
            CURRENT_MARK_WIDTH,
            row.height * 0.6,
        );
        let _ = ctx.fill();
    }

    theme::set_color(
        ctx,
        if row.selected {
            overlay::TEXT_PRIMARY
        } else {
            overlay::TEXT_SECONDARY
        },
    );
    // The family draws its own name. Pango falls back per glyph, so a font with
    // no Latin coverage still shows something readable rather than a row of
    // empty boxes.
    //
    // A family name can be long and a display face can be wide, so the row
    // ellipsizes rather than writing past its own edge and out of the panel.
    draw_text(
        ctx,
        &row.family,
        ROW_FONT_SIZE,
        row.x + ROW_TEXT_INSET,
        row.y + row.height / 2.0 + ROW_FONT_SIZE / 2.5,
        row.width - ROW_TEXT_INSET * 2.0,
        &row.family,
    );
}

/// Where the window sits in the list, as a thumb on a track.
///
/// The list runs to hundreds of families and the panel shows a dozen. Without
/// this there is nothing on screen that says whether you are near the top, the
/// middle, or the end. Same track the command palette draws.
fn scroll_indicator(ctx: &cairo::Context, layout: FontPickerLayout, total: usize, scroll: usize) {
    if total <= layout.visible_rows || layout.visible_rows == 0 {
        return;
    }
    let width = theme::toolbar::SCROLLBAR_WIDTH;
    let radius = theme::toolbar::SCROLLBAR_RADIUS;
    let track_x = layout.list_x + layout.list_width - width;
    let track_h = layout.list_height;

    theme::set_color(ctx, theme::toolbar::COLOR_SCROLLBAR_TRACK);
    draw_rounded_rect(ctx, track_x, layout.list_y, width, track_h, radius);
    let _ = ctx.fill();

    let visible = layout.visible_rows as f64 / total as f64;
    let thumb_h = (track_h * visible)
        .max(SCROLL_THUMB_MIN_HEIGHT)
        .min(track_h);
    let range = total.saturating_sub(layout.visible_rows);
    let progress = if range > 0 {
        (scroll as f64 / range as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let thumb_y = layout.list_y + progress * (track_h - thumb_h);

    theme::set_color(ctx, theme::toolbar::COLOR_SCROLLBAR_SLIDER);
    draw_rounded_rect(ctx, track_x, thumb_y, width, thumb_h, radius);
    let _ = ctx.fill();
}

/// The note that stands in for the list when nothing matched.
///
/// It sits in the row of list height the layout reserves for exactly this, so
/// it cannot land on the caption underneath.
fn empty_note(ctx: &cairo::Context, layout: FontPickerLayout) {
    theme::set_color(ctx, overlay::TEXT_HINT);
    draw_text(
        ctx,
        "Sans",
        CHROME_FONT_SIZE,
        layout.list_x + ROW_TEXT_INSET,
        layout.list_y + layout.list_height / 2.0 + CHROME_FONT_SIZE / 2.5,
        layout.list_width - ROW_TEXT_INSET * 2.0,
        "No font matches that",
    );
}

fn loading_note(ctx: &cairo::Context, layout: FontPickerLayout) {
    theme::set_color(ctx, overlay::TEXT_HINT);
    draw_text(
        ctx,
        "Sans",
        CHROME_FONT_SIZE,
        layout.list_x + ROW_TEXT_INSET,
        layout.list_y + layout.list_height / 2.0 + CHROME_FONT_SIZE / 2.5,
        layout.list_width - ROW_TEXT_INSET * 2.0,
        "Loading system fonts…",
    );
}

fn unavailable_note(ctx: &cairo::Context, layout: FontPickerLayout) {
    theme::set_color(ctx, overlay::TEXT_HINT);
    draw_text(
        ctx,
        "Sans",
        CHROME_FONT_SIZE,
        layout.list_x + ROW_TEXT_INSET,
        layout.list_y + layout.list_height / 2.0 + CHROME_FONT_SIZE / 2.5,
        layout.list_width - ROW_TEXT_INSET * 2.0,
        "System fonts could not be loaded",
    );
}

fn caption(ctx: &cairo::Context, layout: FontPickerLayout, state: &InputState) {
    let target = match state.font_picker_target() {
        FontPickerTarget::Selection => FontPickerTarget::Selection.label(),
        FontPickerTarget::ToolDefault => FontPickerTarget::ToolDefault.label(),
    };
    theme::set_color(ctx, overlay::TEXT_HINT);

    let size = CHROME_FONT_SIZE - 1.0;
    let left = format!("{target} · Tab: {}", state.font_picker_filter().label());
    const KEYS: &str = "Enter apply · Esc cancel";
    let (left_width, keys_width) = share_line(
        layout.query_width - TEXT_GAP,
        text_width(ctx, "Sans", size, &left),
        text_width(ctx, "Sans", size, KEYS),
    );

    draw_text_right(
        ctx,
        "Sans",
        size,
        layout.query_x + layout.query_width,
        layout.caption_y,
        keys_width,
        KEYS,
    );
    draw_text(
        ctx,
        "Sans",
        size,
        layout.query_x,
        layout.caption_y,
        left_width,
        &left,
    );
}

/// Split one line between two labels that both want room.
///
/// Whoever is measured first must not simply take what it asks for: doing that
/// left the caption reading "Tab:…" beside a fully drawn key list, which is the
/// half with less to say. A label that fits inside its share keeps only what it
/// needs and hands the rest over; when both overrun, they halve the line.
fn share_line(available: f64, left: f64, right: f64) -> (f64, f64) {
    if left + right <= available {
        return (left, right);
    }
    let half = available / 2.0;
    if right <= half {
        (available - right, right)
    } else if left <= half {
        (left, available - left)
    } else {
        (half, half)
    }
}

/// A Pango layout for `text`, clipped to `max_width` with an ellipsis.
///
/// Everything the panel draws is user-supplied or system-supplied and none of
/// it is bounded: a query is as long as it is typed, and a family name is as
/// wide as its own display face draws it. Ellipsizing is what keeps any of it
/// from writing over the neighbouring text or out through the panel edge.
fn bounded_layout(
    ctx: &cairo::Context,
    family: &str,
    size: f64,
    max_width: f64,
    text: &str,
) -> pango::Layout {
    let layout = pangocairo::functions::create_layout(ctx);
    let description = pango::FontDescription::from_string(&format!("{family} {size}"));
    layout.set_font_description(Some(&description));
    layout.set_text(text);
    layout.set_ellipsize(pango::EllipsizeMode::End);
    layout.set_width((max_width.max(0.0) * f64::from(pango::SCALE)) as i32);
    layout
}

/// Natural width of `text`, for callers dividing a row between two labels.
fn text_width(ctx: &cairo::Context, family: &str, size: f64, text: &str) -> f64 {
    let layout = pangocairo::functions::create_layout(ctx);
    let description = pango::FontDescription::from_string(&format!("{family} {size}"));
    layout.set_font_description(Some(&description));
    layout.set_text(text);
    f64::from(layout.extents().1.width()) / f64::from(pango::SCALE)
}

fn draw_text(
    ctx: &cairo::Context,
    family: &str,
    size: f64,
    x: f64,
    y: f64,
    max_width: f64,
    text: &str,
) {
    if max_width <= 0.0 {
        return;
    }
    let layout = bounded_layout(ctx, family, size, max_width, text);
    let baseline = f64::from(layout.baseline()) / f64::from(pango::SCALE);
    ctx.move_to(x, y - baseline);
    pangocairo::functions::show_layout(ctx, &layout);
}

fn draw_text_right(
    ctx: &cairo::Context,
    family: &str,
    size: f64,
    right: f64,
    y: f64,
    max_width: f64,
    text: &str,
) {
    if max_width <= 0.0 {
        return;
    }
    let layout = bounded_layout(ctx, family, size, max_width, text);
    let width = (f64::from(layout.extents().1.width()) / f64::from(pango::SCALE)).min(max_width);
    let baseline = f64::from(layout.baseline()) / f64::from(pango::SCALE);
    ctx.move_to(right - width, y - baseline);
    pangocairo::functions::show_layout(ctx, &layout);
}

#[cfg(test)]
mod caption_tests {
    use super::share_line;

    #[test]
    fn two_labels_that_both_fit_keep_their_own_widths() {
        assert_eq!(share_line(400.0, 150.0, 120.0), (150.0, 120.0));
    }

    #[test]
    fn a_short_label_yields_its_slack_rather_than_its_share() {
        // The keys line is short; the caption gets everything it leaves.
        assert_eq!(share_line(400.0, 500.0, 120.0), (280.0, 120.0));
        assert_eq!(share_line(400.0, 120.0, 500.0), (120.0, 280.0));
    }

    #[test]
    fn two_labels_that_both_overrun_halve_the_line() {
        assert_eq!(share_line(400.0, 500.0, 500.0), (200.0, 200.0));
    }
}
