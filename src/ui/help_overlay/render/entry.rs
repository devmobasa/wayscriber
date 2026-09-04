use super::{HelpOverlayBindings, HelpRenderResult, hit, render_help_overlay_result_with_context};

/// Render help overlay showing all keybindings with call-local paint resources.
/// The overlay runtime uses the explicit-context entry point to retain its layout.
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
    let mut caches = crate::ui::UiRenderCaches::default();
    let theme = crate::ui::theme::Theme::dark();
    render_help_overlay_with_context(
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

/// Paint once and return owned scroll and hit geometry without changing the legacy map.
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_help_overlay_with_context(
    render: &mut crate::ui::UiRenderCtx<'_, '_, '_>,
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
    let result = render_help_overlay_result_with_context(
        render,
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
    );
    hit::store_help_hit_map(result.hit_map);
    result.scroll_max
}
