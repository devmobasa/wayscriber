use crate::backend::wayland::{RuntimeOperationController, RuntimeOperationIdSource};

use super::cut_review::review_edits_for_active_region;
use super::*;

pub(in crate::backend::wayland) struct RegionCaptureRuntime {
    active: Option<ActiveScreenRegion>,
    window_snap: Option<WindowSnapSession>,
    review_edits: Option<RegionReviewEdits>,
    next_generation: u64,
    window_query: RuntimeOperationController<
        WindowSnapQuery,
        Result<
            crate::capture::window_geometry::WindowQueryResult,
            crate::capture::window_geometry::WindowGeometryError,
        >,
    >,
    cut_preview: RuntimeOperationController<CutPreviewKey, CutPreviewOutcome>,
}

impl RegionCaptureRuntime {
    pub(in crate::backend::wayland) fn new(
        ids: RuntimeOperationIdSource,
        wake: crate::backend::wayland::RuntimeWakeHandle,
    ) -> Self {
        Self {
            active: None,
            window_snap: None,
            review_edits: None,
            next_generation: 1,
            window_query: RuntimeOperationController::new(ids.clone(), wake.clone()),
            cut_preview: RuntimeOperationController::new(ids, wake),
        }
    }

    pub(in crate::backend::wayland) fn next_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = generation
            .checked_add(1)
            .expect("screen region generation space exhausted");
        generation
    }

    pub(in crate::backend::wayland::state) fn active(&self) -> Option<ActiveScreenRegion> {
        self.active
    }

    pub(in crate::backend::wayland::state) fn active_mut(
        &mut self,
    ) -> Option<&mut ActiveScreenRegion> {
        self.active.as_mut()
    }

    pub(in crate::backend::wayland::state) fn active_slot_mut(
        &mut self,
    ) -> &mut Option<ActiveScreenRegion> {
        &mut self.active
    }

    pub(in crate::backend::wayland::state) fn begin_measure(&mut self, bounds: (u32, u32)) -> u64 {
        let generation = self.next_generation();
        self.active = Some(ActiveScreenRegion::Measure {
            generation,
            bounds,
            anchor: None,
            edge: None,
        });
        generation
    }

    pub(in crate::backend::wayland::state) fn set_pending(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        source: ScreenCaptureSource,
        acquisition: Option<ScreenAcquisitionId>,
    ) {
        self.active = Some(match source {
            ScreenCaptureSource::Frozen => ActiveScreenRegion::PendingFrozen {
                purpose,
                generation,
                acquisition: acquisition.expect("frozen region wait has an acquisition id"),
            },
            ScreenCaptureSource::Zoom => ActiveScreenRegion::PendingZoom {
                purpose,
                generation,
            },
        });
    }

    pub(in crate::backend::wayland::state) fn set_ready(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        source: ScreenSourceToken,
        freeze_ownership: FreezeOwnership,
        square_modifier: bool,
        include_drawings: bool,
    ) {
        self.active = Some(ActiveScreenRegion::Ready {
            purpose,
            generation,
            source,
            freeze_ownership,
            anchor: None,
            raw_edge: None,
            logical_anchor: None,
            logical_edge: None,
            square_modifier,
            legend_dismissed: false,
            include_drawings,
            review_resize: None,
        });
    }

    pub(in crate::backend::wayland) fn clear(&mut self) {
        self.window_snap = None;
        self.review_edits = None;
        self.active = None;
    }

    pub(in crate::backend::wayland) fn review_edits(&self) -> Option<&RegionReviewEdits> {
        self.review_edits.as_ref()
    }

    pub(in crate::backend::wayland) fn review_edits_mut(
        &mut self,
    ) -> Option<&mut RegionReviewEdits> {
        self.review_edits.as_mut()
    }

    pub(in crate::backend::wayland) fn review_edits_slot_mut(
        &mut self,
    ) -> &mut Option<RegionReviewEdits> {
        &mut self.review_edits
    }

    pub(in crate::backend::wayland::state) fn selection_parts(
        &mut self,
    ) -> (
        &mut Option<ActiveScreenRegion>,
        &mut Option<RegionReviewEdits>,
    ) {
        (&mut self.active, &mut self.review_edits)
    }

    pub(in crate::backend::wayland) fn set_review_edits_for(&mut self, rect: ImagePixelRect) {
        self.review_edits = review_edits_for_active_region(self.active, rect);
    }

    pub(in crate::backend::wayland::state) fn window_snap(&self) -> Option<&WindowSnapSession> {
        self.window_snap.as_ref()
    }

    pub(in crate::backend::wayland::state) fn window_snap_mut(
        &mut self,
    ) -> Option<&mut WindowSnapSession> {
        self.window_snap.as_mut()
    }

    pub(in crate::backend::wayland::state) fn set_window_snap(
        &mut self,
        session: WindowSnapSession,
    ) {
        self.window_snap = Some(session);
    }

    pub(in crate::backend::wayland) fn clear_window_snap(&mut self) {
        self.window_snap = None;
    }

    pub(in crate::backend::wayland::state) fn window_query_parts(
        &mut self,
    ) -> (
        &mut RuntimeOperationController<
            WindowSnapQuery,
            Result<
                crate::capture::window_geometry::WindowQueryResult,
                crate::capture::window_geometry::WindowGeometryError,
            >,
        >,
        &mut Option<WindowSnapSession>,
    ) {
        (&mut self.window_query, &mut self.window_snap)
    }

    pub(in crate::backend::wayland) fn cut_preview_mut(
        &mut self,
    ) -> &mut RuntimeOperationController<CutPreviewKey, CutPreviewOutcome> {
        &mut self.cut_preview
    }

    pub(in crate::backend::wayland) fn cut_preview_active(&self) -> bool {
        self.cut_preview.is_active()
    }
}

