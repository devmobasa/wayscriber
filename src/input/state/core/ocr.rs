use super::InputState;
use crate::input::state::{Toast, ToastPriority};

/// Which capture the OCR selector is waiting on before it can arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrCaptureSource {
    Frozen,
    Zoom,
}

/// Which physical input started a region drag.
///
/// A region is one gesture by one device. Wayscriber is modal but the seat is
/// not: a pen can hover or lift while the region is being dragged with the
/// mouse, and without an owner those stray events would move — or submit —
/// somebody else's region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrInputSource {
    Pointer,
    Touch,
    Stylus,
}

/// A finished or in-progress region drag in logical screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OcrSelection {
    pub start: (f64, f64),
    pub end: (f64, f64),
    /// OCR created the freeze it selected against, so OCR must release it.
    pub auto_froze: bool,
}

/// UI-facing lifecycle for the modal `Copy text from screen` region selector.
///
/// It owns transient input and render state only. The selected pixels and the
/// recognition work belong outside `InputState`, so cancelling here can never
/// discard a request that is already running.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum OcrUiState {
    #[default]
    Inactive,
    PendingCapture {
        source: OcrCaptureSource,
        auto_froze: bool,
    },
    /// The selector is live and waiting for the press that starts a region.
    Armed { auto_froze: bool },
    /// A region drag is in progress between `start` and `current`, owned by the
    /// device that pressed. Only that device may move, release, or cancel it.
    Selecting {
        owner: OcrInputSource,
        start: (f64, f64),
        current: (f64, f64),
        auto_froze: bool,
    },
}

impl OcrUiState {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Armed { .. } | Self::Selecting { .. })
    }

    pub fn is_pending(self) -> bool {
        matches!(self, Self::PendingCapture { .. })
    }

    pub fn is_engaged(self) -> bool {
        self.is_active() || self.is_pending()
    }

    pub fn is_selecting(self) -> bool {
        matches!(self, Self::Selecting { .. })
    }

    /// The region drag in logical screen coordinates, if one is in progress.
    pub fn selection(self) -> Option<OcrSelection> {
        match self {
            Self::Selecting {
                start,
                current,
                auto_froze,
                ..
            } => Some(OcrSelection {
                start,
                end: current,
                auto_froze,
            }),
            Self::Inactive | Self::PendingCapture { .. } | Self::Armed { .. } => None,
        }
    }

    /// The device dragging the region, if one is in progress.
    pub fn selection_owner(self) -> Option<OcrInputSource> {
        match self {
            Self::Selecting { owner, .. } => Some(owner),
            Self::Inactive | Self::PendingCapture { .. } | Self::Armed { .. } => None,
        }
    }

    pub fn pending_source(self) -> Option<OcrCaptureSource> {
        match self {
            Self::PendingCapture { source, .. } => Some(source),
            Self::Inactive | Self::Armed { .. } | Self::Selecting { .. } => None,
        }
    }

    fn auto_froze(self) -> bool {
        match self {
            Self::PendingCapture { auto_froze, .. }
            | Self::Armed { auto_froze }
            | Self::Selecting { auto_froze, .. } => auto_froze,
            Self::Inactive => false,
        }
    }
}

impl InputState {
    pub(crate) fn request_copy_text_from_screen(&mut self) {
        self.pending_ocr_request = true;
    }

    pub(crate) fn take_pending_ocr_request(&mut self) -> bool {
        std::mem::take(&mut self.pending_ocr_request)
    }

    pub fn ocr_state(&self) -> OcrUiState {
        self.ocr_ui_state
    }

    pub fn ocr_is_active(&self) -> bool {
        self.ocr_ui_state.is_active()
    }

    pub fn ocr_is_engaged(&self) -> bool {
        self.ocr_ui_state.is_engaged()
    }

    pub(crate) fn set_ocr_pending_capture(&mut self, source: OcrCaptureSource) {
        self.ocr_ui_state = OcrUiState::PendingCapture {
            source,
            auto_froze: matches!(source, OcrCaptureSource::Frozen),
        };
        self.mark_ocr_dirty();
    }

    pub(crate) fn activate_ocr(&mut self, auto_froze: bool) {
        // A capture can take long enough for another interaction to begin while
        // OCR is pending. Entering the modal state must cancel it so the
        // selector cannot swallow the matching release event.
        self.prepare_for_screen_modal();
        self.ocr_ui_state = OcrUiState::Armed { auto_froze };
        self.mark_ocr_dirty();
    }

