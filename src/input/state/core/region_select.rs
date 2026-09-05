use super::InputState;

/// Why the generalized screen-region selector is engaged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionPurposeTag {
    Ocr,
    CaptureDeliver,
    CaptureInteractive,
    Measure,
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
            Self::Measure => SelectionPolicy {
                min_submit_logical_px: None,
                allow_square: false,
                snap_anchor: false,
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

/// Read-only UI projection of the backend-owned selector shared by OCR,
/// native screen-region capture, and the capture-free logical screen ruler.
///
/// The Wayland region-capture controller owns lifecycle, device ownership, and
/// geometry. `InputState` retains only this synchronized value for input gating
/// and painting.
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
    /// Measure mode keeps the last completed logical rectangle visible until
    /// a new press replaces it or the user exits the mode.
    Measured {
        purpose: RegionPurposeTag,
        generation: u64,
        display: RegionSelection,
    },
}

impl RegionSelectUiState {
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Armed { .. }
                | Self::Selecting { .. }
                | Self::Review { .. }
                | Self::Measured { .. }
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

    pub fn is_measured(self) -> bool {
        matches!(self, Self::Measured { .. })
    }

    /// The region drag in logical screen coordinates, if one is in progress.
    pub fn selection(self) -> Option<RegionSelection> {
        match self {
            Self::Selecting { start, current, .. } => Some(RegionSelection {
                start,
                end: current,
            }),
            Self::Review { display, .. } | Self::Measured { display, .. } => Some(display),
            Self::Inactive | Self::PendingCapture { .. } | Self::Armed { .. } => None,
        }
    }

    /// The device dragging the region, if one is in progress.
    pub fn selection_owner(self) -> Option<RegionInputSource> {
        match self {
            Self::Selecting { owner, .. } => Some(owner),
            Self::Review { move_owner, .. } => move_owner,
            Self::Inactive
            | Self::PendingCapture { .. }
            | Self::Armed { .. }
            | Self::Measured { .. } => None,
        }
    }

    pub fn pending_source(self) -> Option<ScreenCaptureSource> {
        match self {
            Self::PendingCapture { source, .. } => Some(source),
            Self::Inactive
            | Self::Armed { .. }
            | Self::Selecting { .. }
            | Self::Review { .. }
            | Self::Measured { .. } => None,
        }
    }

    pub fn purpose(self) -> Option<RegionPurposeTag> {
        match self {
            Self::PendingCapture { purpose, .. }
            | Self::Armed { purpose, .. }
            | Self::Selecting { purpose, .. }
            | Self::Review { purpose, .. }
            | Self::Measured { purpose, .. } => Some(purpose),
            Self::Inactive => None,
        }
    }

    pub fn generation(self) -> Option<u64> {
        match self {
            Self::PendingCapture { generation, .. }
            | Self::Armed { generation, .. }
            | Self::Selecting { generation, .. }
            | Self::Review { generation, .. }
            | Self::Measured { generation, .. } => Some(generation),
            Self::Inactive => None,
        }
    }
}

impl InputState {
    pub(crate) fn request_copy_text_from_screen(&mut self) {
        self.emit_input_effect(super::base::InputEffect::OcrPass {
            requested: true,
            dismissed_by_toolbar: false,
        });
    }

    /// Record that a toolbar interaction dismissed the selector.
    pub(crate) fn note_ocr_cancelled_by_toolbar(&mut self) {
        self.emit_input_effect(super::base::InputEffect::OcrPass {
            requested: false,
            dismissed_by_toolbar: true,
        });
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

    /// Replace the backend-owned selector projection used for input gating and
    /// painting. Lifecycle and geometry mutations remain in the Wayland
    /// region-capture controller.
    pub(crate) fn sync_region_projection(&mut self, next: RegionSelectUiState) {
        if self.region_select_ui_state == next {
            return;
        }
        let targeted_measure_damage = self.region_select_ui_state.purpose()
            == Some(RegionPurposeTag::Measure)
            || next.purpose() == Some(RegionPurposeTag::Measure);
        self.region_select_ui_state = next;
        if !targeted_measure_damage {
            self.dirty_tracker.mark_full();
        }
        self.needs_redraw = true;
    }
}

#[cfg(test)]
impl InputState {
    pub(crate) fn activate_measure_mode_with(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        generation: u64,
    ) {
        self.activate_region_with(measurer, RegionPurposeTag::Measure, generation);
    }

    pub(crate) fn set_region_pending_capture(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        source: ScreenCaptureSource,
    ) {
        self.sync_region_projection(RegionSelectUiState::PendingCapture {
            purpose,
            generation,
            source,
        });
    }

