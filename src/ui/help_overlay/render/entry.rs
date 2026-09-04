use super::{HelpOverlayBindings, HelpRenderResult, render_help_overlay_result_with_context};

/// Render help overlay showing all keybindings with call-local paint resources.
/// Returns the maximum scroll offset and discards interaction geometry.
/// Interactive callers should use [`render_help_overlay_result`] instead.
#[allow(clippy::too_many_arguments)]
pub fn render_help_overlay(
    ctx: &cairo::Context,
    style: &crate::config::HelpOverlayStyle,
    screen_width: u32,
    screen_height: u32,
    frozen_enabled: bool,
    page_index: usize,
    bindings: &HelpOverlayBindings,
    search_query: &str,
    context_filter: bool,
    board_enabled: bool,
    capture_enabled: bool,
    scroll_offset: f64,
    quick_mode: bool,
) -> f64 {
    render_help_overlay_result(
        ctx,
        style,
        screen_width,
        screen_height,
        frozen_enabled,
        page_index,
        bindings,
        search_query,
        context_filter,
        board_enabled,
        capture_enabled,
        scroll_offset,
        quick_mode,
    )
    .scroll_max
}

/// Paint once and return owned scroll and hit geometry.
///
/// Query `result.hit_map.region_at(x, y)` directly, or pass the result to
/// [`crate::input::InputState::install_help_overlay_render_result`] before using
/// that input owner's click and cursor queries. Each caller retains its own map;
/// painting another overlay cannot replace it.
#[allow(clippy::too_many_arguments)]
pub fn render_help_overlay_result(
    ctx: &cairo::Context,
    style: &crate::config::HelpOverlayStyle,
    screen_width: u32,
    screen_height: u32,
    frozen_enabled: bool,
    page_index: usize,
    bindings: &HelpOverlayBindings,
    search_query: &str,
    context_filter: bool,
    board_enabled: bool,
    capture_enabled: bool,
    scroll_offset: f64,
    quick_mode: bool,
) -> HelpRenderResult {
    let mut caches = crate::ui::UiRenderCaches::default();
    let theme = crate::ui::theme::Theme::dark();
    render_help_overlay_result_with_context(
        &mut crate::ui::UiRenderCtx {
            cairo: ctx,
            theme: &theme,
            caches: &mut caches,
        },
        style,
        screen_width,
        screen_height,
        frozen_enabled,
        page_index,
        bindings,
        search_query,
        context_filter,
        board_enabled,
        capture_enabled,
        scroll_offset,
        quick_mode,
    )
}
