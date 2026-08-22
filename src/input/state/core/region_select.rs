use super::InputState;

/// Why the generalized screen-region selector is engaged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionPurposeTag {
    Ocr,
    CaptureDeliver,
    CaptureInteractive,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionPolicy {
    min_submit_logical_px: Option<f64>,
    allow_square: bool,
    snap_anchor: bool,
}

impl SelectionPolicy {
    pub const fn min_submit_logical_px(self) -> Option<f64> {
        self.min_submit_logical_px
    }

    pub const fn allow_square(self) -> bool {
        self.allow_square
    }

    pub const fn snap_anchor(self) -> bool {
        self.snap_anchor
    }
}

impl RegionPurposeTag {
    pub const fn is_capture(self) -> bool {
        matches!(self, Self::CaptureDeliver | Self::CaptureInteractive)
    }

    pub const fn selection_policy(self) -> SelectionPolicy {
        match self {
            Self::Ocr => SelectionPolicy {
                min_submit_logical_px: Some(4.0),
                allow_square: false,
                snap_anchor: false,
            },
            Self::CaptureDeliver | Self::CaptureInteractive => SelectionPolicy {
                min_submit_logical_px: None,
                allow_square: true,
                snap_anchor: true,
            },
        }
    }
}

/// Which captured screen source a region selector is waiting on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenCaptureSource {
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
pub enum RegionInputSource {
    Pointer,
    Touch,
    Stylus,
}

/// A finished or in-progress region drag in logical screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionSelection {
    pub start: (f64, f64),
    pub end: (f64, f64),
}

/// UI-facing lifecycle shared by OCR and native screen-region capture.
///
/// It owns transient input and render state only. The selected pixels and the
/// recognition work belong outside `InputState`, so cancelling here can never
/// discard a request that is already running.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum RegionSelectUiState {
    #[default]
    Inactive,
    PendingCapture {
        purpose: RegionPurposeTag,
        generation: u64,
        source: ScreenCaptureSource,
    },
    /// The selector is live and waiting for the press that starts a region.
    Armed {
        purpose: RegionPurposeTag,
        generation: u64,
    },
    /// A region drag is in progress between `start` and `current`, owned by the
    /// device that pressed. Only that device may move, release, or cancel it.
    Selecting {
        purpose: RegionPurposeTag,
        generation: u64,
        owner: RegionInputSource,
        start: (f64, f64),
        current: (f64, f64),
    },
    /// An interactive capture has a non-empty, image-space rectangle ready
    /// for an explicit destination choice. `display` is derived from that
    /// rectangle and exists here only for painting and hit testing.
    Review {
        purpose: RegionPurposeTag,
        generation: u64,
        display: RegionSelection,
        move_owner: Option<RegionInputSource>,
    },
}

