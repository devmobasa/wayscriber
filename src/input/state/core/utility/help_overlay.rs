use super::super::base::InputState;

/// Upper bound for page navigation. The actual page count is calculated
/// dynamically by the render state. Navigation clamps to the actual count.
const HELP_OVERLAY_MAX_PAGES: usize = 10;

/// Cursor hint for the help overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpOverlayCursorHint {
    /// Default arrow cursor.
    Default,
    /// Text editing cursor (I-beam) for search input.
    Text,
    /// Pointer / hand cursor over a clickable help row or footer action.
    Pointer,
}

/// Outcome of a left-click inside the (open) help overlay, resolved against the
/// real rendered layout via the overlay's pointer hit map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpOverlayClick {
    /// A clickable row (or the "Replay tour" footer) was hit; run this action.
    Run(crate::config::Action),
    /// Inside the overlay chrome but not on an interactive element (no-op).
    Inside,
    /// Outside the overlay box entirely — treated as a dismiss click.
    Outside,
}

/// Pointing modality that owns a pending help-overlay press.
///
/// Releases only resolve the target recorded by the same modality. This lets
/// a canvas gesture that began before help opened finish normally instead of
/// being mistaken for a help click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HelpOverlayPressSource {
    /// Raw Linux pointer button code, so middle/right ownership cannot be
    /// confused with a left-click help action.
    Pointer(u32),
    Touch,
    #[cfg(feature = "tablet-input")]
    Stylus,
}

/// What a completed left press+release gesture over the help overlay should do,
/// after enforcing the same-target contract between the press and the release
/// (see [`InputState::resolve_help_overlay_release`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpOverlayReleaseOutcome {
    /// Press and release landed on the SAME clickable row; run its action.
    Run(crate::config::Action),
    /// Press and release both landed outside the overlay box; dismiss it.
    Dismiss,
    /// Anything else (mismatched targets, bare chrome, no recorded press):
    /// leave the overlay untouched.
    None,
}

