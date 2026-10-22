use crate::config::HelpOverlayStyle;
use crate::ui::UiRenderCaches;
use crate::ui::theme::Theme;
use crate::ui_text::UiTextEngine;

use super::{
    HelpContentOptions, HelpOverlayBindings, HelpRenderResult,
    render_help_overlay_result_with_context,
};

/// Render help overlay showing all keybindings with call-local paint resources.
/// Returns the maximum scroll offset and discards interaction geometry.
/// Interactive callers should use [`render_help_overlay_result`] instead.
#[allow(clippy::too_many_arguments)]
#[expect(
    clippy::must_use_candidate,
    reason = "painting is the purpose; drawing-only callers may ignore the scroll range"
)]
pub fn render_help_overlay(
    ctx: &cairo::Context,
    style: &HelpOverlayStyle,
    screen_width: u32,
    screen_height: u32,
    options: HelpContentOptions,
    page_index: usize,
    bindings: &HelpOverlayBindings,
    search_query: &str,
    scroll_offset: f64,
    quick_mode: bool,
) -> f64 {
    render_help_overlay_result(
        ctx,
        style,
        screen_width,
        screen_height,
        options,
        page_index,
        bindings,
        search_query,
        scroll_offset,
        quick_mode,
    )
    .scroll_max
}

/// Paint once and return owned scroll and hit geometry. Actions without a
/// binding stay hidden, as they are when help opens.
///
/// Query `result.hit_map.region_at(x, y)` directly, or pass the result to
/// [`crate::input::InputState::install_help_overlay_render_result`] before using
/// that input owner's click and cursor queries. Each caller retains its own map;
/// painting another overlay cannot replace it.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn render_help_overlay_result(
    ctx: &cairo::Context,
    style: &HelpOverlayStyle,
    screen_width: u32,
    screen_height: u32,
    options: HelpContentOptions,
    page_index: usize,
    bindings: &HelpOverlayBindings,
    search_query: &str,
    scroll_offset: f64,
    quick_mode: bool,
) -> HelpRenderResult {
    let engine = UiTextEngine::default();
    let mut caches = UiRenderCaches::default();
    let theme = Theme::dark();
    render_help_overlay_result_with_context(
        &engine,
        &mut crate::ui::UiRenderCtx {
            cairo: ctx,
            theme: &theme,
            caches: &mut caches,
        },
        style,
        screen_width,
        screen_height,
        options,
        page_index,
        bindings,
        search_query,
        scroll_offset,
        quick_mode,
        false,
    )
}
