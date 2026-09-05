use super::super::base::InputState;

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
    /// Install the geometry returned by the public help result renderer for
    /// subsequent click and cursor queries on this input owner.
    pub fn install_help_overlay_render_result(
        &mut self,
        result: crate::help_overlay_interaction::HelpRenderResult,
    ) {
        self.help_overlay.install_render_result(result);
    }

    fn open_help_overlay_internal(&mut self, quick_mode: bool, track_usage: bool) {
        self.close_modals_for_open(crate::input::state::core::modal::ModalSurface::HelpOverlay);
        self.help_overlay.open(quick_mode);
        if track_usage {
            self.pending_onboarding_usage.used_help_overlay = true;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn toggle_help_overlay(&mut self) {
        if self.help_overlay.visible {
            self.close_help_overlay();
            return;
        }
        self.open_help_overlay_internal(false, true);
    }

    pub(crate) fn toggle_quick_help(&mut self) {
        if self.help_overlay.visible && self.help_overlay.quick_mode {
            self.close_help_overlay();
            return;
        }
        self.open_help_overlay_internal(true, true);
    }

    /// Close the help overlay and drop the stale pointer hit map so a later
    /// click can never act on the previous frame's rectangles.
    pub(crate) fn close_help_overlay(&mut self) {
        if !self.help_overlay.close() {
            return;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Resolve a left-click at `(x, y)` (screen space) against the real rendered
    /// help layout: a clickable row/footer action, inside chrome, or a dismiss.
    pub fn help_overlay_click_at(&self, x: i32, y: i32) -> HelpOverlayClick {
        match self.help_overlay.region_at(x as f64, y as f64) {
            Some(crate::help_overlay_interaction::HelpOverlayRegion::Row(action)) => {
                HelpOverlayClick::Run(action)
            }
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
        let target = self.help_overlay_click_at(x, y);
        self.help_overlay.note_press(source, target);
    }

    /// Clear a pending help press only when it belongs to `source`. Returns
    /// whether this modality owned the press and therefore owns and swallows its
    /// eventual release.
    pub(crate) fn clear_help_overlay_press_for(&mut self, source: HelpOverlayPressSource) -> bool {
        self.help_overlay.clear_press_for(source)
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
        let released = self.help_overlay_click_at(x, y);
        self.help_overlay.resolve_release(source, released)
    }

    /// Determine the cursor type for the help overlay.
    /// Returns `None` if the help overlay is not open, or the point is outside
    /// the overlay box.
    ///
    /// Resolved against the real rendered layout (the overlay's pointer hit
    /// map): the search well shows a text cursor, clickable rows and the
    /// "Replay tour" footer show a pointer, everything else the default.
    pub fn help_overlay_cursor_hint_at(&self, x: i32, y: i32) -> Option<HelpOverlayCursorHint> {
        if !self.help_overlay.visible {
            return None;
        }

        match self.help_overlay.region_at(x as f64, y as f64)? {
            crate::help_overlay_interaction::HelpOverlayRegion::Search => {
                Some(HelpOverlayCursorHint::Text)
            }
            crate::help_overlay_interaction::HelpOverlayRegion::Row(_) => {
                Some(HelpOverlayCursorHint::Pointer)
            }
            crate::help_overlay_interaction::HelpOverlayRegion::Inside => {
                Some(HelpOverlayCursorHint::Default)
            }
        }
    }
}
