use super::*;

impl WaylandState {
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
        self.stop_board_pan();
        self.set_board_pan_key_held(false);
        self.cancel_toolbar_move_drag();
        self.unlock_pointer();
        self.retire_stylus_contact();

        let generation = self.next_screen_region_generation();
        self.data.active_screen_region = Some(ActiveScreenRegion::Measure {
            generation,
            bounds: (self.surface.width(), self.surface.height()),
            anchor: None,
            edge: None,
        });
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
        self.data
            .active_screen_region
            .is_some_and(ActiveScreenRegion::include_drawings)
    }

    pub(in crate::backend::wayland) fn toggle_region_picker_include_drawings(&mut self) -> bool {
        if !self.input_state.region_state().is_review() {
            return false;
        }
        let Some(checked) = self
            .data
            .active_screen_region
            .as_mut()
            .and_then(ActiveScreenRegion::toggle_include_drawings)
        else {
            return false;
        };
        log::debug!("region picker include drawings: {checked}");
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
        true
    }

    pub(in crate::backend::wayland::state) fn next_screen_region_generation(&mut self) -> u64 {
        let generation = self.data.next_screen_region_generation;
        self.data.next_screen_region_generation = generation
            .checked_add(1)
            .expect("screen region generation space exhausted");
        generation
    }

    pub(in crate::backend::wayland::state) fn set_pending_screen_region(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        source: ScreenCaptureSource,
        acquisition: Option<ScreenAcquisitionId>,
    ) {
        self.data.active_screen_region = Some(match source {
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
        self.data.active_screen_region = Some(ActiveScreenRegion::Ready {
            purpose,
            generation,
            source: token,
            freeze_ownership,
            anchor: None,
            raw_edge: None,
            logical_anchor: None,
            logical_edge: None,
            square_modifier: initial_square_modifier(purpose, self.input_state.modifiers.shift),
            legend_dismissed: false,
            include_drawings,
        });
        self.input_state.activate_region(purpose, generation);
        self.debug_assert_screen_region_invariant();
        true
    }

    pub(in crate::backend::wayland::state) fn clear_screen_region_ui_only(&mut self) {
        self.data.active_screen_region = None;
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
        begin_region_selection_event(
            &mut self.data.active_screen_region,
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
        update_region_selection_event(
            &mut self.data.active_screen_region,
            &mut self.input_state,
            owner,
            (x, y),
        );
    }

    pub(in crate::backend::wayland) fn region_selection_geometry(
        &self,
    ) -> Option<RegionSelectionGeometry> {
        self.data
            .active_screen_region
            .and_then(ActiveScreenRegion::selection_geometry)
    }

    pub(in crate::backend::wayland) fn region_measure_selection(&self) -> Option<RegionSelection> {
        self.data
            .active_screen_region
            .and_then(ActiveScreenRegion::measure_selection)
    }

    pub(in crate::backend::wayland::state) fn region_picker_source_token(
        &self,
    ) -> Option<ScreenSourceToken> {
        match self.data.active_screen_region {
            Some(ActiveScreenRegion::Ready { source, .. }) => Some(source),
            Some(ActiveScreenRegion::Measure { .. })
            | Some(ActiveScreenRegion::PendingFrozen { .. })
            | Some(ActiveScreenRegion::PendingZoom { .. })
            | None => None,
        }
    }

    pub(in crate::backend::wayland) fn region_picker_legend_dismissed(&self) -> bool {
        self.data
            .active_screen_region
            .is_some_and(ActiveScreenRegion::legend_dismissed)
    }

    pub(in crate::backend::wayland) fn whole_image_region_selection(
        &self,
    ) -> Option<RegionSelectionFinalize> {
        self.data
            .active_screen_region
            .and_then(ActiveScreenRegion::whole_image_selection)
    }

    pub(in crate::backend::wayland) fn enter_region_review(
        &mut self,
        rect: ImagePixelRect,
    ) -> bool {
        let Some(region) = self.data.active_screen_region.as_mut() else {
            return false;
        };
        let purpose = region.purpose();
        let generation = region.generation();
        let Some(display) = region.enter_review(rect) else {
            return false;
        };
        self.input_state
            .activate_region_review(purpose, generation, display);
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
        let Some(display) = self
            .data
            .active_screen_region
            .as_mut()
            .and_then(|region| region.nudge_review(delta_x, delta_y))
        else {
            return false;
        };
        self.input_state.update_region_review_display(display);
        true
    }

    pub(in crate::backend::wayland) fn region_review_rect(&self) -> Option<ImagePixelRect> {
        match self.data.active_screen_region {
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
        self.data
            .active_screen_region
            .and_then(|region| region.picker_measurement(pointer))
    }

    /// Recompute capture geometry from compositor-authoritative Shift state.
    /// Individual key events remain swallowed by the modal selector.
    pub(in crate::backend::wayland) fn sync_region_square_modifier(&mut self, shift: bool) -> bool {
        sync_region_square_modifier_event(
            &mut self.data.active_screen_region,
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
        region_owner_lost_event(
            &mut self.data.active_screen_region,
            &mut self.input_state,
            source,
        )
    }

    pub(in crate::backend::wayland::state) fn debug_assert_screen_region_invariant(&self) {
        debug_assert!(screen_region_invariant(
            self.data.active_screen_region,
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
                    self.data.active_eyedropper_source,
                    &source_matches,
                ),
                active_region_source_changed(
                    self.data.active_screen_region,
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
                .data
                .active_screen_region
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
