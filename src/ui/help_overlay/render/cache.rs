use super::super::sections::HelpOverlayBindings;
use super::state::{OverlayLayout, build_overlay_layout};
use std::cell::RefCell;

/// Style fields converted to integers for stable comparison.
/// f64 values are stored as hundredths to avoid floating-point comparison issues.
#[derive(Clone, PartialEq)]
struct StyleKey {
    font_size_hundredths: i32,
    font_family: String,
    line_height_hundredths: i32,
    padding_hundredths: i32,
    bg_color: [i32; 4],
    border_color: [i32; 4],
    border_width_hundredths: i32,
    text_color: [i32; 4],
}

impl StyleKey {
    fn from_style(style: &crate::config::HelpOverlayStyle) -> Self {
        fn to_hundredths(v: f64) -> i32 {
            (v * 100.0).round() as i32
        }
        fn color_to_int(c: [f64; 4]) -> [i32; 4] {
            [
                to_hundredths(c[0]),
                to_hundredths(c[1]),
                to_hundredths(c[2]),
                to_hundredths(c[3]),
            ]
        }
        Self {
            font_size_hundredths: to_hundredths(style.font_size),
            font_family: style.font_family.clone(),
            line_height_hundredths: to_hundredths(style.line_height),
            padding_hundredths: to_hundredths(style.padding),
            bg_color: color_to_int(style.bg_color),
            border_color: color_to_int(style.border_color),
            border_width_hundredths: to_hundredths(style.border_width),
            text_color: color_to_int(style.text_color),
        }
    }
}

/// Cache key for help overlay layout.
/// Includes all parameters that affect layout computations
/// (style, text measurement, grid building). Scroll offset is handled separately.
#[derive(Clone, PartialEq)]
struct LayoutCacheKey {
    style: StyleKey,
    screen_width: u32,
    screen_height: u32,
    frozen_enabled: bool,
    page_index: usize,
    bindings_key: String,
    search_query: String,
    context_filter: bool,
    board_enabled: bool,
    capture_enabled: bool,
    quick_mode: bool,
}

struct CachedLayout {
    key: LayoutCacheKey,
    layout: OverlayLayout,
}

thread_local! {
    static LAYOUT_CACHE: RefCell<Option<CachedLayout>> = const { RefCell::new(None) };
}

/// Get or build the overlay layout, using cached version if inputs haven't changed.
///
/// This avoids expensive text measurement and grid layout computations on every
/// render frame when the help overlay is visible.
#[allow(clippy::too_many_arguments)]
pub(super) fn get_or_build_overlay_layout(
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
    title_text: &str,
    version_line: &str,
    note_text_base: &str,
    close_hint_text: &str,
    quick_mode: bool,
) -> OverlayLayout {
    let key = LayoutCacheKey {
        style: StyleKey::from_style(style),
        screen_width,
        screen_height,
        frozen_enabled,
        page_index,
        bindings_key: bindings.cache_key().to_string(),
        search_query: search_query.to_string(),
        context_filter,
        board_enabled,
        capture_enabled,
        quick_mode,
    };

    LAYOUT_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();

        // Check if we have a valid cached layout
        if let Some(cached) = cache.as_ref().filter(|c| c.key == key) {
            // Cache hit - just update scroll offset and return
            let mut layout = cached.layout.clone();
            layout.scroll_offset = scroll_offset.clamp(0.0, layout.scroll_max);
            return layout;
        }

        // Cache miss - build new layout
        let layout = build_overlay_layout(
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
            title_text,
            version_line,
            note_text_base,
            close_hint_text,
            quick_mode,
        );

        // Store in cache
        *cache = Some(CachedLayout {
            key,
            layout: layout.clone(),
        });

        layout
    })
}

/// Invalidate the help overlay layout cache.
/// Call this when style settings change.
#[allow(dead_code)]
pub fn invalidate_help_overlay_cache() {
    LAYOUT_CACHE.with(|cache| {
        *cache.borrow_mut() = None;
    });
}
