use super::super::sections::HelpOverlayBindings;
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

/// Measured layout retained by one help-overlay renderer.
pub(super) struct HelpOverlayLayoutCache {
    cached: Option<CachedLayout>,
}

impl HelpOverlayLayoutCache {
    pub(super) fn new() -> Self {
        Self { cached: None }
    }

    /// Get or build the overlay layout, using the retained version when its
    /// complete measurement key still matches.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn get_or_build(
        &mut self,
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
        header: &super::header::HeaderContent<'_>,
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

        if let Some(cached) = self.cached.as_ref().filter(|cached| cached.key == key) {
            let mut layout = cached.layout.clone();
            layout.scroll_offset = scroll_offset.clamp(0.0, layout.scroll_max);
            return layout;
        }

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
            header,
            note_text_base,
            close_hint_text,
            quick_mode,
        );

        self.cached = Some(CachedLayout {
            key,
            layout: layout.clone(),
        });

        layout
    }

    pub(super) fn invalidate(&mut self) {
        self.cached = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HelpOverlayStyle;

    fn populate(cache: &mut HelpOverlayLayoutCache, width: u32, height: u32) {
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, width as i32, height as i32)
                .expect("the help-cache test fixture uses valid positive surface dimensions");
        let context = cairo::Context::new(&surface)
            .expect("the help-cache test fixture creates a context from its image surface");
        let bindings = HelpOverlayBindings::default();
        let hints = [];
        let header = super::super::header::HeaderContent {
            version: "v-test",
            intro: None,
            hints: &hints,
        };

        cache.get_or_build(
            &context,
            &HelpOverlayStyle::default(),
            width,
            height,
            false,
            0,
            &bindings,
            "",
            false,
            true,
            true,
            0.0,
            "Wayscriber Controls",
            &header,
            "Note",
            "F1 / Esc to close",
            false,
        );
    }

    #[test]
    fn independent_cache_owners_do_not_share_layout_or_invalidation() {
        let mut first = HelpOverlayLayoutCache::new();
        let mut second = HelpOverlayLayoutCache::new();

        populate(&mut first, 800, 600);
        assert_eq!(
            first.cached.as_ref().map(|cached| cached.key.screen_width),
            Some(800)
        );
        assert!(second.cached.is_none());

        populate(&mut second, 1024, 768);
        first.invalidate();

        assert!(first.cached.is_none());
        assert_eq!(
            second.cached.as_ref().map(|cached| cached.key.screen_width),
            Some(1024)
        );
    }
}