impl WaylandState {
    pub(in crate::backend::wayland::state) fn retire_region_selection_owner(
        &mut self,
        owner: Option<RegionInputSource>,
    ) {
        match owner {
            Some(source @ (RegionInputSource::Pointer | RegionInputSource::Touch)) => {
                self.pointer.suppress_release(source);
            }
            Some(RegionInputSource::Stylus) => self.retire_stylus_contact(),
            None => {}
        }
    }

    pub(in crate::backend::wayland) fn handle_measure_mode_action(&mut self) {
        match measure_mode_transition(
            self.input_state.region_state().purpose(),
            self.input_state.screen_modal_is_engaged(),
        ) {
            MeasureModeTransition::Cancel => {
                self.cancel_measure_mode();
                return;
            }
            MeasureModeTransition::Refuse => {
                self.input_state.push_toast(
                    crate::input::state::ToastPriority::Info,
                    "measure.refused",
                    crate::input::state::Toast::info(
                        "Finish or cancel the current screen selection first.",
                    ),
                );
                return;
            }
            MeasureModeTransition::Start => {}
        }

        self.input_state.prepare_for_screen_modal();
        self.zoom.stop_pan();
        self.pointer.stop_board_pan();
        self.pointer.set_board_pan_key_held(false);
        self.cancel_toolbar_move_drag();
        self.unlock_pointer();
        self.retire_stylus_contact();

        let generation = self
            .region_capture
            .begin_measure((self.surface.width(), self.surface.height()));
        self.input_state.activate_measure_mode(generation);
        self.debug_assert_screen_region_invariant();
    }

    pub(in crate::backend::wayland) fn cancel_measure_mode(&mut self) -> bool {
        if self.input_state.region_state().purpose() != Some(RegionPurposeTag::Measure) {
            return false;
        }
        self.clear_screen_region_ui_only();
        true
    }

    pub(in crate::backend::wayland) fn region_picker_include_drawings(&self) -> bool {
        self.region_capture
            .active()
            .is_some_and(ActiveScreenRegion::include_drawings)
    }

