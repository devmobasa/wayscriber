use super::super::primitives::{BADGE_PADDING, BADGE_STACK_GAP, BadgeAlign, draw_badge};
use super::super::theme::overlay;

/// Vertical inset of the floating page badge from the screen edge.
const PAGE_BADGE_EDGE_PADDING: f64 = overlay::SPACING_SM;

/// Warning tint for the FROZEN badge — literal red for the safety state,
/// deliberately never abstracted behind the theme.
pub(crate) const FROZEN_BADGE_TINT: [f64; 4] = [0.82, 0.22, 0.2, 0.9];
/// Teal tint for the zoom badge.
pub(crate) const ZOOM_BADGE_TINT: [f64; 4] = [0.2, 0.52, 0.7, 0.9];
/// Olive tint for the pan badge.
pub(crate) const PAN_BADGE_TINT: [f64; 4] = [0.33, 0.44, 0.24, 0.92];
/// Teal accent tint for the text-editing badge.
pub(crate) const EDITING_BADGE_TINT: [f64; 4] = [0.2, 0.55, 0.65, 0.9];

// Shared badge specs (labels, font sizes, hint) consumed by both the
// top-corner badges below and the HUD-stacked pills in `bar.rs`
// (`layout_mode_badges`) so the two renderings cannot drift.

/// FROZEN badge label.
pub(crate) const FROZEN_BADGE_LABEL: &str = "FROZEN";
/// FROZEN badge label font size.
pub(crate) const FROZEN_BADGE_FONT_SIZE: f64 = 16.0;
/// Zoom badge label font size.
pub(crate) const ZOOM_BADGE_FONT_SIZE: f64 = 15.0;
/// Pan badge label font size.
pub(crate) const PAN_BADGE_FONT_SIZE: f64 = 14.0;
/// EDITING badge label.
pub(crate) const EDITING_BADGE_LABEL: &str = "EDITING";
/// EDITING badge label font size.
pub(crate) const EDITING_BADGE_FONT_SIZE: f64 = 15.0;
/// EDITING badge hint line shown below the label.
pub(crate) const EDITING_BADGE_HINT: (&str, f64) = ("Return=save  Esc=cancel", 11.0);

/// Zoom badge label ("ZOOM {pct}%", plus " LOCKED" when the zoom is locked).
pub(crate) fn zoom_badge_label(zoom_scale: f64, locked: bool) -> String {
    let pct = (zoom_scale * 100.0).round() as i32;
    if locked {
        format!("ZOOM {pct}% LOCKED")
    } else {
        format!("ZOOM {pct}%")
    }
}

/// Pan badge label in the HUD's mixed-case form; the top-corner badge
/// renders the same label uppercased.
pub(crate) fn pan_badge_label(panned: bool) -> &'static str {
    if panned {
        "PANNED Space+Drag"
    } else {
        "PAN Space+Drag"
    }
}

/// Render a small badge indicating frozen mode (visible even when status bar is hidden).
pub fn render_frozen_badge(ctx: &cairo::Context, screen_width: u32, _screen_height: u32) {
    draw_badge(
        ctx,
        screen_width as f64 - BADGE_PADDING,
        BADGE_PADDING,
        BadgeAlign::Right,
        FROZEN_BADGE_LABEL,
        FROZEN_BADGE_FONT_SIZE,
        None,
        FROZEN_BADGE_TINT,
    );
}

/// Render a small badge indicating zoom mode (visible even when status bar is hidden).
///
/// Returns the vertical space consumed (badge height plus stacking gap) so
/// callers can position the next stacked badge below it.
pub fn render_zoom_badge(
    ctx: &cairo::Context,
    screen_width: u32,
    _screen_height: u32,
    zoom_scale: f64,
    locked: bool,
) -> f64 {
    let label = zoom_badge_label(zoom_scale, locked);
    let height = draw_badge(
        ctx,
        screen_width as f64 - BADGE_PADDING,
        BADGE_PADDING,
        BadgeAlign::Right,
        &label,
        ZOOM_BADGE_FONT_SIZE,
        None,
        ZOOM_BADGE_TINT,
    );
    height + BADGE_STACK_GAP
}

/// Render a small badge indicating canvas pan is available on the active solid board.
///
/// Returns the vertical space consumed (badge height plus stacking gap) so
/// callers can position the next stacked badge below it.
pub fn render_pan_badge(
    ctx: &cairo::Context,
    screen_width: u32,
    _screen_height: u32,
    panned: bool,
    offset_y: f64,
) -> f64 {
    // Same label as the HUD-stacked pill, uppercased for the top corner
    // (historic form; keeps the rendering visually unchanged).
    let label = pan_badge_label(panned).to_uppercase();
    let height = draw_badge(
        ctx,
        screen_width as f64 - BADGE_PADDING,
        BADGE_PADDING + offset_y,
        BadgeAlign::Right,
        &label,
        PAN_BADGE_FONT_SIZE,
        None,
        PAN_BADGE_TINT,
    );
    height + BADGE_STACK_GAP
}

/// Render a small badge indicating text edit mode is active.
pub fn render_editing_badge(
    ctx: &cairo::Context,
    screen_width: u32,
    _screen_height: u32,
    offset_y: f64,
) {
    draw_badge(
        ctx,
        screen_width as f64 - BADGE_PADDING,
        BADGE_PADDING + offset_y,
        BadgeAlign::Right,
        EDITING_BADGE_LABEL,
        EDITING_BADGE_FONT_SIZE,
        Some(EDITING_BADGE_HINT),
        EDITING_BADGE_TINT,
    );
}

/// Render a small badge indicating the current page (visible even when status bar is hidden).
#[allow(clippy::too_many_arguments)]
pub fn render_page_badge(
    ctx: &cairo::Context,
    _screen_width: u32,
    _screen_height: u32,
    board_index: usize,
    board_count: usize,
    board_name: &str,
    page_index: usize,
    page_count: usize,
) {
    let truncated_name = crate::util::truncate_with_ellipsis(board_name, 20);
    let board_label = if !truncated_name.trim().is_empty() {
        if board_count > 1 {
            Some(format!(
                "Board {}/{}: {}",
                board_index + 1,
                board_count.max(1),
                truncated_name
            ))
        } else {
            Some(format!("Board: {}", truncated_name))
        }
    } else if board_count > 1 {
        Some(format!("Board {}/{}", board_index + 1, board_count.max(1)))
    } else {
        None
    };
    let page_label = if page_count > 1 {
        Some(format!("Page {}/{}", page_index + 1, page_count.max(1)))
    } else {
        None
    };
    let label = match (board_label, page_label) {
        (Some(board), Some(page)) => format!("{board} | {page}"),
        (Some(board), None) => board,
        (None, Some(page)) => page,
        (None, None) => return,
    };
    draw_badge(
        ctx,
        BADGE_PADDING,
        PAGE_BADGE_EDGE_PADDING,
        BadgeAlign::Left,
        &label,
        15.0,
        None,
        // Neutral cool tone
        [0.2, 0.32, 0.45, 0.92],
    );
}
