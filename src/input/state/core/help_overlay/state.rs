use crate::help_overlay_interaction::{HelpHitMap, HelpOverlayRegion, HelpRenderResult};
use crate::input::state::{HelpOverlayClick, HelpOverlayPressSource, HelpOverlayReleaseOutcome};

/// Upper bound for page navigation. Rendering clamps this to the actual page count.
pub(crate) const MAX_PAGES: usize = 10;

/// Visibility, navigation, search, and pointer bookkeeping for the help overlay.
#[derive(Debug, Default)]
pub struct HelpOverlayState {
    hit_map: Option<HelpHitMap>,
    pub(in crate::input::state) visible: bool,
    pub(in crate::input::state) page: usize,
    pub(in crate::input::state) search: String,
    pub(in crate::input::state) search_cursor: usize,
    pub(in crate::input::state) scroll: f64,
    pub(in crate::input::state) scroll_max: f64,
    pub(in crate::input::state) pending_presses: Vec<(HelpOverlayPressSource, HelpOverlayClick)>,
    pub(in crate::input::state) consume_only_presses: Vec<HelpOverlayPressSource>,
    pub(in crate::input::state) quick_mode: bool,
}

impl HelpOverlayState {
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn page(&self) -> usize {
        self.page
    }

    pub fn query(&self) -> &str {
        &self.search
    }

    pub fn scroll(&self) -> f64 {
        self.scroll
    }

    pub fn is_quick_mode(&self) -> bool {
        self.quick_mode
    }

    /// Install geometry and scroll bounds from the same completed help paint.
    pub fn install_render_result(&mut self, result: HelpRenderResult) {
        self.update_scroll_extent(result.scroll_max);
        self.hit_map = Some(result.hit_map);
    }

    /// Query this overlay's last rendered geometry in screen coordinates.
    pub fn region_at(&self, x: f64, y: f64) -> Option<HelpOverlayRegion> {
        self.hit_map.as_ref()?.region_at(x, y)
    }

    pub(crate) fn open(&mut self, quick_mode: bool) {
        self.hit_map = None;
        self.visible = true;
        self.quick_mode = quick_mode;
        self.page = 0;
        self.scroll = 0.0;
        self.scroll_max = 0.0;
        self.retire_press_targets();
    }

    pub(crate) fn close(&mut self) -> bool {
        if !self.visible {
            return false;
        }
        self.hit_map = None;
        self.visible = false;
        self.quick_mode = false;
        self.scroll = 0.0;
        self.scroll_max = 0.0;
        self.retire_press_targets();
        true
    }

    pub(crate) fn note_press(&mut self, source: HelpOverlayPressSource, target: HelpOverlayClick) {
        if let Some(index) = self
            .consume_only_presses
            .iter()
            .position(|pending_source| *pending_source == source)
        {
            self.consume_only_presses.swap_remove(index);
        }
        if let Some((_, pending_target)) = self
            .pending_presses
            .iter_mut()
            .find(|(pending_source, _)| *pending_source == source)
        {
            *pending_target = target;
        } else {
            self.pending_presses.push((source, target));
        }
    }

    pub(crate) fn clear_press_for(&mut self, source: HelpOverlayPressSource) -> bool {
        let mut removed = remove_source(&mut self.pending_presses, source);
        if let Some(index) = self
            .consume_only_presses
            .iter()
            .position(|pending_source| *pending_source == source)
        {
            self.consume_only_presses.swap_remove(index);
            removed = true;
        }
        removed
    }

    pub(crate) fn resolve_release(
        &mut self,
        source: HelpOverlayPressSource,
        released: HelpOverlayClick,
    ) -> Option<HelpOverlayReleaseOutcome> {
        if !self.visible {
            return self
                .clear_press_for(source)
                .then_some(HelpOverlayReleaseOutcome::None);
        }
        if let Some(index) = self
            .consume_only_presses
            .iter()
            .position(|pending_source| *pending_source == source)
        {
            self.consume_only_presses.swap_remove(index);
            return Some(HelpOverlayReleaseOutcome::None);
        }
        let index = self
            .pending_presses
            .iter()
            .position(|(pressed_source, _)| *pressed_source == source)?;
        let (_, pressed) = self.pending_presses.swap_remove(index);
        Some(match (pressed, released) {
            (HelpOverlayClick::Run(pressed), HelpOverlayClick::Run(released))
                if pressed == released =>
            {
                HelpOverlayReleaseOutcome::Run(released)
            }
            (HelpOverlayClick::Outside, HelpOverlayClick::Outside) => {
                HelpOverlayReleaseOutcome::Dismiss
            }
            _ => HelpOverlayReleaseOutcome::None,
        })
    }

    pub(crate) fn scroll_by(&mut self, delta: f64) -> bool {
        let next = if self.scroll_max > 0.0 {
            (self.scroll + delta).clamp(0.0, self.scroll_max)
        } else {
            (self.scroll + delta).max(0.0)
        };
        if (next - self.scroll).abs() <= f64::EPSILON {
            return false;
        }
        self.scroll = next;
        true
    }

    pub(crate) fn update_scroll_extent(&mut self, scroll_max: f64) {
        self.scroll_max = scroll_max;
        self.scroll = self.scroll.clamp(0.0, scroll_max);
    }

    pub(crate) fn next_page(&mut self) -> bool {
        if self.page + 1 >= MAX_PAGES {
            return false;
        }
        self.page += 1;
        self.scroll = 0.0;
        true
    }

    pub(crate) fn previous_page(&mut self) -> bool {
        if self.page == 0 {
            return false;
        }
        self.page -= 1;
        self.scroll = 0.0;
        true
    }

