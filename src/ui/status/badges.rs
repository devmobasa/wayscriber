use super::super::primitives::{BADGE_PADDING, BADGE_STACK_GAP, BadgeAlign, draw_badge};
use super::super::theme::overlay;

/// Vertical inset of the floating page badge from the screen edge.
const PAGE_BADGE_EDGE_PADDING: f64 = overlay::SPACING_SM;

/// Render a small badge indicating frozen mode (visible even when status bar is hidden).
pub fn render_frozen_badge(ctx: &cairo::Context, screen_width: u32, _screen_height: u32) {
    draw_badge(
        ctx,
        screen_width as f64 - BADGE_PADDING,
        BADGE_PADDING,
        BadgeAlign::Right,
        "FROZEN",
        16.0,
        None,
        // Warning tint
        [0.82, 0.22, 0.2, 0.9],
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
    let zoom_pct = (zoom_scale * 100.0).round() as i32;
    let label = if locked {
        format!("ZOOM {}% LOCKED", zoom_pct)
    } else {
        format!("ZOOM {}%", zoom_pct)
    };
    let height = draw_badge(
        ctx,
        screen_width as f64 - BADGE_PADDING,
        BADGE_PADDING,
        BadgeAlign::Right,
        &label,
        15.0,
        None,
        // Teal tint
        [0.2, 0.52, 0.7, 0.9],
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
    let label = if panned {
        "PANNED SPACE+DRAG"
    } else {
        "PAN SPACE+DRAG"
    };
    let height = draw_badge(
        ctx,
        screen_width as f64 - BADGE_PADDING,
        BADGE_PADDING + offset_y,
        BadgeAlign::Right,
        label,
        14.0,
        None,
        // Olive tint
        [0.33, 0.44, 0.24, 0.92],
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
        "EDITING",
        15.0,
        Some(("Return=save  Esc=cancel", 11.0)),
        // Teal accent
        [0.2, 0.55, 0.65, 0.9],
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