    pub(in crate::backend::wayland) fn toggle_region_picker_include_drawings(&mut self) -> bool {
        if !self.input_state.region_state().is_review() {
            return false;
        }
        let Some(checked) = self
            .region_capture
            .active_mut()
            .and_then(ActiveScreenRegion::toggle_include_drawings)
        else {
            return false;
        };
        log::debug!("region picker include drawings: {checked}");
        if self
            .region_capture
            .review_edits()
            .is_some_and(|edits| !edits.cuts.is_empty())
        {
            if let Some(fingerprint) = self.current_region_fingerprint()
                && let Some(edits) = self.region_capture.review_edits_mut()
            {
                edits.invalidate_base(fingerprint);
            }
            self.schedule_region_cut_preview();
        }
        self.mark_region_cut_ui_dirty();
        true
    }

    pub(in crate::backend::wayland::state) fn set_pending_screen_region(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        source: ScreenCaptureSource,
        acquisition: Option<ScreenAcquisitionId>,
    ) {
        self.region_capture
            .set_pending(purpose, generation, source, acquisition);
        self.input_state
            .set_region_pending_capture(purpose, generation, source);
        self.debug_assert_screen_region_invariant();
    }

    pub(in crate::backend::wayland::state) fn activate_screen_region(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        freeze_ownership: FreezeOwnership,
        include_drawings: bool,
    ) -> bool {
        let Some(source) = crate::backend::wayland::state::screen_image::displayed_screen_image(
            &self.zoom,
            &self.frozen,
            self.input_state.board_is_transparent(),
        ) else {
            return false;
        };
        let Some(token) = current_screen_source_token(
            &source,
            &self.zoom,
            &self.frozen,
            (self.surface.width(), self.surface.height()),
        ) else {
            return false;
        };
        self.region_capture.set_ready(
            purpose,
            generation,
            token,
            freeze_ownership,
            initial_square_modifier(purpose, self.input_state.modifiers.shift),
            include_drawings,
        );
        self.input_state.activate_region(purpose, generation);
        self.start_region_window_query(purpose, generation, token, freeze_ownership);
        self.debug_assert_screen_region_invariant();
        true
    }

    pub(in crate::backend::wayland::state) fn clear_screen_region_ui_only(&mut self) {
        self.region_capture.clear();
        self.input_state.cancel_region_ui_only();
        self.debug_assert_screen_region_invariant();
    }

    pub(in crate::backend::wayland) fn begin_region_selection(
        &mut self,
        owner: RegionInputSource,
        x: f64,
        y: f64,
    ) -> bool {
        self.cancel_screen_modals_if_source_changed();
        if self.begin_region_window_choice(owner, (x, y)) {
            return true;
        }
        begin_region_selection_event(
            self.region_capture.active_slot_mut(),
            &mut self.input_state,
            owner,
            (x, y),
        )
    }

    pub(in crate::backend::wayland) fn update_region_selection(
        &mut self,
        owner: RegionInputSource,
        x: f64,
        y: f64,
    ) {
        self.cancel_screen_modals_if_source_changed();
        if self.update_region_window_hover((x, y)) {
            return;
        }
        if self.update_region_cut_drag(owner, (x, y)) {
            return;
        }
        update_region_selection_event(
            self.region_capture.active_slot_mut(),
            &mut self.input_state,
            owner,
            (x, y),
        );
        if self.input_state.region_state().is_review() {
            self.sync_region_review_source_rect();
        }
    }

    pub(in crate::backend::wayland) fn region_selection_geometry(
        &self,
    ) -> Option<RegionSelectionGeometry> {
        if let Some(rect) = self.highlighted_region_window_rect()
            && let Some(ActiveScreenRegion::Ready {
                purpose, source, ..
            }) = self.region_capture.active()
        {
            let display = super::super::screen_image::screen_rect_for_image_rect(&source, rect);
            return Some(RegionSelectionGeometry::authoritative(
                purpose,
                rect,
                RegionSelection {
                    start: (f64::from(display.x), f64::from(display.y)),
                    end: (
                        f64::from(display.x + display.width),
                        f64::from(display.y + display.height),
                    ),
                },
            ));
        }
        self.region_capture
            .active()
            .and_then(ActiveScreenRegion::selection_geometry)
            .and_then(|geometry| {
                if !self.input_state.region_state().is_review() {
                    return Some(geometry);
                }
                let Some(display) = self.region_cut_displayed_selection() else {
                    return Some(geometry);
                };
                let purpose = self.region_capture.active()?.purpose();
                let rect = geometry.image_rect()?;
                Some(RegionSelectionGeometry::review(purpose, rect, display))
            })
    }