impl RegionSelectUiState {
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Armed { .. } | Self::Selecting { .. } | Self::Review { .. }
        )
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

    pub fn is_review(self) -> bool {
        matches!(self, Self::Review { .. })
    }

    /// The region drag in logical screen coordinates, if one is in progress.
    pub fn selection(self) -> Option<RegionSelection> {
        match self {
            Self::Selecting { start, current, .. } => Some(RegionSelection {
                start,
                end: current,
            }),
            Self::Review { display, .. } => Some(display),
            Self::Inactive | Self::PendingCapture { .. } | Self::Armed { .. } => None,
        }
    }

    /// The device dragging the region, if one is in progress.
    pub fn selection_owner(self) -> Option<RegionInputSource> {
        match self {
            Self::Selecting { owner, .. } => Some(owner),
            Self::Review { move_owner, .. } => move_owner,
            Self::Inactive | Self::PendingCapture { .. } | Self::Armed { .. } => None,
        }
    }

    pub fn pending_source(self) -> Option<ScreenCaptureSource> {
        match self {
            Self::PendingCapture { source, .. } => Some(source),
            Self::Inactive | Self::Armed { .. } | Self::Selecting { .. } | Self::Review { .. } => {
                None
            }
        }
    }

    pub fn purpose(self) -> Option<RegionPurposeTag> {
        match self {
            Self::PendingCapture { purpose, .. }
            | Self::Armed { purpose, .. }
            | Self::Selecting { purpose, .. }
            | Self::Review { purpose, .. } => Some(purpose),
            Self::Inactive => None,
        }
    }

    pub fn generation(self) -> Option<u64> {
        match self {
            Self::PendingCapture { generation, .. }
            | Self::Armed { generation, .. }
            | Self::Selecting { generation, .. }
            | Self::Review { generation, .. } => Some(generation),
            Self::Inactive => None,
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

    /// Record that a toolbar interaction dismissed the selector.
    pub(crate) fn note_ocr_cancelled_by_toolbar(&mut self) {
        self.ocr_cancelled_by_toolbar = true;
    }

    pub(crate) fn take_ocr_cancelled_by_toolbar(&mut self) -> bool {
        std::mem::take(&mut self.ocr_cancelled_by_toolbar)
    }

    pub fn region_state(&self) -> RegionSelectUiState {
        self.region_select_ui_state
    }

    pub fn region_is_active(&self) -> bool {
        self.region_select_ui_state.is_active()
    }

    pub fn region_is_engaged(&self) -> bool {
        self.region_select_ui_state.is_engaged()
    }

    pub(crate) fn set_region_pending_capture(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        source: ScreenCaptureSource,
    ) {
        self.region_select_ui_state = RegionSelectUiState::PendingCapture {
            purpose,
            generation,
            source,
        };
        self.mark_region_dirty();
    }

    pub(crate) fn activate_region(&mut self, purpose: RegionPurposeTag, generation: u64) {
        // A capture can take long enough for another interaction to begin while
        // OCR is pending. Entering the modal state must cancel it so the
        // selector cannot swallow the matching release event.
        self.prepare_for_screen_modal();
        self.region_select_ui_state = RegionSelectUiState::Armed {
            purpose,
            generation,
        };
        self.mark_region_dirty();
    }

    /// Begin a region drag owned by `owner`.
    ///
    /// Ignored unless the selector is armed, so a stray press during capture
    /// cannot start a region against a stale image — and, because a drag in
    /// progress is no longer armed, a second device cannot take one over.
    pub(crate) fn start_region_selection(
        &mut self,
        owner: RegionInputSource,
        point: (f64, f64),
    ) -> bool {
        let (purpose, generation) = match self.region_select_ui_state {
            RegionSelectUiState::Armed {
                purpose,
                generation,
            }
            | RegionSelectUiState::Review {
                purpose,
                generation,
                ..
            } => (purpose, generation),
            _ => return false,
        };
        self.region_select_ui_state = RegionSelectUiState::Selecting {
            purpose,
            generation,
            owner,
            start: point,
            current: point,
        };
        self.mark_region_dirty();
        true
    }

    pub(crate) fn activate_region_review(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        display: RegionSelection,
    ) {
        debug_assert_eq!(purpose, RegionPurposeTag::CaptureInteractive);
        self.region_select_ui_state = RegionSelectUiState::Review {
            purpose,
            generation,
            display,
            move_owner: None,
        };
        self.mark_region_dirty();
    }

    pub(crate) fn begin_region_review_move(&mut self, owner: RegionInputSource) -> bool {
        let RegionSelectUiState::Review { move_owner, .. } = &mut self.region_select_ui_state
        else {
            return false;
        };
        if move_owner.is_some() {
            return false;
        }
        *move_owner = Some(owner);
        self.mark_region_dirty();
        true
    }

    pub(crate) fn update_region_review_display(&mut self, display: RegionSelection) {
        let RegionSelectUiState::Review {
            display: current, ..
        } = &mut self.region_select_ui_state
        else {
            return;
        };
        if *current != display {
            *current = display;
            self.mark_region_dirty();
        }
    }

    pub(crate) fn finish_region_review_move(&mut self, owner: RegionInputSource) -> bool {
        let RegionSelectUiState::Review { move_owner, .. } = &mut self.region_select_ui_state
        else {
            return false;
        };
        if *move_owner != Some(owner) {
            return false;
        }
        *move_owner = None;
        self.mark_region_dirty();
        true
    }

    /// Move the region, if `source` is the device dragging it. Motion from any
    /// other device is somebody else's and is dropped.
    pub(crate) fn update_region_selection(&mut self, source: RegionInputSource, point: (f64, f64)) {
        if let RegionSelectUiState::Selecting { owner, current, .. } =
            &mut self.region_select_ui_state
            && *owner == source
            && *current != point
        {
            *current = point;
            self.mark_region_dirty();
        }
    }

    /// Whether `source` is the device dragging the region right now. The
    /// release and per-device cancellation paths check this before acting, so
    /// one device cannot submit or discard another's region.
    pub(crate) fn region_selection_is_owned_by(&self, source: RegionInputSource) -> bool {
        self.region_select_ui_state.selection_owner() == Some(source)
    }

    /// Abandon a drag that was too small to be a region and wait for the next
    /// one. A mis-click should not drop the user out of the selector, and it
    /// must not release a capture the selector is still using.
    pub(crate) fn rearm_region_selection(&mut self) {
        if let RegionSelectUiState::Selecting {
            purpose,
            generation,
            ..
        } = self.region_select_ui_state
        {
            self.region_select_ui_state = RegionSelectUiState::Armed {
                purpose,
                generation,
            };
            self.mark_region_dirty();
        }
    }

    pub(crate) fn cancel_region_ui_only(&mut self) {
        if !matches!(self.region_select_ui_state, RegionSelectUiState::Inactive) {
            self.region_select_ui_state = RegionSelectUiState::Inactive;
            self.mark_region_dirty();
        }
    }

    /// The selector paints a full-surface scrim with the selected region cut
    /// out of it, so every change repaints the whole surface: an incremental
    /// buffer would otherwise keep the scrim from an earlier frame.
    fn mark_region_dirty(&mut self) {
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::{DrawingState, MouseButton, Tool};

    // Frozen-capture ownership and capture-failure reporting intentionally no
    // longer live in InputState; their regressions belong to the backend
    // acquisition and routing tests. This suite preserves the former OCR UI,
    // interaction, tool, and history guarantees at their generalized seam.

    fn activate_ocr_region(state: &mut InputState, generation: u64) {
        state.activate_region(RegionPurposeTag::Ocr, generation);
    }

    #[test]
    fn region_policy_preserves_ocr_geometry_and_reserves_capture_geometry() {
        assert!(!RegionPurposeTag::Ocr.is_capture());
        let ocr = RegionPurposeTag::Ocr.selection_policy();
        assert_eq!(ocr.min_submit_logical_px(), Some(4.0));
        assert!(!ocr.allow_square());
        assert!(!ocr.snap_anchor());

        for purpose in [
            RegionPurposeTag::CaptureDeliver,
            RegionPurposeTag::CaptureInteractive,
        ] {
            assert!(purpose.is_capture());
            let capture = purpose.selection_policy();
            assert_eq!(capture.min_submit_logical_px(), None);
            assert!(capture.allow_square());
            assert!(capture.snap_anchor());
        }
    }

    #[test]
    fn neutral_region_ui_tracks_purpose_generation_source_and_raw_preview() {
        let mut state = make_test_input_state();
        state.set_region_pending_capture(RegionPurposeTag::Ocr, 17, ScreenCaptureSource::Frozen);
        assert_eq!(state.region_state().generation(), Some(17));
        assert_eq!(state.region_state().purpose(), Some(RegionPurposeTag::Ocr));
        assert_eq!(
            state.region_state().pending_source(),
            Some(ScreenCaptureSource::Frozen)
        );

        activate_ocr_region(&mut state, 17);
        assert!(state.start_region_selection(RegionInputSource::Pointer, (10.25, 20.75),));
        state.update_region_selection(RegionInputSource::Pointer, (14.5, 27.125));

        assert_eq!(
            state.region_state().selection(),
            Some(RegionSelection {
                start: (10.25, 20.75),
                end: (14.5, 27.125),
            })
        );
        assert_eq!(state.region_state().generation(), Some(17));
    }

    #[test]
    fn device_ownership_rearm_and_cancel_are_purpose_neutral() {
        let mut state = make_test_input_state();
        activate_ocr_region(&mut state, 3);
        assert!(state.start_region_selection(RegionInputSource::Pointer, (10.0, 20.0)));
        state.update_region_selection(RegionInputSource::Stylus, (500.0, 500.0));
        assert_eq!(
            state.region_state().selection().map(|region| region.end),
            Some((10.0, 20.0))
        );
        state.update_region_selection(RegionInputSource::Pointer, (60.0, 90.0));
        assert!(state.region_selection_is_owned_by(RegionInputSource::Pointer));

        state.rearm_region_selection();
        assert_eq!(
            state.region_state(),
            RegionSelectUiState::Armed {
                purpose: RegionPurposeTag::Ocr,
                generation: 3,
            }
        );
        state.cancel_region_ui_only();
        assert_eq!(state.region_state(), RegionSelectUiState::Inactive);
    }

    #[test]
    fn interactive_review_keeps_display_and_move_ownership_explicit() {
        let mut state = make_test_input_state();
        let display = RegionSelection {
            start: (20.0, 30.0),
            end: (80.0, 90.0),
        };
        state.activate_region_review(RegionPurposeTag::CaptureInteractive, 14, display);

        assert!(state.region_state().is_review());
        assert!(state.region_is_active());
        assert_eq!(state.region_state().selection(), Some(display));
        assert!(state.begin_region_review_move(RegionInputSource::Pointer));
        assert!(!state.begin_region_review_move(RegionInputSource::Touch));
        assert!(state.region_selection_is_owned_by(RegionInputSource::Pointer));

        let moved = RegionSelection {
            start: (25.0, 35.0),
            end: (85.0, 95.0),
        };
        state.update_region_review_display(moved);
        assert_eq!(state.region_state().selection(), Some(moved));
        assert!(!state.finish_region_review_move(RegionInputSource::Stylus));
        assert!(state.finish_region_review_move(RegionInputSource::Pointer));
        assert!(state.region_state().selection_owner().is_none());
    }

    #[test]
    fn pressing_outside_review_can_start_a_fresh_owned_selection() {
        let mut state = make_test_input_state();
        state.activate_region_review(
            RegionPurposeTag::CaptureInteractive,
            15,
            RegionSelection {
                start: (20.0, 30.0),
                end: (80.0, 90.0),
            },
        );

        assert!(state.start_region_selection(RegionInputSource::Touch, (4.0, 5.0)));
        assert!(state.region_state().is_selecting());
        assert!(state.region_selection_is_owned_by(RegionInputSource::Touch));
        assert_eq!(
            state.region_state().selection(),
            Some(RegionSelection {
                start: (4.0, 5.0),
                end: (4.0, 5.0),
            })
        );
    }

    #[test]
    fn a_press_before_the_capture_lands_does_not_start_a_region() {
        let mut state = make_test_input_state();
        state.set_region_pending_capture(RegionPurposeTag::Ocr, 4, ScreenCaptureSource::Frozen);

        assert!(!state.start_region_selection(RegionInputSource::Pointer, (10.0, 20.0)));
        assert!(state.region_state().selection().is_none());
        assert_eq!(
            state.region_state().pending_source(),
            Some(ScreenCaptureSource::Frozen)
        );
    }

    #[test]
    fn waiting_for_zoom_can_be_cancelled_without_leaving_region_state() {
        let mut state = make_test_input_state();
        state.set_region_pending_capture(RegionPurposeTag::Ocr, 5, ScreenCaptureSource::Zoom);

        assert_eq!(
            state.region_state().pending_source(),
            Some(ScreenCaptureSource::Zoom)
        );
        state.cancel_region_ui_only();

        assert_eq!(state.region_state(), RegionSelectUiState::Inactive);
        assert!(state.region_state().selection().is_none());
    }

    #[test]
    fn cancelling_mid_drag_yields_no_region_and_a_second_cancel_is_a_no_op() {
        let mut state = make_test_input_state();
        activate_ocr_region(&mut state, 5);
        assert!(state.start_region_selection(RegionInputSource::Pointer, (10.0, 20.0)));

        state.cancel_region_ui_only();
        assert_eq!(state.region_state(), RegionSelectUiState::Inactive);
        assert!(state.region_state().selection().is_none());

        state.cancel_region_ui_only();
        assert_eq!(state.region_state(), RegionSelectUiState::Inactive);
    }

    /// Clicking the OCR toolbar button while the selector is up toggles it off.
    /// The cancellation latch belongs to that input batch and must not survive it.
    #[test]
    fn the_toolbar_cancel_latch_marks_one_batch_and_then_clears() {
        let mut state = make_test_input_state();
        assert!(!state.take_ocr_cancelled_by_toolbar());

        activate_ocr_region(&mut state, 6);
        state.cancel_region_ui_only();
        state.note_ocr_cancelled_by_toolbar();
        state.request_copy_text_from_screen();

        assert!(state.take_ocr_cancelled_by_toolbar());
        assert!(state.take_pending_ocr_request());
        assert!(
            !state.take_ocr_cancelled_by_toolbar(),
            "the latch outlived the interaction that set it"
        );
    }

    #[test]
    fn only_the_device_that_started_a_region_can_move_it() {
        let mut state = make_test_input_state();
        activate_ocr_region(&mut state, 7);
        assert!(state.start_region_selection(RegionInputSource::Pointer, (10.0, 20.0)));

        state.update_region_selection(RegionInputSource::Stylus, (500.0, 500.0));
        state.update_region_selection(RegionInputSource::Touch, (400.0, 400.0));
        assert_eq!(
            state.region_state().selection().map(|region| region.end),
            Some((10.0, 20.0)),
            "another device dragged the pointer's region"
        );

        state.update_region_selection(RegionInputSource::Pointer, (60.0, 90.0));
        assert_eq!(
            state.region_state().selection().map(|region| region.end),
            Some((60.0, 90.0))
        );
        assert!(!state.region_selection_is_owned_by(RegionInputSource::Stylus));
        assert!(!state.region_selection_is_owned_by(RegionInputSource::Touch));
        assert!(state.region_selection_is_owned_by(RegionInputSource::Pointer));
    }

    #[test]
    fn a_second_device_cannot_take_over_a_region_in_progress() {
        let mut state = make_test_input_state();
        activate_ocr_region(&mut state, 8);
        assert!(state.start_region_selection(RegionInputSource::Touch, (10.0, 20.0)));

        assert!(!state.start_region_selection(RegionInputSource::Pointer, (200.0, 200.0)));
        assert!(state.region_selection_is_owned_by(RegionInputSource::Touch));
        assert_eq!(
            state.region_state().selection().map(|region| region.start),
            Some((10.0, 20.0))
        );
    }

    #[test]
    fn ownership_resets_with_each_region() {
        let mut state = make_test_input_state();
        activate_ocr_region(&mut state, 9);
        assert!(state.start_region_selection(RegionInputSource::Stylus, (10.0, 20.0)));
        state.rearm_region_selection();

        assert!(state.region_state().selection_owner().is_none());
        assert!(state.start_region_selection(RegionInputSource::Pointer, (30.0, 40.0)));
        assert!(state.region_selection_is_owned_by(RegionInputSource::Pointer));
    }

    #[test]
    fn opening_the_selector_mid_stroke_cancels_the_stroke_and_arms_without_a_region() {
        let mut state = make_test_input_state();
        state.set_tool_override(Some(Tool::Marker));
        state.on_mouse_press(MouseButton::Left, 10, 20);
        state.on_mouse_motion(30, 40);
        assert!(matches!(state.state, DrawingState::Drawing { .. }));

        activate_ocr_region(&mut state, 10);

        assert!(matches!(state.state, DrawingState::Idle));
        assert!(state.active_drag_button.is_none());
        assert_eq!(state.active_tool(), Tool::Marker);
        assert_eq!(
            state.region_state(),
            RegionSelectUiState::Armed {
                purpose: RegionPurposeTag::Ocr,
                generation: 10,
            }
        );
        assert!(state.region_state().selection().is_none());
    }

    #[test]
    fn activation_cancels_an_interaction_started_while_capture_was_pending() {
        let mut state = make_test_input_state();
        state.set_region_pending_capture(RegionPurposeTag::Ocr, 11, ScreenCaptureSource::Frozen);
        state.on_mouse_press(MouseButton::Left, 10, 20);
        assert!(matches!(state.state, DrawingState::Drawing { .. }));

        activate_ocr_region(&mut state, 11);

        assert!(matches!(state.state, DrawingState::Idle));
        assert!(state.active_drag_button.is_none());
        assert_eq!(
            state.region_state(),
            RegionSelectUiState::Armed {
                purpose: RegionPurposeTag::Ocr,
                generation: 11,
            }
        );
        assert!(state.region_state().selection().is_none());
    }

    /// OCR is an action, not a tool: the whole selector lifecycle must preserve
    /// whichever representative drawing tool the user had selected.
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
            state.set_region_pending_capture(
                RegionPurposeTag::Ocr,
                12,
                ScreenCaptureSource::Frozen,
            );
            activate_ocr_region(&mut state, 12);
            assert!(state.start_region_selection(RegionInputSource::Pointer, (10.0, 10.0)));
            state.update_region_selection(RegionInputSource::Pointer, (80.0, 60.0));
            state.cancel_region_ui_only();

            assert_eq!(
                state.active_tool(),
                tool,
                "OCR changed the tool away from {tool:?}"
            );
        }
    }

    /// Region selection is not a canvas edit. One undo after the lifecycle must
    /// remove the stroke drawn before it, proving no region history entry was added.
    #[test]
    fn the_selector_lifecycle_adds_no_shape_and_no_history_entry() {
        let mut state = make_test_input_state();
        state.on_mouse_press(MouseButton::Left, 0, 0);
        state.on_mouse_motion(10, 10);
        state.on_mouse_release(MouseButton::Left, 10, 10);
        assert_eq!(state.boards.active_frame().shapes.len(), 1);

        activate_ocr_region(&mut state, 13);
        assert!(state.start_region_selection(RegionInputSource::Pointer, (10.0, 10.0)));
        state.update_region_selection(RegionInputSource::Pointer, (120.0, 90.0));
        state.cancel_region_ui_only();
        assert_eq!(state.boards.active_frame().shapes.len(), 1);

        state.handle_action(crate::domain::Action::Undo);

        assert_eq!(state.boards.active_frame().shapes.len(), 0);
    }

    #[test]
    fn engaged_region_selector_owns_modal_keyboard_and_text_input() {
        let mut state = make_test_input_state();
        assert!(!state.modal_blocks_canvas_key_repeat());
        assert!(!state.modal_owns_text_input());

        state.set_region_pending_capture(RegionPurposeTag::Ocr, 5, ScreenCaptureSource::Frozen);
        assert!(state.modal_blocks_canvas_key_repeat());
        assert!(state.modal_owns_text_input());

        activate_ocr_region(&mut state, 5);
        assert!(state.modal_blocks_canvas_key_repeat());
        assert!(state.modal_owns_text_input());

        state.cancel_region_ui_only();
        assert!(!state.modal_blocks_canvas_key_repeat());
        assert!(!state.modal_owns_text_input());
    }
}