    pub(crate) fn activate_region_with(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        purpose: RegionPurposeTag,
        generation: u64,
    ) {
        self.prepare_for_screen_modal_with_measurer(measurer);
        self.sync_region_projection(RegionSelectUiState::Armed {
            purpose,
            generation,
        });
    }

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
            }
            | RegionSelectUiState::Measured {
                purpose,
                generation,
                ..
            } => (purpose, generation),
            _ => return false,
        };
        self.sync_region_projection(RegionSelectUiState::Selecting {
            purpose,
            generation,
            owner,
            start: point,
            current: point,
        });
        true
    }

    pub(crate) fn activate_region_review(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        display: RegionSelection,
    ) {
        self.sync_region_projection(RegionSelectUiState::Review {
            purpose,
            generation,
            display,
            move_owner: None,
        });
    }

    pub(crate) fn begin_region_review_move(&mut self, owner: RegionInputSource) -> bool {
        let RegionSelectUiState::Review {
            purpose,
            generation,
            display,
            move_owner: None,
        } = self.region_select_ui_state
        else {
            return false;
        };
        self.sync_region_projection(RegionSelectUiState::Review {
            purpose,
            generation,
            display,
            move_owner: Some(owner),
        });
        true
    }

    pub(crate) fn update_region_review_display(&mut self, display: RegionSelection) {
        let RegionSelectUiState::Review {
            purpose,
            generation,
            move_owner,
            ..
        } = self.region_select_ui_state
        else {
            return;
        };
        self.sync_region_projection(RegionSelectUiState::Review {
            purpose,
            generation,
            display,
            move_owner,
        });
    }

    pub(crate) fn finish_region_review_move(&mut self, owner: RegionInputSource) -> bool {
        let RegionSelectUiState::Review {
            purpose,
            generation,
            display,
            move_owner: Some(current),
        } = self.region_select_ui_state
        else {
            return false;
        };
        if current != owner {
            return false;
        }
        self.sync_region_projection(RegionSelectUiState::Review {
            purpose,
            generation,
            display,
            move_owner: None,
        });
        true
    }

    pub(crate) fn update_region_selection(&mut self, source: RegionInputSource, point: (f64, f64)) {
        let RegionSelectUiState::Selecting {
            purpose,
            generation,
            owner,
            start,
            ..
        } = self.region_select_ui_state
        else {
            return;
        };
        if owner == source {
            self.sync_region_projection(RegionSelectUiState::Selecting {
                purpose,
                generation,
                owner,
                start,
                current: point,
            });
        }
    }

    pub(crate) fn complete_measurement(&mut self, source: RegionInputSource) -> bool {
        let RegionSelectUiState::Selecting {
            purpose: RegionPurposeTag::Measure,
            generation,
            owner,
            start,
            current,
        } = self.region_select_ui_state
        else {
            return false;
        };
        if owner != source {
            return false;
        }
        self.sync_region_projection(RegionSelectUiState::Measured {
            purpose: RegionPurposeTag::Measure,
            generation,
            display: RegionSelection {
                start,
                end: current,
            },
        });
        true
    }

    pub(crate) fn region_selection_is_owned_by(&self, source: RegionInputSource) -> bool {
        self.region_select_ui_state.selection_owner() == Some(source)
    }

    pub(crate) fn rearm_region_selection(&mut self) {
        if let RegionSelectUiState::Selecting {
            purpose,
            generation,
            ..
        } = self.region_select_ui_state
        {
            self.sync_region_projection(RegionSelectUiState::Armed {
                purpose,
                generation,
            });
        }
    }

    pub(crate) fn cancel_region_ui_only(&mut self) {
        self.sync_region_projection(RegionSelectUiState::Inactive);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::state::{InputEffect, InputEffectDrain};
    use crate::input::{DrawingState, MouseButton, Tool};

    // Frozen-capture ownership and capture-failure reporting intentionally no
    // longer live in InputState; their regressions belong to the backend
    // acquisition and routing tests. This suite preserves the former OCR UI,
    // interaction, tool, and history guarantees at their generalized seam.

    fn activate_ocr_region(state: &mut InputState, generation: u64) {
        state.activate_region_with(
            &crate::draw::TextMeasurer::default(),
            RegionPurposeTag::Ocr,
            generation,
        );
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
    fn measure_mode_keeps_the_completed_rectangle_without_capture_state() {
        let mut state = make_test_input_state();
        state.activate_measure_mode_with(&crate::draw::TextMeasurer::default(), 7);
        assert!(state.start_region_selection(RegionInputSource::Pointer, (12.0, 18.0)));
        state.update_region_selection(RegionInputSource::Pointer, (42.0, 63.0));

        assert!(state.complete_measurement(RegionInputSource::Pointer));

        assert!(state.region_state().is_measured());
        assert_eq!(
            state.region_state().selection(),
            Some(RegionSelection {
                start: (12.0, 18.0),
                end: (42.0, 63.0),
            })
        );
        assert!(state.take_pending_backend_action().is_none());
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
    /// The cancellation signal merges with the request for that input batch.
    #[test]
    fn the_toolbar_cancel_signal_merges_with_the_same_batch_request() {
        let mut state = make_test_input_state();

        activate_ocr_region(&mut state, 6);
        state.cancel_region_ui_only();
        state.note_ocr_cancelled_by_toolbar();
        state.request_copy_text_from_screen();

        assert!(matches!(
            state.drain_input_effects(InputEffectDrain::Runtime).first(),
            Some(InputEffect::OcrPass {
                requested: true,
                dismissed_by_toolbar: true
            })
        ));
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
        assert!(!state.pointer_drag_active());
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
        assert!(!state.pointer_drag_active());
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
        let test_text_measurer = crate::draw::TextMeasurer::default();
        let test_ui_engine = crate::ui_text::UiTextEngine::default();
        let test_text_resources = crate::input::state::InputTextResources {
            measurer: &test_text_measurer,
            ui_engine: &test_ui_engine,
        };

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

        state.handle_action_with_resources(test_text_resources, crate::domain::Action::Undo);

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
