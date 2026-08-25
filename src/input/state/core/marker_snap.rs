//! The marker's snap-to-text mode.
//!
//! A highlighter drawn by hand over a line of code or prose always comes out
//! crooked, and on a recording that is what the audience remembers. Snap mode
//! locks the stroke to a detected text row: the pointer picks the row, the drag
//! sets how much of it is covered, and the stroke itself is a straight segment
//! at the row's vertical center.
//!
//! The rows come from a Tesseract layout scan of the displayed screen image,
//! run by the backend once per capture (`src/backend/wayland/state/marker_snap.rs`).
//! This module owns only what the input layer needs: the mode flag, the rows in
//! logical screen coordinates, the lock that a drag takes on one row, and the
//! canvas-space geometry a stroke is built from.
//!
//! Every path degrades to freehand. No screen image, no scan yet, no engine, no
//! text under the pointer — all of them mean the marker behaves exactly as it
//! did before this module existed.

use super::InputState;
use crate::input::text_snap::{SnappedTextRow, TextSnapMap};
use crate::input::tool::Tool;

/// What snap mode is currently able to do, for the status line and the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerSnapState {
    /// The mode is off; the marker draws freehand.
    Off,
    /// On, but nothing has been scanned yet because there is no screen image.
    AwaitingScreen,
    /// A layout scan is running. The marker draws freehand until it lands.
    Scanning,
    /// Rows are available and the marker snaps to them.
    Ready,
    /// The scan completed and found no text to snap to.
    NoText,
    /// Snapping cannot run here, with a stable reason for the status line.
    Unavailable(MarkerSnapBlocker),
}

/// Why snapping is unavailable. Each maps to one user-facing sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerSnapBlocker {
    /// Tesseract is not installed.
    EngineMissing,
    /// The board is not transparent, so there is no screen text to snap to.
    OpaqueBoard,
    /// Screen capture is disabled, so no image can be scanned.
    CaptureDisabled,
    /// The scan failed for an engine reason already reported in the log.
    ScanFailed,
}

impl MarkerSnapState {
    /// Whether strokes actually snap right now.
    pub fn snaps(self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Whether the mode is on, whatever it is currently able to do.
    pub fn enabled(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// Short status text. Deliberately says what the user can do about it.
    pub fn status_text(self) -> Option<&'static str> {
        match self {
            Self::Off => None,
            Self::AwaitingScreen => Some("Snap: freeze the screen to find text"),
            Self::Scanning => Some("Snap: reading screen text..."),
            Self::Ready => Some("Snap: on"),
            Self::NoText => Some("Snap: no text found"),
            Self::Unavailable(MarkerSnapBlocker::EngineMissing) => {
                Some("Snap needs Tesseract installed")
            }
            Self::Unavailable(MarkerSnapBlocker::OpaqueBoard) => {
                Some("Snap needs a transparent board")
            }
            Self::Unavailable(MarkerSnapBlocker::CaptureDisabled) => {
                Some("Snap needs screen capture enabled")
            }
            Self::Unavailable(MarkerSnapBlocker::ScanFailed) => Some("Snap: screen scan failed"),
        }
    }
}

impl InputState {
    /// Whether the marker is set to snap to text.
    pub fn marker_snap_to_text(&self) -> bool {
        self.marker_snap_state.enabled()
    }

    pub fn marker_snap_state(&self) -> MarkerSnapState {
        self.marker_snap_state
    }