    pub(crate) fn first_page(&mut self) -> bool {
        self.set_page(0)
    }

    pub(crate) fn last_page(&mut self) -> bool {
        self.set_page(MAX_PAGES - 1)
    }

    fn set_page(&mut self, page: usize) -> bool {
        if self.page == page {
            return false;
        }
        self.page = page;
        self.scroll = 0.0;
        true
    }

    pub(crate) fn clear_search(&mut self) -> bool {
        if self.search.is_empty() && self.search_cursor == 0 {
            return false;
        }
        self.search.clear();
        self.search_cursor = 0;
        self.scroll = 0.0;
        true
    }

    pub(crate) fn insert_search(&mut self, text: &str) {
        let byte_index = self
            .search
            .char_indices()
            .nth(self.search_cursor)
            .map_or(self.search.len(), |(index, _)| index);
        self.search.insert_str(byte_index, text);
        self.search_cursor += text.chars().count();
        self.scroll = 0.0;
    }

    pub(crate) fn backspace_search(&mut self) -> bool {
        if self.search_cursor == 0 {
            return false;
        }
        let start = self
            .search
            .char_indices()
            .nth(self.search_cursor - 1)
            .map_or(0, |(index, _)| index);
        let end = self
            .search
            .char_indices()
            .nth(self.search_cursor)
            .map_or(self.search.len(), |(index, _)| index);
        self.search.replace_range(start..end, "");
        self.search_cursor -= 1;
        self.scroll = 0.0;
        true
    }

    fn retire_press_targets(&mut self) {
        for (source, _) in self.pending_presses.drain(..) {
            if !self.consume_only_presses.contains(&source) {
                self.consume_only_presses.push(source);
            }
        }
    }
}

fn remove_source(
    presses: &mut Vec<(HelpOverlayPressSource, HelpOverlayClick)>,
    source: HelpOverlayPressSource,
) -> bool {
    let Some(index) = presses
        .iter()
        .position(|(pressed_source, _)| *pressed_source == source)
    else {
        return false;
    };
    presses.swap_remove(index);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Action;

    #[test]
    fn scrolling_clamps_to_the_rendered_extent() {
        let mut state = HelpOverlayState {
            scroll_max: 100.0,
            ..Default::default()
        };
        assert!(state.scroll_by(48.0));
        assert_eq!(state.scroll, 48.0);
        assert!(state.scroll_by(96.0));
        assert_eq!(state.scroll, 100.0);
        assert!(!state.scroll_by(1.0));
        state.note_press(HelpOverlayPressSource::Touch, HelpOverlayClick::Inside);
        state.install_render_result(HelpRenderResult {
            scroll_max: 24.0,
            hit_map: HelpHitMap::new((0.0, 0.0, 100.0, 100.0), None, []),
        });
        assert_eq!(state.region_at(50.0, 50.0), Some(HelpOverlayRegion::Inside));
        assert_eq!(state.pending_presses.len(), 1);
        assert_eq!((state.scroll, state.scroll_max), (24.0, 24.0));
    }

    #[test]
    fn navigation_clamps_to_the_page_bound_and_resets_scroll() {
        let mut state = HelpOverlayState {
            scroll: 42.0,
            ..Default::default()
        };
        assert!(state.next_page());
        assert_eq!((state.page, state.scroll), (1, 0.0));
        assert!(state.last_page());
        assert_eq!(state.page, MAX_PAGES - 1);
        assert!(!state.next_page());
        assert!(state.previous_page());
        assert!(state.first_page());
        assert!(!state.previous_page());
    }

    #[test]
    fn unicode_search_editing_tracks_scalar_cursor_positions() {
        let mut state = HelpOverlayState::default();
        state.insert_search("a🙂b");
        assert!(state.backspace_search());
        assert_eq!(state.search, "a🙂");
        assert_eq!(state.search_cursor, 2);
        state.insert_search("é");
        assert_eq!(state.search, "a🙂é");
        assert_eq!(state.search_cursor, 3);
        assert!(state.clear_search());
        assert_eq!((state.search.as_str(), state.search_cursor), ("", 0));
    }

    #[test]
    fn press_release_requires_matching_source_and_target() {
        let mut state = HelpOverlayState::default();
        state.open(false);
        state.note_press(
            HelpOverlayPressSource::Pointer(1),
            HelpOverlayClick::Run(Action::ClearCanvas),
        );
        assert_eq!(
            state.resolve_release(
                HelpOverlayPressSource::Touch,
                HelpOverlayClick::Run(Action::ClearCanvas)
            ),
            None
        );
        assert_eq!(
            state.resolve_release(
                HelpOverlayPressSource::Pointer(1),
                HelpOverlayClick::Run(Action::ToggleHelp)
            ),
            Some(HelpOverlayReleaseOutcome::None)
        );
    }

    #[test]
    fn closing_retires_press_without_retargeting_a_reopened_overlay() {
        let mut state = HelpOverlayState::default();
        state.open(false);
        state.install_render_result(HelpRenderResult {
            scroll_max: 40.0,
            hit_map: HelpHitMap::new((0.0, 0.0, 100.0, 100.0), None, []),
        });
        assert_eq!(state.region_at(50.0, 50.0), Some(HelpOverlayRegion::Inside));
        state.note_press(HelpOverlayPressSource::Touch, HelpOverlayClick::Outside);
        assert!(state.close());
        assert_eq!(state.region_at(50.0, 50.0), None);
        state.open(false);
        assert_eq!(state.region_at(50.0, 50.0), None);
        assert_eq!(
            state.resolve_release(HelpOverlayPressSource::Touch, HelpOverlayClick::Outside),
            Some(HelpOverlayReleaseOutcome::None)
        );
    }
}