impl InputState {
    fn open_help_overlay_internal(&mut self, quick_mode: bool, track_usage: bool) {
        self.close_radial_menu();
        self.show_help = true;
        self.help_overlay_quick_mode = quick_mode;
        self.help_overlay_scroll = 0.0;
        self.help_overlay_scroll_max = 0.0;
        // Defensively drop any geometry left from a previous open. The hit map
        // is normally cleared on close, but re-opening should never expose the
        // prior layout to a click before the first fresh render repopulates it.
        self.retire_help_overlay_press_targets();
        crate::ui::clear_help_overlay_hit_map();
        if track_usage {
            self.pending_onboarding_usage.used_help_overlay = true;
        }
        self.help_overlay_page = 0;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn toggle_help_overlay(&mut self) {
        if self.show_help {
            self.close_help_overlay();
            return;
        }
        self.open_help_overlay_internal(false, true);
    }

    pub(crate) fn toggle_quick_help(&mut self) {
        if self.show_help && self.help_overlay_quick_mode {
            self.close_help_overlay();
            return;
        }
        self.open_help_overlay_internal(true, true);
    }

    /// Close the help overlay and drop the stale pointer hit map so a later
    /// click can never act on the previous frame's rectangles.
    pub(crate) fn close_help_overlay(&mut self) {
        if !self.show_help {
            return;
        }
        self.show_help = false;
        self.help_overlay_quick_mode = false;
        self.help_overlay_scroll = 0.0;
        self.help_overlay_scroll_max = 0.0;
        self.retire_help_overlay_press_targets();
        crate::ui::clear_help_overlay_hit_map();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Resolve a left-click at `(x, y)` (screen space) against the real rendered
    /// help layout: a clickable row/footer action, inside chrome, or a dismiss.
    pub fn help_overlay_click_at(&self, x: i32, y: i32) -> HelpOverlayClick {
        match crate::ui::help_overlay_region_at(x as f64, y as f64) {
            Some(crate::ui::HelpOverlayRegion::Row(action)) => HelpOverlayClick::Run(action),
            Some(_) => HelpOverlayClick::Inside,
            None => HelpOverlayClick::Outside,
        }
    }

    /// Record the help target under a press (screen space) so the matching
    /// release can enforce source ownership and, for left clicks, a same-target
    /// contract. Mirrors the toast press guard: the press only *marks* intent,
    /// never acts.
    pub(crate) fn note_help_overlay_press(
        &mut self,
        source: HelpOverlayPressSource,
        x: i32,
        y: i32,
    ) {
        // A new physical press supersedes any consume-only token left by a
        // release the compositor never delivered for this source.
        if let Some(index) = self
            .help_overlay_consume_only_presses
            .iter()
            .position(|pending_source| *pending_source == source)
        {
            self.help_overlay_consume_only_presses.swap_remove(index);
        }
        let target = self.help_overlay_click_at(x, y);
        if let Some((_, pending_target)) = self
            .help_overlay_pending_presses
            .iter_mut()
            .find(|(pending_source, _)| *pending_source == source)
        {
            *pending_target = target;
        } else {
            self.help_overlay_pending_presses.push((source, target));
        }
    }

    /// Clear a pending help press only when it belongs to `source`. Returns
    /// whether this modality owned the press and therefore owns and swallows its
    /// eventual release.
    pub(crate) fn clear_help_overlay_press_for(&mut self, source: HelpOverlayPressSource) -> bool {
        let mut removed = false;
        if let Some(index) = self
            .help_overlay_pending_presses
            .iter()
            .position(|(pressed_source, _)| *pressed_source == source)
        {
            self.help_overlay_pending_presses.swap_remove(index);
            removed = true;
        }
        if let Some(index) = self
            .help_overlay_consume_only_presses
            .iter()
            .position(|pending_source| *pending_source == source)
        {
            self.help_overlay_consume_only_presses.swap_remove(index);
            removed = true;
        }
        removed
    }

    /// Resolve a left release at `(x, y)` (screen space) against the target
    /// recorded by [`Self::note_help_overlay_press`], enforcing a same-target
    /// contract before acting. A row runs only when the release lands on the
    /// SAME row as the press, so pressing on bare chrome (or outside) and
    /// dragging onto a clickable row — e.g. the destructive Clear row — never
    /// fires it. A dismiss requires the press and release to both fall outside
    /// the box. Consumes the recorded press.
    pub(crate) fn resolve_help_overlay_release(
        &mut self,
        source: HelpOverlayPressSource,
        x: i32,
        y: i32,
    ) -> Option<HelpOverlayReleaseOutcome> {
        if !self.show_help {
            return self
                .clear_help_overlay_press_for(source)
                .then_some(HelpOverlayReleaseOutcome::None);
        }
        if let Some(index) = self
            .help_overlay_consume_only_presses
            .iter()
            .position(|pending_source| *pending_source == source)
        {
            self.help_overlay_consume_only_presses.swap_remove(index);
            return Some(HelpOverlayReleaseOutcome::None);
        }
        let index = self
            .help_overlay_pending_presses
            .iter()
            .position(|(pressed_source, _)| *pressed_source == source)?;
        let pressed = self.help_overlay_pending_presses.swap_remove(index);
        let released = self.help_overlay_click_at(x, y);
        Some(match (pressed.1, released) {
            (HelpOverlayClick::Run(pressed_action), HelpOverlayClick::Run(released_action))
                if pressed_action == released_action =>
            {
                HelpOverlayReleaseOutcome::Run(released_action)
            }
            (HelpOverlayClick::Outside, HelpOverlayClick::Outside) => {
                HelpOverlayReleaseOutcome::Dismiss
            }
            _ => HelpOverlayReleaseOutcome::None,
        })
    }

    /// Invalidate action targets from the current help layout while retaining
    /// ownership of their physical releases. This prevents releases from
    /// leaking into a surface opened after help closes, without allowing an
    /// old press to act against a newly rendered help layout.
    fn retire_help_overlay_press_targets(&mut self) {
        for (source, _) in self.help_overlay_pending_presses.drain(..) {
            if !self.help_overlay_consume_only_presses.contains(&source) {
                self.help_overlay_consume_only_presses.push(source);
            }
        }
    }

    pub(crate) fn help_overlay_next_page(&mut self) -> bool {
        // Use upper bound; render state clamps to actual page count
        let next_page = self.help_overlay_page + 1;
        if next_page < HELP_OVERLAY_MAX_PAGES {
            self.help_overlay_page = next_page;
            self.help_overlay_scroll = 0.0;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return true;
        }
        false
    }

    pub(crate) fn help_overlay_prev_page(&mut self) -> bool {
        if self.help_overlay_page > 0 {
            self.help_overlay_page -= 1;
            self.help_overlay_scroll = 0.0;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    /// Clear help search and reset cursor position.
    #[allow(dead_code)]
    pub(crate) fn clear_help_search(&mut self) {
        self.help_overlay_search.clear();
        self.help_overlay_search_cursor = 0;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Move help search cursor left.
    #[allow(dead_code)]
    pub(crate) fn help_search_cursor_left(&mut self) {
        if self.help_overlay_search_cursor > 0 {
            // Move back by one character (handle UTF-8 properly)
            let text = &self.help_overlay_search;
            if let Some((idx, _)) = text
                .char_indices()
                .take(self.help_overlay_search_cursor)
                .last()
            {
                self.help_overlay_search_cursor = text[..idx].chars().count();
            }
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Move help search cursor right.
    #[allow(dead_code)]
    pub(crate) fn help_search_cursor_right(&mut self) {
        let char_count = self.help_overlay_search.chars().count();
        if self.help_overlay_search_cursor < char_count {
            self.help_overlay_search_cursor += 1;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Insert text at cursor position.
    #[allow(dead_code)]
    pub(crate) fn help_search_insert(&mut self, text: &str) {
        let cursor = self.help_overlay_search_cursor;
        let current = &self.help_overlay_search;
        let byte_idx = current
            .char_indices()
            .nth(cursor)
            .map(|(i, _)| i)
            .unwrap_or(current.len());
        self.help_overlay_search.insert_str(byte_idx, text);
        self.help_overlay_search_cursor += text.chars().count();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Delete character before cursor (backspace).
    #[allow(dead_code)]
    pub(crate) fn help_search_backspace(&mut self) {
        if self.help_overlay_search_cursor > 0 {
            let current = &self.help_overlay_search;
            let cursor = self.help_overlay_search_cursor;
            // Find byte index of previous character
            let char_indices: Vec<_> = current.char_indices().collect();
            if cursor <= char_indices.len() {
                let _start_idx = if cursor >= 2 {
                    char_indices[cursor - 2].0 + char_indices[cursor - 2].1.len_utf8()
                } else {
                    0
                };
                let end_idx = if cursor - 1 < char_indices.len() {
                    char_indices[cursor - 1].0 + char_indices[cursor - 1].1.len_utf8()
                } else {
                    current.len()
                };
                self.help_overlay_search
                    .replace_range(char_indices[cursor - 1].0..end_idx, "");
                self.help_overlay_search_cursor -= 1;
            }
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Determine the cursor type for the help overlay.
    /// Returns `None` if the help overlay is not open, or the point is outside
    /// the overlay box.
    ///
    /// Resolved against the real rendered layout (the overlay's pointer hit
    /// map): the search well shows a text cursor, clickable rows and the
    /// "Replay tour" footer show a pointer, everything else the default.
    pub fn help_overlay_cursor_hint_at(&self, x: i32, y: i32) -> Option<HelpOverlayCursorHint> {
        if !self.show_help {
            return None;
        }

        match crate::ui::help_overlay_region_at(x as f64, y as f64)? {
            crate::ui::HelpOverlayRegion::Search => Some(HelpOverlayCursorHint::Text),
            crate::ui::HelpOverlayRegion::Row(_) => Some(HelpOverlayCursorHint::Pointer),
            crate::ui::HelpOverlayRegion::Inside => Some(HelpOverlayCursorHint::Default),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        )
    }

    #[test]
    fn toggle_help_overlay_opens_and_tracks_usage() {
        let mut state = make_state();
        state.toggle_help_overlay();

        assert!(state.show_help);
        assert!(!state.help_overlay_quick_mode);
        assert!(state.pending_onboarding_usage.used_help_overlay);
        assert_eq!(state.help_overlay_page, 0);
    }

    #[test]
    fn opening_help_closes_radial_menu() {
        let mut state = make_state();
        state.open_radial_menu(320.0, 240.0);
        assert!(state.is_radial_menu_open());

        state.toggle_help_overlay();

        assert!(state.show_help);
        assert!(!state.is_radial_menu_open());
    }

    #[test]
    fn toggle_quick_help_closes_when_already_in_quick_mode() {
        let mut state = make_state();
        state.toggle_quick_help();
        assert!(state.show_help);
        assert!(state.help_overlay_quick_mode);

        state.toggle_quick_help();
        assert!(!state.show_help);
        assert!(!state.help_overlay_quick_mode);
    }

    #[test]
    fn help_overlay_page_navigation_resets_scroll_and_respects_bounds() {
        let mut state = make_state();
        state.help_overlay_scroll = 123.0;
        assert!(state.help_overlay_next_page());
        assert_eq!(state.help_overlay_page, 1);
        assert_eq!(state.help_overlay_scroll, 0.0);

        state.help_overlay_page = HELP_OVERLAY_MAX_PAGES - 1;
        assert!(!state.help_overlay_next_page());
        assert!(state.help_overlay_prev_page());
        assert_eq!(state.help_overlay_page, HELP_OVERLAY_MAX_PAGES - 2);
    }

    #[test]
    fn help_search_insert_and_cursor_movement_handle_unicode_scalars() {
        let mut state = make_state();
        state.help_search_insert("a🙂");
        assert_eq!(state.help_overlay_search, "a🙂");
        assert_eq!(state.help_overlay_search_cursor, 2);

        state.help_search_cursor_left();
        assert_eq!(state.help_overlay_search_cursor, 1);
        state.help_search_cursor_right();
        assert_eq!(state.help_overlay_search_cursor, 2);
    }

    #[test]
    fn help_search_backspace_removes_previous_unicode_character() {
        let mut state = make_state();
        state.help_overlay_search = "a🙂b".to_string();
        state.help_overlay_search_cursor = 2;

        state.help_search_backspace();

        assert_eq!(state.help_overlay_search, "ab");
        assert_eq!(state.help_overlay_search_cursor, 1);
    }

    #[test]
    fn help_overlay_cursor_hint_maps_real_layout_regions() {
        let mut state = make_state();
        // A closed overlay never reports a hint, whatever the hit map holds.
        assert_eq!(state.help_overlay_cursor_hint_at(150, 215), None);

        state.toggle_help_overlay();
        crate::ui::install_help_hit_map_for_test(
            (100.0, 100.0, 200.0, 300.0),
            Some((110.0, 130.0, 180.0, 24.0)),
            &[(120.0, 200.0, 160.0, 30.0, crate::config::Action::ToggleHelp)],
        );

        assert_eq!(
            state.help_overlay_cursor_hint_at(150, 215),
            Some(HelpOverlayCursorHint::Pointer)
        );
        assert_eq!(
            state.help_overlay_cursor_hint_at(150, 140),
            Some(HelpOverlayCursorHint::Text)
        );
        assert_eq!(
            state.help_overlay_cursor_hint_at(150, 280),
            Some(HelpOverlayCursorHint::Default)
        );
        assert_eq!(state.help_overlay_cursor_hint_at(10, 10), None);

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn help_overlay_click_runs_rows_and_dismisses_outside() {
        let mut state = make_state();
        state.toggle_help_overlay();
        crate::ui::install_help_hit_map_for_test(
            (100.0, 100.0, 200.0, 300.0),
            Some((110.0, 130.0, 180.0, 24.0)),
            &[(
                120.0,
                200.0,
                160.0,
                30.0,
                crate::config::Action::ToggleStatusBar,
            )],
        );

        assert_eq!(
            state.help_overlay_click_at(150, 215),
            HelpOverlayClick::Run(crate::config::Action::ToggleStatusBar)
        );
        assert_eq!(
            state.help_overlay_click_at(150, 140),
            HelpOverlayClick::Inside
        );
        assert_eq!(
            state.help_overlay_click_at(150, 280),
            HelpOverlayClick::Inside
        );
        assert_eq!(
            state.help_overlay_click_at(10, 10),
            HelpOverlayClick::Outside
        );

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn close_help_overlay_resets_state_and_clears_hit_map() {
        let mut state = make_state();
        state.toggle_help_overlay();
        state.help_overlay_scroll = 42.0;
        crate::ui::install_help_hit_map_for_test(
            (100.0, 100.0, 200.0, 300.0),
            None,
            &[(120.0, 200.0, 160.0, 30.0, crate::config::Action::ToggleHelp)],
        );

        state.close_help_overlay();

        assert!(!state.show_help);
        assert_eq!(state.help_overlay_scroll, 0.0);
        // Closing dropped the stale hit map, so a later click resolves outside.
        assert_eq!(crate::ui::help_overlay_region_at(150.0, 215.0), None);
        assert_eq!(
            state.help_overlay_click_at(150, 215),
            HelpOverlayClick::Outside
        );
    }

    /// Install a hit map with a single clickable row at (120..280, 200..230)
    /// inside the box (100..300, 100..400) and a search well at (110..290,
    /// 130..154). The overlay is opened first so the install survives the
    /// open-time defensive clear, mirroring a real render pass populating the
    /// map while help is visible.
    fn state_with_help_row(action: crate::config::Action) -> InputState {
        let mut state = make_state();
        state.toggle_help_overlay();
        crate::ui::install_help_hit_map_for_test(
            (100.0, 100.0, 200.0, 300.0),
            Some((110.0, 130.0, 180.0, 24.0)),
            &[(120.0, 200.0, 160.0, 30.0, action)],
        );
        state
    }

    #[test]
    fn help_release_runs_row_only_when_press_and_release_share_the_row() {
        let mut state = state_with_help_row(crate::config::Action::ClearCanvas);

        // Press and release both on the row -> the row's action runs.
        state.note_help_overlay_press(HelpOverlayPressSource::Pointer(1), 150, 215);
        assert_eq!(
            state.resolve_help_overlay_release(HelpOverlayPressSource::Pointer(1), 150, 215),
            Some(HelpOverlayReleaseOutcome::Run(
                crate::config::Action::ClearCanvas
            ))
        );
        // The recorded press was consumed.
        assert!(state.help_overlay_pending_presses.is_empty());

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn help_press_on_chrome_then_drag_onto_row_does_not_run() {
        // The destructive-action hazard: a press that starts on bare chrome and
        // is dragged onto a clickable row (here the ClearCanvas row) must never
        // execute it.
        let mut state = state_with_help_row(crate::config::Action::ClearCanvas);

        // (150, 280) is inside the box but below the row and search well: chrome.
        state.note_help_overlay_press(HelpOverlayPressSource::Pointer(1), 150, 280);
        assert_eq!(
            state.help_overlay_pending_presses,
            vec![(HelpOverlayPressSource::Pointer(1), HelpOverlayClick::Inside)]
        );
        assert_eq!(
            state.resolve_help_overlay_release(HelpOverlayPressSource::Pointer(1), 150, 215),
            Some(HelpOverlayReleaseOutcome::None)
        );

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn help_press_outside_then_release_on_row_does_not_run() {
        let mut state = state_with_help_row(crate::config::Action::ClearCanvas);

        state.note_help_overlay_press(HelpOverlayPressSource::Pointer(1), 10, 10);
        assert_eq!(
            state.help_overlay_pending_presses,
            vec![(
                HelpOverlayPressSource::Pointer(1),
                HelpOverlayClick::Outside
            )]
        );
        assert_eq!(
            state.resolve_help_overlay_release(HelpOverlayPressSource::Pointer(1), 150, 215),
            Some(HelpOverlayReleaseOutcome::None)
        );

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn help_press_and_release_outside_dismisses() {
        let mut state = state_with_help_row(crate::config::Action::ClearCanvas);

        state.note_help_overlay_press(HelpOverlayPressSource::Pointer(1), 10, 10);
        assert_eq!(
            state.resolve_help_overlay_release(HelpOverlayPressSource::Pointer(1), 20, 20),
            Some(HelpOverlayReleaseOutcome::Dismiss)
        );

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn help_press_on_row_then_release_on_chrome_does_not_run() {
        let mut state = state_with_help_row(crate::config::Action::ClearCanvas);

        state.note_help_overlay_press(HelpOverlayPressSource::Pointer(1), 150, 215);
        assert_eq!(
            state.resolve_help_overlay_release(HelpOverlayPressSource::Pointer(1), 150, 280),
            Some(HelpOverlayReleaseOutcome::None)
        );

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn help_release_without_a_recorded_press_is_inert() {
        let mut state = state_with_help_row(crate::config::Action::ClearCanvas);

        // No note_help_overlay_press call: a release cannot fabricate intent.
        assert!(state.help_overlay_pending_presses.is_empty());
        assert_eq!(
            state.resolve_help_overlay_release(HelpOverlayPressSource::Touch, 150, 215),
            None
        );

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn help_release_requires_the_modality_that_owned_the_press() {
        let mut state = state_with_help_row(crate::config::Action::ClearCanvas);
        state.note_help_overlay_press(HelpOverlayPressSource::Touch, 150, 215);

        assert_eq!(
            state.resolve_help_overlay_release(HelpOverlayPressSource::Pointer(1), 150, 215),
            None,
            "a pointer release must fall through when touch owns the help press"
        );
        assert!(
            !state.help_overlay_pending_presses.is_empty(),
            "another modality must not consume the touch press"
        );
        assert_eq!(
            state.resolve_help_overlay_release(HelpOverlayPressSource::Touch, 150, 215),
            Some(HelpOverlayReleaseOutcome::Run(
                crate::config::Action::ClearCanvas
            ))
        );

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn help_pointer_ownership_is_tracked_per_button() {
        let mut state = state_with_help_row(crate::config::Action::ClearCanvas);
        let left = HelpOverlayPressSource::Pointer(1);
        let middle = HelpOverlayPressSource::Pointer(2);
        state.note_help_overlay_press(left, 150, 215);
        state.note_help_overlay_press(middle, 150, 215);

        assert!(
            state.clear_help_overlay_press_for(middle),
            "a middle press made while help is open owns its release"
        );
        assert!(
            !state.clear_help_overlay_press_for(middle),
            "a middle release whose press preceded help must fall through"
        );
        assert_eq!(
            state.resolve_help_overlay_release(left, 150, 215),
            Some(HelpOverlayReleaseOutcome::Run(
                crate::config::Action::ClearCanvas
            )),
            "middle ownership must not consume the left help click"
        );

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn closing_help_keeps_its_press_release_owned_without_running_the_stale_target() {
        let mut state = state_with_help_row(crate::config::Action::ClearCanvas);
        let pointer = HelpOverlayPressSource::Pointer(1);
        state.note_help_overlay_press(pointer, 150, 215);

        state.close_help_overlay();

        assert!(state.help_overlay_pending_presses.is_empty());
        assert_eq!(state.help_overlay_consume_only_presses, vec![pointer]);

        assert_eq!(
            state.resolve_help_overlay_release(pointer, 150, 215),
            Some(HelpOverlayReleaseOutcome::None),
            "a press swallowed by help must still consume its physical release after help closes"
        );
        assert!(state.help_overlay_pending_presses.is_empty());
        assert!(state.help_overlay_consume_only_presses.is_empty());

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn reopening_help_cannot_retarget_a_press_from_the_previous_layout() {
        let mut state = state_with_help_row(crate::config::Action::ClearCanvas);
        let pointer = HelpOverlayPressSource::Pointer(1);
        state.note_help_overlay_press(pointer, 150, 215);

        state.close_help_overlay();
        state.toggle_help_overlay();
        crate::ui::install_help_hit_map_for_test(
            (100.0, 100.0, 200.0, 300.0),
            None,
            &[(
                120.0,
                200.0,
                160.0,
                30.0,
                crate::config::Action::ClearCanvas,
            )],
        );

        assert_eq!(
            state.resolve_help_overlay_release(pointer, 150, 215),
            Some(HelpOverlayReleaseOutcome::None),
            "an old press may only be consumed, never resolved against a reopened layout"
        );

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn a_new_help_press_supersedes_consume_only_ownership_for_its_source() {
        let mut state = state_with_help_row(crate::config::Action::ClearCanvas);
        let pointer = HelpOverlayPressSource::Pointer(1);
        state.note_help_overlay_press(pointer, 150, 215);
        state.close_help_overlay();
        state.toggle_help_overlay();
        crate::ui::install_help_hit_map_for_test(
            (100.0, 100.0, 200.0, 300.0),
            None,
            &[(
                120.0,
                200.0,
                160.0,
                30.0,
                crate::config::Action::ClearCanvas,
            )],
        );

        state.note_help_overlay_press(pointer, 150, 215);

        assert_eq!(
            state.resolve_help_overlay_release(pointer, 150, 215),
            Some(HelpOverlayReleaseOutcome::Run(
                crate::config::Action::ClearCanvas
            ))
        );

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn opening_help_drops_stale_hit_map_geometry() {
        let mut state = make_state();
        // Simulate geometry left over from a previous open.
        crate::ui::install_help_hit_map_for_test(
            (100.0, 100.0, 200.0, 300.0),
            None,
            &[(120.0, 200.0, 160.0, 30.0, crate::config::Action::ToggleHelp)],
        );

        // Opening must drop it so a click can never act on the previous layout
        // before the first fresh render repopulates the map.
        state.toggle_help_overlay();

        assert!(state.show_help);
        assert_eq!(crate::ui::help_overlay_region_at(150.0, 215.0), None);
        assert!(state.help_overlay_pending_presses.is_empty());

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn starting_the_tour_routes_help_close_through_the_canonical_closer() {
        let mut state = make_state();
        state.toggle_help_overlay();
        crate::ui::install_help_hit_map_for_test(
            (100.0, 100.0, 200.0, 300.0),
            None,
            &[(120.0, 200.0, 160.0, 30.0, crate::config::Action::ToggleHelp)],
        );

        state.start_tour();

        assert!(!state.show_help);
        // Routing through close_help_overlay dropped the cached hit map, so a
        // click after help reopens can never act on this stale layout.
        assert_eq!(crate::ui::help_overlay_region_at(150.0, 215.0), None);

        crate::ui::clear_help_overlay_hit_map();
    }

    #[test]
    fn opening_the_command_palette_routes_help_close_through_the_canonical_closer() {
        let mut state = make_state();
        state.toggle_help_overlay();
        crate::ui::install_help_hit_map_for_test(
            (100.0, 100.0, 200.0, 300.0),
            None,
            &[(120.0, 200.0, 160.0, 30.0, crate::config::Action::ToggleHelp)],
        );

        state.toggle_command_palette();

        assert!(!state.show_help);
        assert!(state.command_palette_open);
        assert_eq!(crate::ui::help_overlay_region_at(150.0, 215.0), None);

        crate::ui::clear_help_overlay_hit_map();
    }
}