    /// Turn the mode on or off. Returns whether anything changed.
    ///
    /// Turning it on asks the backend for a scan rather than assuming one; the
    /// rows from a previous capture are dropped, because they describe a screen
    /// that is no longer the one being drawn on.
    pub fn set_marker_snap_to_text(&mut self, enabled: bool) -> bool {
        if self.marker_snap_state.enabled() == enabled {
            return false;
        }
        if enabled {
            self.marker_snap_state = MarkerSnapState::AwaitingScreen;
            self.request_marker_snap_scan();
        } else {
            self.marker_snap_state = MarkerSnapState::Off;
            self.marker_text_snap = TextSnapMap::default();
            self.pending_marker_snap_scan = false;
            self.active_marker_snap_row = None;
        }
        self.last_marker_snap_preview_bounds = None;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    /// Flip the mode. Returns the new value.
    pub fn toggle_marker_snap_to_text(&mut self) -> bool {
        let next = !self.marker_snap_state.enabled();
        self.set_marker_snap_to_text(next);
        next
    }

    /// Flip the mode from the explicit action, arming the marker with it.
    ///
    /// Turning a marker mode on while holding the pen used to confirm "Marker
    /// snaps to text" and then do nothing observable: the scan, the status
    /// chip, and the preview are all gated on the marker being in hand, so the
    /// toast was the only thing that happened. Asking for the mode is asking
    /// for the tool, the way picking the highlight tool also switches click
    /// highlights on.
    pub(crate) fn announce_marker_snap_toggle_with_tool(&mut self) {
        if !self.marker_snap_state.enabled() {
            self.set_tool_override(Some(Tool::Marker));
        }
        self.announce_marker_snap_toggle();
    }

    /// Flip the mode and say what happened, without touching the active tool.
    ///
    /// Turning it on cannot promise snapping — there may be no screen image, no
    /// engine, or no text — so the toast says what the mode is now and lets the
    /// status line carry what it is able to do.
    pub(crate) fn announce_marker_snap_toggle(&mut self) {
        let enabled = self.toggle_marker_snap_to_text();
        let message = if enabled {
            "Marker snaps to text"
        } else {
            "Marker draws freehand"
        };
        log::info!("Marker snap to text: {enabled}");
        self.push_toast(
            super::ToastPriority::Info,
            "marker.snap",
            super::Toast::info(message),
        );
    }

    /// Report a state the backend resolved (scanning, blocked, finished).
    ///
    /// Ignored while the mode is off, so a completion that arrives after the
    /// user turned snapping off cannot switch it back on.
    pub(crate) fn set_marker_snap_state(&mut self, state: MarkerSnapState) {
        if !self.marker_snap_state.enabled() || self.marker_snap_state == state {
            return;
        }
        self.marker_snap_state = state;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Ask the backend to scan the displayed screen image.
    pub(crate) fn request_marker_snap_scan(&mut self) {
        if !self.marker_snap_state.enabled() {
            return;
        }
        self.pending_marker_snap_scan = true;
    }

    /// Take the scan request, if any. The backend calls this once per batch.
    pub fn take_pending_marker_snap_scan(&mut self) -> bool {
        std::mem::take(&mut self.pending_marker_snap_scan)
    }

    /// Install the rows a finished scan produced, in logical screen coordinates.
    pub(crate) fn install_marker_text_snap(&mut self, map: TextSnapMap) {
        if !self.marker_snap_state.enabled() {
            return;
        }
        let empty = map.is_empty();
        self.marker_text_snap = map;
        self.marker_snap_state = if empty {
            MarkerSnapState::NoText
        } else {
            MarkerSnapState::Ready
        };
        self.last_marker_snap_preview_bounds = None;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Drop the rows because the screen image they describe is gone.
    ///
    /// A locked drag keeps its row: the stroke the user is in the middle of
    /// drawing must not jump or straighten differently halfway through.
    pub(crate) fn invalidate_marker_text_snap(&mut self) {
        if self.marker_text_snap.is_empty() && !self.marker_snap_state.snaps() {
            return;
        }
        self.marker_text_snap = TextSnapMap::default();
        if self.marker_snap_state.enabled() {
            self.marker_snap_state = MarkerSnapState::AwaitingScreen;
        }
        self.last_marker_snap_preview_bounds = None;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Whether a marker stroke started now would snap.
    pub(crate) fn marker_snap_armed(&self) -> bool {
        self.marker_snap_state.snaps() && !self.marker_text_snap.is_empty()
    }

    /// Whether the marker is the tool in hand and snapping is armed.
    pub fn marker_snap_preview_active(&self) -> bool {
        self.marker_snap_armed() && self.active_tool() == Tool::Marker
    }

    /// The row under a canvas point, in logical screen coordinates.
    pub fn marker_snap_row_at_canvas(
        &self,
        canvas_x: i32,
        canvas_y: i32,
    ) -> Option<SnappedTextRow> {
        if !self.marker_snap_armed() {
            return None;
        }
        let (screen_x, screen_y) = self.screen_coords_for_canvas(canvas_x, canvas_y);
        self.marker_text_snap
            .row_at((f64::from(screen_x), f64::from(screen_y)))
    }

    /// The row the active drag locked onto at press time, if it took one.
    pub fn active_marker_snap_row(&self) -> Option<SnappedTextRow> {
        self.active_marker_snap_row
    }

    /// Lock a row for a drag that is starting, if the press landed on one.
    ///
    /// Called for every marker press; a press away from text simply locks
    /// nothing and the stroke stays freehand for its whole life, which is what
    /// keeps a fallback stroke from straightening mid-drag.
    pub(crate) fn begin_marker_snap_drag(&mut self, tool: Tool, canvas_x: i32, canvas_y: i32) {
        self.active_marker_snap_row = (tool == Tool::Marker)
            .then(|| self.marker_snap_row_at_canvas(canvas_x, canvas_y))
            .flatten();
    }

    /// Release the lock at the end of a drag, however it ended.
    pub(crate) fn end_marker_snap_drag(&mut self) -> Option<SnappedTextRow> {
        self.active_marker_snap_row.take()
    }

    /// The hover preview for a pointer at a canvas point, in canvas coordinates.
    ///
    /// `None` whenever the marker is not snapping or the pointer is not over a
    /// row, which is also what tells the caller to draw nothing.
    pub fn marker_snap_hover_preview(
        &self,
        canvas_x: i32,
        canvas_y: i32,
    ) -> Option<crate::ui::MarkerSnapPreview> {
        if !self.marker_snap_preview_active() {
            return None;
        }
        let row = self.marker_snap_row_at_canvas(canvas_x, canvas_y)?;
        let center_y = row.center_y.round() as i32;
        let left = self.canvas_coords_for_screen(row.left.round() as i32, center_y);
        let right = self.canvas_coords_for_screen(row.right.round() as i32, center_y);
        let scale = self.current_canvas_scale();
        let thickness = if scale.is_finite() && scale > 0.0 {
            row.thickness() / scale
        } else {
            row.thickness()
        };
        Some(crate::ui::MarkerSnapPreview {
            left: f64::from(left.0),
            right: f64::from(right.0),
            center_y: f64::from(left.1),
            thickness: thickness.max(1.0),
            pointer_x: f64::from(canvas_x),
        })
    }

    /// Damage the hover preview for a pointer that has moved, if it changed.
    ///
    /// Returns whether a repaint is needed. The preview is chrome the dirty
    /// tracker knows nothing about, so both the rectangle it is leaving and the
    /// one it is arriving at have to be named here; a repaint driven only by
    /// the new one would smear the I-beam across every row it passed.
    pub(crate) fn update_marker_snap_hover_damage(&mut self, canvas_x: i32, canvas_y: i32) -> bool {
        let bounds = self
            .marker_snap_hover_preview(canvas_x, canvas_y)
            .and_then(crate::ui::marker_snap_preview_bounds);
        if bounds == self.last_marker_snap_preview_bounds {
            return false;
        }
        for rect in [self.last_marker_snap_preview_bounds, bounds]
            .into_iter()
            .flatten()
        {
            self.dirty_tracker.mark_rect(rect);
        }
        self.last_marker_snap_preview_bounds = bounds;
        true
    }

    /// The canvas-space geometry of a snapped stroke between two canvas x's.
    ///
    /// Returns the two endpoints and the stroke thickness. Both endpoints share
    /// the row's vertical center, which is what makes the stroke straight
    /// regardless of how the pointer wandered.
    pub(crate) fn marker_snap_stroke_canvas(
        &self,
        row: SnappedTextRow,
        start: (i32, i32),
        current: (i32, i32),
    ) -> Option<(Vec<(i32, i32)>, f64)> {
        let start_screen_x = f64::from(self.screen_coords_for_canvas(start.0, start.1).0);
        let current_screen_x = f64::from(self.screen_coords_for_canvas(current.0, current.1).0);
        let (left, right) = row.span(start_screen_x, current_screen_x);
        if !left.is_finite() || !right.is_finite() {
            return None;
        }

        let center_y = row.center_y.round() as i32;
        let first = self.canvas_coords_for_screen(left.round() as i32, center_y);
        let second = self.canvas_coords_for_screen(right.round() as i32, center_y);

        let scale = self.current_canvas_scale();
        let thickness = if scale.is_finite() && scale > 0.0 {
            row.thickness() / scale
        } else {
            row.thickness()
        };
        Some((vec![first, second], thickness.max(1.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::text_snap::TextSnapLine;

    fn line(left: f64, top: f64, right: f64, bottom: f64) -> TextSnapLine {
        TextSnapLine {
            left,
            top,
            right,
            bottom,
        }
    }

    fn ready_state() -> InputState {
        let mut state = make_test_input_state();
        state.set_marker_snap_to_text(true);
        state.install_marker_text_snap(TextSnapMap::new([line(100.0, 100.0, 400.0, 120.0)]));
        state
    }

    #[test]
    fn turning_the_mode_on_asks_for_a_scan_and_turning_it_off_forgets_the_rows() {
        let mut state = make_test_input_state();

        assert!(state.set_marker_snap_to_text(true));
        assert!(state.marker_snap_to_text());
        assert!(state.take_pending_marker_snap_scan());
        assert!(!state.take_pending_marker_snap_scan(), "consumed once");

        state.install_marker_text_snap(TextSnapMap::new([line(0.0, 0.0, 50.0, 20.0)]));
        assert_eq!(state.marker_snap_state(), MarkerSnapState::Ready);

        assert!(state.set_marker_snap_to_text(false));
        assert_eq!(state.marker_snap_state(), MarkerSnapState::Off);
        assert!(!state.marker_snap_armed());
    }

    #[test]
    fn setting_the_same_mode_twice_changes_nothing() {
        let mut state = make_test_input_state();

        assert!(state.set_marker_snap_to_text(true));
        assert!(!state.set_marker_snap_to_text(true));
    }

    #[test]
    fn a_scan_that_found_nothing_is_reported_as_such_rather_than_as_ready() {
        let mut state = make_test_input_state();
        state.set_marker_snap_to_text(true);

        state.install_marker_text_snap(TextSnapMap::default());

        assert_eq!(state.marker_snap_state(), MarkerSnapState::NoText);
        assert!(!state.marker_snap_armed());
    }

    #[test]
    fn a_completion_arriving_after_the_mode_was_turned_off_cannot_turn_it_back_on() {
        let mut state = make_test_input_state();
        state.set_marker_snap_to_text(true);
        state.set_marker_snap_to_text(false);

        state.install_marker_text_snap(TextSnapMap::new([line(0.0, 0.0, 50.0, 20.0)]));
        state.set_marker_snap_state(MarkerSnapState::Ready);

        assert_eq!(state.marker_snap_state(), MarkerSnapState::Off);
        assert!(!state.marker_snap_armed());
    }

    #[test]
    fn losing_the_screen_image_drops_the_rows_and_waits_for_a_new_scan() {
        let mut state = ready_state();

        state.invalidate_marker_text_snap();

        assert_eq!(state.marker_snap_state(), MarkerSnapState::AwaitingScreen);
        assert!(!state.marker_snap_armed());
    }

    #[test]
    fn a_press_on_text_locks_a_row_and_a_press_away_from_text_locks_nothing() {
        let mut state = ready_state();

        state.begin_marker_snap_drag(Tool::Marker, 200, 110);
        assert!(state.active_marker_snap_row().is_some());
        assert!(state.end_marker_snap_drag().is_some());
        assert!(state.active_marker_snap_row().is_none());

        state.begin_marker_snap_drag(Tool::Marker, 200, 900);
        assert!(
            state.active_marker_snap_row().is_none(),
            "a press in empty space must draw freehand for the whole stroke"
        );
    }

    #[test]
    fn only_the_marker_takes_a_snap_lock() {
        let mut state = ready_state();

        state.begin_marker_snap_drag(Tool::Pen, 200, 110);

        assert!(state.active_marker_snap_row().is_none());
    }

    #[test]
    fn a_snapped_stroke_is_two_points_on_one_row_however_the_pointer_wandered() {
        let state = ready_state();
        let row = state.marker_snap_row_at_canvas(200, 110).expect("row");

        let (points, thickness) = state
            .marker_snap_stroke_canvas(row, (150, 105), (300, 480))
            .expect("snapped geometry");

        assert_eq!(points.len(), 2);
        assert_eq!(
            points[0].1, points[1].1,
            "both endpoints sit on the row's center"
        );
        assert!(points[0].0 < points[1].0);
        assert!(thickness > 20.0, "thickness covers the 20px row");
    }

    #[test]
    fn a_backwards_drag_produces_the_same_stroke_as_a_forwards_one() {
        let state = ready_state();
        let row = state.marker_snap_row_at_canvas(200, 110).expect("row");

        let forward = state.marker_snap_stroke_canvas(row, (150, 110), (300, 110));
        let backward = state.marker_snap_stroke_canvas(row, (300, 110), (150, 110));

        assert_eq!(forward, backward);
    }

    #[test]
    fn a_drag_past_the_end_of_the_text_stops_at_the_row() {
        let state = ready_state();
        let row = state.marker_snap_row_at_canvas(200, 110).expect("row");

        let (points, _) = state
            .marker_snap_stroke_canvas(row, (150, 110), (9000, 110))
            .expect("snapped geometry");

        assert!(
            points[1].0 < 500,
            "the highlight ends at the text, not where the pointer went"
        );
    }

    #[test]
    fn no_rows_means_no_snapping_and_no_preview() {
        let mut state = make_test_input_state();
        state.set_marker_snap_to_text(true);

        assert!(!state.marker_snap_armed());
        assert!(!state.marker_snap_preview_active());
        assert!(state.marker_snap_row_at_canvas(200, 110).is_none());
    }

    #[test]
    fn moving_between_rows_damages_the_row_left_as_well_as_the_one_entered() {
        let mut state = make_test_input_state();
        state.set_marker_snap_to_text(true);
        state.set_tool_override(Some(Tool::Marker));
        state.install_marker_text_snap(TextSnapMap::new([
            line(100.0, 100.0, 400.0, 120.0),
            line(100.0, 200.0, 400.0, 220.0),
        ]));
        let _ = state.take_dirty_regions();

        assert!(state.update_marker_snap_hover_damage(200, 110));
        let first = state.take_dirty_regions();
        assert_eq!(first.len(), 1, "arriving on a row damages just that row");

        assert!(state.update_marker_snap_hover_damage(200, 210));
        let second = state.take_dirty_regions();
        assert_eq!(
            second.len(),
            2,
            "moving to another row must also repaint the one the I-beam left"
        );
        assert!(second[0].y < second[1].y || second[1].y < second[0].y);
    }

    #[test]
    fn a_pointer_that_has_not_left_its_row_asks_for_no_repaint() {
        let mut state = make_test_input_state();
        state.set_marker_snap_to_text(true);
        state.set_tool_override(Some(Tool::Marker));
        state.install_marker_text_snap(TextSnapMap::new([line(100.0, 100.0, 400.0, 120.0)]));

        assert!(state.update_marker_snap_hover_damage(200, 110));
        assert!(
            !state.update_marker_snap_hover_damage(200, 112),
            "the preview spans the whole row, so sliding along it changes nothing"
        );
    }

    #[test]
    fn leaving_every_row_repaints_the_last_one_and_then_stays_quiet() {
        let mut state = make_test_input_state();
        state.set_marker_snap_to_text(true);
        state.set_tool_override(Some(Tool::Marker));
        state.install_marker_text_snap(TextSnapMap::new([line(100.0, 100.0, 400.0, 120.0)]));
        state.update_marker_snap_hover_damage(200, 110);
        let _ = state.take_dirty_regions();

        assert!(state.update_marker_snap_hover_damage(200, 900));
        assert_eq!(state.take_dirty_regions().len(), 1);
        assert!(!state.update_marker_snap_hover_damage(200, 950));
    }

    #[test]
    fn a_tool_that_is_not_the_marker_shows_no_preview() {
        let mut state = make_test_input_state();
        state.set_marker_snap_to_text(true);
        state.set_tool_override(Some(Tool::Pen));
        state.install_marker_text_snap(TextSnapMap::new([line(100.0, 100.0, 400.0, 120.0)]));

        assert!(!state.marker_snap_preview_active());
        assert!(state.marker_snap_hover_preview(200, 110).is_none());
        assert!(!state.update_marker_snap_hover_damage(200, 110));
    }

    #[test]
    fn the_explicit_toggle_arms_the_marker_so_the_mode_has_something_to_act_on() {
        let mut state = make_test_input_state();
        state.set_tool_override(Some(Tool::Pen));

        state.announce_marker_snap_toggle_with_tool();

        assert!(state.marker_snap_to_text());
        assert_eq!(
            state.active_tool(),
            Tool::Marker,
            "confirming a marker mode while holding the pen must not be a no-op"
        );
    }

    #[test]
    fn turning_the_mode_off_leaves_the_active_tool_alone() {
        let mut state = make_test_input_state();
        state.announce_marker_snap_toggle_with_tool();
        state.set_tool_override(Some(Tool::Pen));

        state.announce_marker_snap_toggle_with_tool();

        assert!(!state.marker_snap_to_text());
        assert_eq!(state.active_tool(), Tool::Pen);
    }

    #[test]
    fn the_tool_key_path_never_changes_the_tool_it_was_pressed_on() {
        let mut state = make_test_input_state();
        state.set_tool_override(Some(Tool::Marker));

        state.announce_marker_snap_toggle();

        assert!(state.marker_snap_to_text());
        assert_eq!(state.active_tool(), Tool::Marker);
    }

    #[test]
    fn every_state_the_user_can_reach_names_itself_except_off() {
        assert!(MarkerSnapState::Off.status_text().is_none());
        for state in [
            MarkerSnapState::AwaitingScreen,
            MarkerSnapState::Scanning,
            MarkerSnapState::Ready,
            MarkerSnapState::NoText,
            MarkerSnapState::Unavailable(MarkerSnapBlocker::EngineMissing),
            MarkerSnapState::Unavailable(MarkerSnapBlocker::OpaqueBoard),
            MarkerSnapState::Unavailable(MarkerSnapBlocker::CaptureDisabled),
            MarkerSnapState::Unavailable(MarkerSnapBlocker::ScanFailed),
        ] {
            assert!(state.status_text().is_some(), "{state:?} needs wording");
        }
    }
}