    pub(in crate::backend::wayland) fn region_measure_selection(&self) -> Option<RegionSelection> {
        self.region_capture
            .active()
            .and_then(ActiveScreenRegion::measure_selection)
    }

    pub(in crate::backend::wayland::state) fn region_picker_source_token(
        &self,
    ) -> Option<ScreenSourceToken> {
        match self.region_capture.active() {
            Some(ActiveScreenRegion::Ready { source, .. }) => Some(source),
            Some(ActiveScreenRegion::Measure { .. })
            | Some(ActiveScreenRegion::PendingFrozen { .. })
            | Some(ActiveScreenRegion::PendingZoom { .. })
            | None => None,
        }
    }

    pub(in crate::backend::wayland) fn region_picker_legend_dismissed(&self) -> bool {
        self.region_capture
            .active()
            .is_some_and(ActiveScreenRegion::legend_dismissed)
    }

    pub(in crate::backend::wayland) fn whole_image_region_selection(
        &self,
    ) -> Option<RegionSelectionFinalize> {
        self.region_capture
            .active()
            .and_then(ActiveScreenRegion::whole_image_selection)
    }

    pub(in crate::backend::wayland) fn enter_region_review(
        &mut self,
        rect: ImagePixelRect,
    ) -> bool {
        let Some(region) = self.region_capture.active_mut() else {
            return false;
        };
        let purpose = region.purpose();
        let generation = region.generation();
        let Some(display) = region.enter_review(rect) else {
            return false;
        };
        self.input_state
            .activate_region_review(purpose, generation, display);
        self.create_region_review_edits(rect);
        self.debug_assert_screen_region_invariant();
        true
    }

    pub(in crate::backend::wayland) fn nudge_region_review(
        &mut self,
        delta_x: i64,
        delta_y: i64,
    ) -> bool {
        self.cancel_screen_modals_if_source_changed();
        if !self.input_state.region_state().is_review() {
            return false;
        }
        if self.region_review_crop_locked() {
            return true;
        }
        let Some(display) = self
            .region_capture
            .active_mut()
            .and_then(|region| region.nudge_review(delta_x, delta_y))
        else {
            return false;
        };
        self.input_state.update_region_review_display(display);
        self.sync_region_review_source_rect();
        true
    }

    /// The grip a device is currently dragging in Review, if any.
    pub(in crate::backend::wayland) fn region_review_resize_handle(
        &self,
    ) -> Option<SelectionHandle> {
        if !self.input_state.region_state().is_review() {
            return None;
        }
        self.region_capture
            .active()
            .and_then(ActiveScreenRegion::review_resize_handle)
    }

    /// The grip under `point`, for hover feedback. Shares its placement with
    /// the press path and the renderer.
    pub(in crate::backend::wayland) fn region_review_handle_at(
        &self,
        point: (f64, f64),
    ) -> Option<SelectionHandle> {
        if !self.input_state.region_state().is_review() {
            return None;
        }
        if self.region_review_crop_locked() || self.region_cut_mode_armed() {
            return None;
        }
        self.region_capture
            .active()
            .and_then(|region| review_resize_handle_at(&region, point))
    }

    pub(in crate::backend::wayland) fn region_review_rect(&self) -> Option<ImagePixelRect> {
        match self.region_capture.active() {
            Some(region) if self.input_state.region_state().is_review() => {
                region.stored_review_rect()
            }
            _ => None,
        }
    }

    pub(in crate::backend::wayland) fn region_picker_measurement(
        &self,
        pointer: (f64, f64),
    ) -> Option<RegionPickerMeasurement> {
        if let Some(rect) = self.highlighted_region_window_rect() {
            return Some(RegionPickerMeasurement::Size {
                width: rect.width(),
                height: rect.height(),
            });
        }
        if self.input_state.region_state().is_review()
            && let Some(size) = self
                .region_capture
                .review_edits()
                .and_then(RegionReviewEdits::displayed_output_size)
        {
            return Some(RegionPickerMeasurement::Size {
                width: size.0,
                height: size.1,
            });
        }
        self.region_capture
            .active()
            .and_then(|region| region.picker_measurement(pointer))
    }