    /// Begin a region drag owned by `owner`.
    ///
    /// Ignored unless the selector is armed, so a stray press during capture
    /// cannot start a region against a stale image — and, because a drag in
    /// progress is no longer armed, a second device cannot take one over.
    pub(crate) fn start_ocr_selection(&mut self, owner: OcrInputSource, point: (f64, f64)) -> bool {
        let OcrUiState::Armed { auto_froze } = self.ocr_ui_state else {
            return false;
        };
        self.ocr_ui_state = OcrUiState::Selecting {
            owner,
            start: point,
            current: point,
            auto_froze,
        };
        self.mark_ocr_dirty();
        true
    }

    /// Move the region, if `source` is the device dragging it. Motion from any
    /// other device is somebody else's and is dropped.
    pub(crate) fn update_ocr_selection(&mut self, source: OcrInputSource, point: (f64, f64)) {
        if let OcrUiState::Selecting { owner, current, .. } = &mut self.ocr_ui_state
            && *owner == source
            && *current != point
        {
            *current = point;
            self.mark_ocr_dirty();
        }
    }

    /// Whether `source` is the device dragging the region right now. The
    /// release and per-device cancellation paths check this before acting, so
    /// one device cannot submit or discard another's region.
    pub(crate) fn ocr_selection_is_owned_by(&self, source: OcrInputSource) -> bool {
        self.ocr_ui_state.selection_owner() == Some(source)
    }

    /// Abandon a drag that was too small to be a region and wait for the next
    /// one. A mis-click should not drop the user out of the selector, and it
    /// must not release a capture the selector is still using.
    pub(crate) fn rearm_ocr_selection(&mut self) {
        if let OcrUiState::Selecting { auto_froze, .. } = self.ocr_ui_state {
            self.ocr_ui_state = OcrUiState::Armed { auto_froze };
            self.mark_ocr_dirty();
        }
    }

    /// Leave OCR selection and report whether it created frozen mode itself.
    pub(crate) fn cancel_ocr(&mut self) -> bool {
        let auto_froze = self.ocr_ui_state.auto_froze();
        if !matches!(self.ocr_ui_state, OcrUiState::Inactive) {
            self.ocr_ui_state = OcrUiState::Inactive;
            self.mark_ocr_dirty();
        }
        auto_froze
    }

    pub(crate) fn report_ocr_capture_failure_if_unreported(&mut self) {
        if self.ui_toast.is_none() {
            self.push_toast(
                ToastPriority::Critical,
                "ocr",
                Toast::error("Screen capture for text recognition failed."),
            );
        }
    }

