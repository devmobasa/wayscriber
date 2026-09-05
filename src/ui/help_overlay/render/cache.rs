use super::super::content::HelpContentSnapshot;
use super::state::{OverlayLayout, build_overlay_layout};

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
    page_index: usize,
    content_revision: u64,
    search_query: String,
    quick_mode: bool,
}

struct CachedLayout {
    key: LayoutCacheKey,
    layout: OverlayLayout,
}

/// One help layout retained for one UI rendering owner.
#[derive(Default)]
pub(in crate::ui) struct HelpLayoutCache {
    entry: Option<CachedLayout>,
    #[cfg(test)]
    builds: usize,
}

impl HelpLayoutCache {
    /// Get or build the overlay layout, using cached version if inputs haven't changed.
    ///
    /// This avoids expensive text measurement and grid layout computations on every
    /// render frame when the help overlay is visible.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn get_or_build_overlay_layout(
        &mut self,
        engine: &crate::ui_text::UiTextEngine,
        ctx: &cairo::Context,
        style: &crate::config::HelpOverlayStyle,
        screen_width: u32,
        screen_height: u32,
        page_index: usize,
        content: &HelpContentSnapshot,
        search_query: &str,
        scroll_offset: f64,
        title_text: &str,
        header: &super::header::HeaderContent<'_>,
        note_text_base: &str,
        close_hint_text: &str,
        quick_mode: bool,
    ) -> OverlayLayout {
        let key = LayoutCacheKey {
            style: StyleKey::from_style(style),
            screen_width,
            screen_height,
            page_index,
            content_revision: content.revision,
            search_query: search_query.to_string(),
            quick_mode,
        };

        // Check if we have a valid cached layout
        if let Some(cached) = self.entry.as_ref().filter(|c| c.key == key) {
            // Cache hit - just update scroll offset and return
            let mut layout = cached.layout.clone();
            layout.scroll_offset = scroll_offset.clamp(0.0, layout.scroll_max);
            return layout;
        }

        #[cfg(test)]
        {
            self.builds += 1;
        }
        // Cache miss - build new layout
        let layout = build_overlay_layout(
            engine,
            ctx,
            style,
            screen_width,
            screen_height,
            page_index,
            content,
            search_query,
            scroll_offset,
            title_text,
            header,
            note_text_base,
            close_hint_text,
            quick_mode,
        );

        // Store in cache
        self.entry = Some(CachedLayout {
            key,
            layout: layout.clone(),
        });

        layout
    }
}

#[cfg(test)]
#[path = "tests/cache.rs"]
mod tests;