    /// Recompute capture geometry from compositor-authoritative Shift state.
    /// Individual key events remain swallowed by the modal selector.
    pub(in crate::backend::wayland) fn sync_region_square_modifier(&mut self, shift: bool) -> bool {
        sync_region_square_modifier_event(
            self.region_capture.active_slot_mut(),
            &mut self.input_state,
            shift,
        )
    }

    /// Resolve loss of the device that owns a drag without crossing lifecycle
    /// ownership. Capture keeps its reservation and image; OCR asks its owner
    /// to run the existing terminal cancellation path.
    pub(in crate::backend::wayland) fn region_owner_lost(
        &mut self,
        source: RegionInputSource,
    ) -> RegionOwnerLoss {
        if self.abandon_region_cut_drag(source) {
            return RegionOwnerLoss::Rearmed;
        }
        region_owner_lost_event(
            self.region_capture.active_slot_mut(),
            &mut self.input_state,
            source,
        )
    }

    pub(in crate::backend::wayland::state) fn debug_assert_screen_region_invariant(&self) {
        debug_assert!(screen_region_invariant(
            self.region_capture.active(),
            self.input_state.region_state(),
        ));
    }

    pub(in crate::backend::wayland) fn cancel_screen_modals_if_source_changed(&mut self) {
        let (cancel_eyedropper, cancel_region) = {
            let current_source =
                crate::backend::wayland::state::screen_image::displayed_screen_image(
                    &self.zoom,
                    &self.frozen,
                    self.input_state.board_is_transparent(),
                );
            let source_matches = |expected| {
                current_source.as_ref().is_some_and(|source| {
                    screen_source_is(
                        &expected,
                        source,
                        &self.zoom,
                        &self.frozen,
                        (self.surface.width(), self.surface.height()),
                    )
                })
            };
            (
                active_eyedropper_source_changed(
                    self.input_state.eyedropper_is_active(),
                    self.acquisition.eyedropper_source(),
                    &source_matches,
                ),
                active_region_source_changed(
                    self.region_capture.active(),
                    (self.surface.width(), self.surface.height()),
                    &source_matches,
                ),
            )
        };
        if cancel_eyedropper {
            self.cancel_eyedropper();
        }
        if cancel_region {
            match self
                .region_capture
                .active()
                .map(ActiveScreenRegion::purpose)
            {
                Some(RegionPurposeTag::Ocr) => {
                    self.cancel_ocr();
                }
                Some(RegionPurposeTag::CaptureDeliver | RegionPurposeTag::CaptureInteractive) => {
                    self.cancel_region_capture_for_source_change();
                }
                Some(RegionPurposeTag::Measure) => {
                    self.cancel_measure_mode();
                }
                None => {}
            }
        }
    }
}

#[cfg(test)]
mod owner_tests {
    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;

    fn runtime() -> RegionCaptureRuntime {
        let wake = RuntimeWakeSource::new().expect("runtime wake source");
        RegionCaptureRuntime::new(RuntimeOperationIdSource::new(), wake.handle())
    }

    #[test]
    fn generations_are_monotonic() {
        let mut runtime = runtime();

        assert_eq!(runtime.next_generation(), 1);
        assert_eq!(runtime.next_generation(), 2);
    }

    #[test]
    fn clearing_retires_the_active_region() {
        let mut runtime = runtime();
        let generation = runtime.begin_measure((1920, 1080));
        assert_eq!(generation, 1);

        runtime.clear();

        assert!(runtime.active().is_none());
        assert!(runtime.review_edits().is_none());
        assert!(runtime.window_snap().is_none());
    }

    #[test]
    #[should_panic(expected = "screen region generation space exhausted")]
    fn generation_exhaustion_is_explicit() {
        let mut runtime = runtime();
        runtime.next_generation = u64::MAX;

        let _ = runtime.next_generation();
    }
}