    /// The selector paints a full-surface scrim with the selected region cut
    /// out of it, so every change repaints the whole surface: an incremental
    /// buffer would otherwise keep the scrim from an earlier frame.
    fn mark_ocr_dirty(&mut self) {
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::{DrawingState, MouseButton, Tool};

    #[test]
    fn cancel_reports_auto_frozen_ownership() {
        let mut state = make_test_input_state();
        state.set_ocr_pending_capture(OcrCaptureSource::Frozen);
        state.activate_ocr(true);

        assert!(state.cancel_ocr());
        assert_eq!(state.ocr_state(), OcrUiState::Inactive);
    }

    #[test]
    fn waiting_for_zoom_does_not_claim_frozen_mode() {
        let mut state = make_test_input_state();
        state.set_ocr_pending_capture(OcrCaptureSource::Zoom);

        assert!(!state.cancel_ocr());
        assert_eq!(state.ocr_state(), OcrUiState::Inactive);
    }

    #[test]
    fn a_drag_records_both_corners_and_the_capture_it_owns() {
        let mut state = make_test_input_state();
        state.activate_ocr(true);

        assert!(state.start_ocr_selection(OcrInputSource::Pointer, (10.0, 20.0)));
        state.update_ocr_selection(OcrInputSource::Pointer, (60.0, 90.0));
        assert!(state.ocr_state().is_selecting());

        assert_eq!(
            state.ocr_state().selection(),
            Some(OcrSelection {
                start: (10.0, 20.0),
                end: (60.0, 90.0),
                auto_froze: true,
            })
        );
    }

    /// A pen already drawing when the selector opens leaves it `Armed`, never
    /// `Selecting`: the tip-up that follows has no region to submit and is
    /// swallowed. That is exactly why the backend retires the stylus contact
    /// when the modal opens instead of waiting for an `Up` it will consume.
    #[test]
    fn opening_the_selector_mid_stroke_cancels_the_stroke_and_arms_without_a_region() {
        let mut state = make_test_input_state();
        state.on_mouse_press(MouseButton::Left, 10, 20);
        state.on_mouse_motion(30, 40);
        assert!(matches!(state.state, DrawingState::Drawing { .. }));

        state.activate_ocr(true);

        assert!(matches!(state.state, DrawingState::Idle));
        assert_eq!(state.ocr_state(), OcrUiState::Armed { auto_froze: true });
        assert!(state.ocr_state().selection().is_none());
    }

    #[test]
    fn a_press_before_the_capture_lands_does_not_start_a_region() {
        let mut state = make_test_input_state();
        state.set_ocr_pending_capture(OcrCaptureSource::Frozen);

        assert!(!state.start_ocr_selection(OcrInputSource::Pointer, (10.0, 20.0)));
        assert!(state.ocr_state().selection().is_none());
    }

    #[test]
    fn cancelling_mid_drag_yields_no_region() {
        let mut state = make_test_input_state();
        state.activate_ocr(true);
        state.start_ocr_selection(OcrInputSource::Pointer, (10.0, 20.0));

        assert!(state.cancel_ocr());
        assert_eq!(state.ocr_state().selection(), None);
        // The state is gone, so a second cancel cannot release the freeze twice.
        assert!(!state.cancel_ocr());
    }

    /// The seat is not modal even though Wayscriber is: a pen can hover, and a
    /// contact retired when the selector opened keeps reporting until it lifts.
    /// Neither may steer a region the mouse is dragging.
    #[test]
    fn only_the_device_that_started_a_region_can_move_or_finish_it() {
        let mut state = make_test_input_state();
        state.activate_ocr(true);
        assert!(state.start_ocr_selection(OcrInputSource::Pointer, (10.0, 20.0)));

        state.update_ocr_selection(OcrInputSource::Stylus, (500.0, 500.0));
        state.update_ocr_selection(OcrInputSource::Touch, (400.0, 400.0));
        assert_eq!(
            state.ocr_state().selection().map(|region| region.end),
            Some((10.0, 20.0)),
            "another device dragged the pointer's region"
        );

        state.update_ocr_selection(OcrInputSource::Pointer, (60.0, 90.0));
        assert_eq!(
            state.ocr_state().selection().map(|region| region.end),
            Some((60.0, 90.0))
        );

        assert!(!state.ocr_selection_is_owned_by(OcrInputSource::Stylus));
        assert!(!state.ocr_selection_is_owned_by(OcrInputSource::Touch));
        assert!(state.ocr_selection_is_owned_by(OcrInputSource::Pointer));
    }

    /// A second device pressing mid-drag must not take the region over: the
    /// selector is no longer armed, so its press finds nothing to start.
    #[test]
    fn a_second_device_cannot_take_over_a_region_in_progress() {
        let mut state = make_test_input_state();
        state.activate_ocr(true);
        assert!(state.start_ocr_selection(OcrInputSource::Touch, (10.0, 20.0)));

        assert!(!state.start_ocr_selection(OcrInputSource::Pointer, (200.0, 200.0)));

        assert!(state.ocr_selection_is_owned_by(OcrInputSource::Touch));
        assert_eq!(
            state.ocr_state().selection().map(|region| region.start),
            Some((10.0, 20.0))
        );
    }

    /// Ownership is per drag, not per session: after one device's region ends,
    /// the re-armed selector accepts the next press from any device.
    #[test]
    fn ownership_resets_with_each_region() {
        let mut state = make_test_input_state();
        state.activate_ocr(true);
        state.start_ocr_selection(OcrInputSource::Stylus, (10.0, 20.0));
        state.rearm_ocr_selection();

        assert!(state.ocr_state().selection_owner().is_none());
        assert!(state.start_ocr_selection(OcrInputSource::Pointer, (30.0, 40.0)));
        assert!(state.ocr_selection_is_owned_by(OcrInputSource::Pointer));
    }

    #[test]
    fn a_mis_click_rearms_the_selector_instead_of_leaving_it() {
        let mut state = make_test_input_state();
        state.activate_ocr(true);
        state.start_ocr_selection(OcrInputSource::Pointer, (10.0, 20.0));

        state.rearm_ocr_selection();

        assert_eq!(state.ocr_state(), OcrUiState::Armed { auto_froze: true });
        assert!(state.ocr_is_active());
    }

    #[test]
    fn activation_cancels_an_interaction_started_while_capture_was_pending() {
        let mut state = make_test_input_state();
        state.set_ocr_pending_capture(OcrCaptureSource::Frozen);
        state.on_mouse_press(MouseButton::Left, 10, 20);
        assert!(matches!(state.state, DrawingState::Drawing { .. }));

        state.activate_ocr(true);

        assert!(matches!(state.state, DrawingState::Idle));
        assert!(state.active_drag_button.is_none());
        assert!(state.ocr_is_active());
        // The gesture cancelled here started during the wait, not at the
        // request — and the selector arms with no region, so the release that
        // ends it is swallowed. Activation is therefore the only point that can
        // retire the backend contact behind it.
        assert_eq!(state.ocr_state(), OcrUiState::Armed { auto_froze: true });
        assert!(state.ocr_state().selection().is_none());
    }

    /// OCR is an action, not a tool: whatever the user was drawing with before
    /// must still be selected after the selector opens, drags, and closes.
    #[test]
    fn the_whole_selector_lifecycle_preserves_every_representative_tool() {
        for tool in [
            Tool::Pen,
            Tool::Select,
            Tool::Marker,
            Tool::Eraser,
            Tool::Arrow,
            Tool::Highlight,
            Tool::Blur,
        ] {
            let mut state = make_test_input_state();
            state.set_tool_override(Some(tool));

            state.request_copy_text_from_screen();
            assert!(state.take_pending_ocr_request());
            state.set_ocr_pending_capture(OcrCaptureSource::Frozen);
            state.activate_ocr(true);
            state.start_ocr_selection(OcrInputSource::Pointer, (10.0, 10.0));
            state.update_ocr_selection(OcrInputSource::Pointer, (80.0, 60.0));
            state.cancel_ocr();

            assert_eq!(
                state.active_tool(),
                tool,
                "OCR changed the tool away from {tool:?}"
            );
        }
    }

    /// Nothing about a recognition is a canvas edit. One undo after the whole
    /// lifecycle must remove the stroke drawn before it — if OCR had pushed a
    /// history entry, that undo would have been spent on OCR instead.
    #[test]
    fn the_selector_lifecycle_adds_no_shape_and_no_history_entry() {
        let mut state = make_test_input_state();
        state.on_mouse_press(MouseButton::Left, 0, 0);
        state.on_mouse_motion(10, 10);
        state.on_mouse_release(MouseButton::Left, 10, 10);
        assert_eq!(state.boards.active_frame().shapes.len(), 1);

        state.activate_ocr(true);
        state.start_ocr_selection(OcrInputSource::Pointer, (10.0, 10.0));
        state.update_ocr_selection(OcrInputSource::Pointer, (120.0, 90.0));
        state.cancel_ocr();
        assert_eq!(state.boards.active_frame().shapes.len(), 1);

        state.handle_action(crate::domain::Action::Undo);

        assert_eq!(state.boards.active_frame().shapes.len(), 0);
    }

    /// The selector swallows every key press it receives. A key still held
    /// from before it opened — from the toolbar, the palette, or a mouse path —
    /// must not keep repeating into the canvas behind it.
    #[test]
    fn an_engaged_selector_owns_the_keyboard_and_stops_canvas_key_repeat() {
        let mut state = make_test_input_state();
        assert!(!state.modal_blocks_canvas_key_repeat());
        assert!(!state.modal_owns_text_input());

        state.set_ocr_pending_capture(OcrCaptureSource::Frozen);
        assert!(state.modal_blocks_canvas_key_repeat());
        assert!(state.modal_owns_text_input());

        state.activate_ocr(true);
        assert!(state.modal_blocks_canvas_key_repeat());
        assert!(state.modal_owns_text_input());

        state.cancel_ocr();
        assert!(!state.modal_blocks_canvas_key_repeat());
        assert!(!state.modal_owns_text_input());
    }

    #[test]
    fn capture_failure_preserves_a_more_specific_existing_error() {
        let mut state = make_test_input_state();
        state.push_toast(
            ToastPriority::Critical,
            "ocr",
            Toast::error("Freeze failed after the display changed size"),
        );

        state.report_ocr_capture_failure_if_unreported();

        assert_eq!(
            state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
            Some("Freeze failed after the display changed size")
        );
    }
}
