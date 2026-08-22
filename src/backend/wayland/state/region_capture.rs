use crate::backend::wayland::acquisition::ScreenAcquisitionId;
use crate::input::state::{
    RegionInputSource, RegionPurposeTag, RegionSelectUiState, ScreenCaptureSource,
};
use crate::screen_pixels::{ImagePixelRect, ImagePoint, clamp_edge, pixel_span};

use super::WaylandState;
use super::screen_image::{
    ScreenSourceToken, current_screen_source_token, image_point_for_screen_point, screen_source_is,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FreezeOwnership {
    PreExisting,
    PickerOwned { image_generation: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ActiveScreenRegion {
    PendingFrozen {
        purpose: RegionPurposeTag,
        generation: u64,
        acquisition: ScreenAcquisitionId,
    },
    PendingZoom {
        purpose: RegionPurposeTag,
        generation: u64,
    },
    Ready {
        purpose: RegionPurposeTag,
        generation: u64,
        source: ScreenSourceToken,
        freeze_ownership: FreezeOwnership,
        anchor: Option<ImagePoint>,
        raw_edge: Option<ImagePoint>,
        logical_anchor: Option<(f64, f64)>,
        logical_edge: Option<(f64, f64)>,
    },
}

impl ActiveScreenRegion {
    pub const fn purpose(self) -> RegionPurposeTag {
        match self {
            Self::PendingFrozen { purpose, .. }
            | Self::PendingZoom { purpose, .. }
            | Self::Ready { purpose, .. } => purpose,
        }
    }

    pub const fn generation(self) -> u64 {
        match self {
            Self::PendingFrozen { generation, .. }
            | Self::PendingZoom { generation, .. }
            | Self::Ready { generation, .. } => generation,
        }
    }

    pub const fn pending_acquisition(self) -> Option<ScreenAcquisitionId> {
        match self {
            Self::PendingFrozen { acquisition, .. } => Some(acquisition),
            Self::PendingZoom { .. } | Self::Ready { .. } => None,
        }
    }

    pub fn waits_for_acquisition(self, id: ScreenAcquisitionId) -> bool {
        matches!(self, Self::PendingFrozen { acquisition, .. } if acquisition == id)
    }

    pub const fn owned_frozen_generation(self) -> Option<u64> {
        match self {
            Self::Ready {
                freeze_ownership: FreezeOwnership::PickerOwned { image_generation },
                ..
            } => Some(image_generation),
            Self::PendingFrozen { .. }
            | Self::PendingZoom { .. }
            | Self::Ready {
                freeze_ownership: FreezeOwnership::PreExisting,
                ..
            } => None,
        }
    }

    fn selection_rect(self) -> Option<ImagePixelRect> {
        let Self::Ready {
            purpose,
            anchor: Some(anchor),
            raw_edge: Some(raw_edge),
            logical_anchor,
            logical_edge,
            source,
            ..
        } = self
        else {
            return None;
        };
        if let Some(minimum) = purpose.selection_policy().min_submit_logical_px() {
            let (Some(logical_anchor), Some(logical_edge)) = (logical_anchor, logical_edge) else {
                return None;
            };
            if (logical_edge.0 - logical_anchor.0).abs() < minimum
                || (logical_edge.1 - logical_anchor.1).abs() < minimum
            {
                return None;
            }
        }
        pixel_span(anchor, raw_edge, source.image_size)?
            .try_into()
            .ok()
    }

    fn begin_selection(&mut self, logical: (f64, f64)) -> bool {
        let Self::Ready {
            purpose,
            source,
            anchor,
            raw_edge,
            logical_anchor,
            logical_edge,
            ..
        } = self
        else {
            return false;
        };
        if anchor.is_some()
            || raw_edge.is_some()
            || logical_anchor.is_some()
            || logical_edge.is_some()
        {
            return false;
        }
        let Some(mapped) = clamp_edge(
            image_point_for_screen_point(source, logical),
            source.image_size,
        ) else {
            return false;
        };
        debug_assert_eq!(*purpose, RegionPurposeTag::Ocr);
        *anchor = Some(mapped);
        *raw_edge = Some(mapped);
        *logical_anchor = Some(logical);
        *logical_edge = Some(logical);
        true
    }

    fn update_endpoint(&mut self, logical: (f64, f64)) -> bool {
        let Self::Ready {
            source,
            raw_edge,
            logical_edge,
            ..
        } = self
        else {
            return false;
        };
        let Some(mapped) = clamp_edge(
            image_point_for_screen_point(source, logical),
            source.image_size,
        ) else {
            return false;
        };
        *raw_edge = Some(mapped);
        *logical_edge = Some(logical);
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RegionSelectionFinalize {
    NotOwned,
    Rearmed,
    Selected(ImagePixelRect),
}

fn begin_region_selection_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    owner: RegionInputSource,
    logical: (f64, f64),
) -> bool {
    if !backend
        .as_mut()
        .is_some_and(|region| region.begin_selection(logical))
    {
        return false;
    }
    input_state.start_region_selection(owner, logical)
}

fn update_region_selection_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    owner: RegionInputSource,
    logical: (f64, f64),
) {
    if !input_state.region_selection_is_owned_by(owner) {
        return;
    }
    if backend
        .as_mut()
        .is_some_and(|region| region.update_endpoint(logical))
    {
        input_state.update_region_selection(owner, logical);
    }
}

fn rearm_region_selection_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
) {
    if let Some(ActiveScreenRegion::Ready {
        anchor,
        raw_edge,
        logical_anchor,
        logical_edge,
        ..
    }) = backend.as_mut()
    {
        *anchor = None;
        *raw_edge = None;
        *logical_anchor = None;
        *logical_edge = None;
    }
    input_state.rearm_region_selection();
}

pub(super) fn finalize_region_selection_event(
    backend: &mut Option<ActiveScreenRegion>,
    input_state: &mut crate::input::InputState,
    owner: RegionInputSource,
    logical: (f64, f64),
) -> RegionSelectionFinalize {
    if !input_state.region_selection_is_owned_by(owner) {
        return RegionSelectionFinalize::NotOwned;
    }
    update_region_selection_event(backend, input_state, owner, logical);
    let Some(rect) = backend
        .as_ref()
        .copied()
        .and_then(ActiveScreenRegion::selection_rect)
    else {
        rearm_region_selection_event(backend, input_state);
        return RegionSelectionFinalize::Rearmed;
    };
    RegionSelectionFinalize::Selected(rect)
}

impl WaylandState {
    pub(super) fn next_screen_region_generation(&mut self) -> u64 {
        let generation = self.data.next_screen_region_generation;
        self.data.next_screen_region_generation = generation
            .checked_add(1)
            .expect("screen region generation space exhausted");
        generation
    }

    pub(super) fn set_pending_screen_region(
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

    pub(super) fn activate_screen_region(
        &mut self,
        purpose: RegionPurposeTag,
        generation: u64,
        freeze_ownership: FreezeOwnership,
    ) -> bool {
        let Some(source) = super::screen_image::displayed_screen_image(
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
        });
        self.input_state.activate_region(purpose, generation);
        self.debug_assert_screen_region_invariant();
        true
    }

    pub(super) fn clear_screen_region_ui_only(&mut self) {
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

    pub(super) fn debug_assert_screen_region_invariant(&self) {
        debug_assert!(screen_region_invariant(
            self.data.active_screen_region,
            self.input_state.region_state(),
        ));
    }

    pub(in crate::backend::wayland) fn cancel_screen_modals_if_source_changed(&mut self) {
        let (cancel_eyedropper, cancel_ocr) = {
            let current_source = super::screen_image::displayed_screen_image(
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
                active_ocr_source_changed(self.data.active_screen_region, &source_matches),
            )
        };
        if cancel_eyedropper {
            self.cancel_eyedropper();
        }
        if cancel_ocr {
            self.cancel_ocr();
        }
    }
}

fn screen_region_invariant(backend: Option<ActiveScreenRegion>, ui: RegionSelectUiState) -> bool {
    match (backend, ui) {
        (None, RegionSelectUiState::Inactive) => true,
        (None, _) | (Some(_), RegionSelectUiState::Inactive) => false,
        (Some(region), ui) => {
            ui.generation() == Some(region.generation()) && ui.purpose() == Some(region.purpose())
        }
    }
}

fn active_ocr_source_changed(
    region: Option<ActiveScreenRegion>,
    source_matches: &impl Fn(ScreenSourceToken) -> bool,
) -> bool {
    matches!(
        region,
        Some(ActiveScreenRegion::Ready {
            purpose: RegionPurposeTag::Ocr,
            source,
            ..
        }) if !source_matches(source)
    )
}

fn active_eyedropper_source_changed(
    active: bool,
    expected_source: Option<ScreenSourceToken>,
    source_matches: &impl Fn(ScreenSourceToken) -> bool,
) -> bool {
    active && !expected_source.is_some_and(source_matches)
}

pub(super) fn owned_generation_is_current(
    expected: u64,
    current: u64,
    frozen_active: bool,
) -> bool {
    frozen_active && expected == current
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn pending_and_ready_region_state_preserve_generation_and_freeze_ownership() {
        let pending = ActiveScreenRegion::PendingZoom {
            purpose: RegionPurposeTag::Ocr,
            generation: 9,
        };
        assert_eq!(pending.generation(), 9);
        assert_eq!(pending.purpose(), RegionPurposeTag::Ocr);
        assert_eq!(pending.owned_frozen_generation(), None);

        let ready = ActiveScreenRegion::Ready {
            purpose: RegionPurposeTag::Ocr,
            generation: 9,
            source: ScreenSourceToken {
                output_id: 1,
                output_layout_generation: 0,
                kind: super::super::screen_image::ScreenImageKind::Frozen,
                image_generation: 44,
                image_size: (100, 80),
                stride: 400,
                surface: (100, 80),
                output_scale: 1,
                output_transform: wayland_client::protocol::wl_output::Transform::Normal,
                zoom_transformed: false,
                zoom_scale: 1.0,
                zoom_view_offset: (0.0, 0.0),
            },
            freeze_ownership: FreezeOwnership::PickerOwned {
                image_generation: 44,
            },
            anchor: None,
            raw_edge: None,
            logical_anchor: None,
            logical_edge: None,
        };
        assert_eq!(ready.generation(), pending.generation());
        assert_eq!(ready.owned_frozen_generation(), Some(44));
    }

    #[test]
    fn backend_and_ui_generation_must_match() {
        let backend = Some(ActiveScreenRegion::PendingZoom {
            purpose: RegionPurposeTag::Ocr,
            generation: 7,
        });
        assert!(screen_region_invariant(
            backend,
            RegionSelectUiState::PendingCapture {
                purpose: RegionPurposeTag::Ocr,
                generation: 7,
                source: ScreenCaptureSource::Zoom,
            }
        ));
        assert!(!screen_region_invariant(
            backend,
            RegionSelectUiState::PendingCapture {
                purpose: RegionPurposeTag::Ocr,
                generation: 8,
                source: ScreenCaptureSource::Zoom,
            }
        ));
        assert!(!screen_region_invariant(
            backend,
            RegionSelectUiState::Inactive
        ));
    }

    #[test]
    fn armed_freeze_release_is_generation_checked_and_consumed_once() {
        assert!(owned_generation_is_current(44, 44, true));
        assert!(!owned_generation_is_current(44, 45, true));
        assert!(!owned_generation_is_current(44, 44, false));

        let mut owned = ocr_region(1.0);
        if let ActiveScreenRegion::Ready {
            freeze_ownership, ..
        } = &mut owned
        {
            *freeze_ownership = FreezeOwnership::PickerOwned {
                image_generation: 44,
            };
        }
        let mut armed = Some(owned);
        let first = armed
            .take()
            .and_then(ActiveScreenRegion::owned_frozen_generation);
        let second = armed
            .take()
            .and_then(ActiveScreenRegion::owned_frozen_generation);
        assert_eq!(first, Some(44));
        assert_eq!(
            second, None,
            "a second cancellation has no region to release"
        );
    }

    fn ocr_region(scale: f64) -> ActiveScreenRegion {
        ActiveScreenRegion::Ready {
            purpose: RegionPurposeTag::Ocr,
            generation: 1,
            source: ScreenSourceToken {
                output_id: 1,
                output_layout_generation: 0,
                kind: super::super::screen_image::ScreenImageKind::Frozen,
                image_generation: 1,
                image_size: ((100.0 * scale) as u32, (80.0 * scale) as u32),
                stride: (400.0 * scale) as i32,
                surface: (100, 80),
                output_scale: 1,
                output_transform: wayland_client::protocol::wl_output::Transform::Normal,
                zoom_transformed: false,
                zoom_scale: 1.0,
                zoom_view_offset: (0.0, 0.0),
            },
            freeze_ownership: FreezeOwnership::PreExisting,
            anchor: None,
            raw_edge: None,
            logical_anchor: None,
            logical_edge: None,
        }
    }

    #[test]
    fn ready_ocr_detects_replaced_or_missing_source_but_pending_ocr_keeps_waiting() {
        let ready = ocr_region(1.0);
        let ActiveScreenRegion::Ready { source, .. } = ready else {
            unreachable!("OCR fixture is ready")
        };
        let mut replacement = source;
        replacement.image_generation += 1;

        assert!(!active_ocr_source_changed(Some(ready), &|expected| {
            expected == source
        }));
        assert!(active_ocr_source_changed(Some(ready), &|expected| {
            expected == replacement
        }));
        assert!(active_ocr_source_changed(Some(ready), &|_| false));
        assert!(!active_ocr_source_changed(
            Some(ActiveScreenRegion::PendingZoom {
                purpose: RegionPurposeTag::Ocr,
                generation: 1,
            }),
            &|_| false,
        ));
    }

    #[test]
    fn active_eyedropper_detects_replaced_missing_or_untracked_source() {
        let ActiveScreenRegion::Ready { source, .. } = ocr_region(1.0) else {
            unreachable!("OCR fixture is ready")
        };
        let mut replacement = source;
        replacement.image_generation += 1;

        assert!(!active_eyedropper_source_changed(
            true,
            Some(source),
            &|expected| expected == source,
        ));
        assert!(active_eyedropper_source_changed(
            true,
            Some(source),
            &|expected| expected == replacement,
        ));
        assert!(active_eyedropper_source_changed(
            true,
            Some(source),
            &|_| false,
        ));
        assert!(active_eyedropper_source_changed(true, None, &|_| true));
        assert!(!active_eyedropper_source_changed(
            false,
            Some(source),
            &|_| false,
        ));
    }

    #[test]
    fn ocr_three_logical_pixels_rearms_but_exactly_four_submits() {
        let mut region = ocr_region(1.0);
        assert!(region.begin_selection((10.0, 20.0)));
        assert!(region.update_endpoint((13.0, 23.0)));
        assert_eq!(region.selection_rect(), None);

        assert!(region.update_endpoint((14.0, 24.0)));
        assert_eq!(
            region
                .selection_rect()
                .map(|rect| (rect.x(), rect.y(), rect.size())),
            Some((10, 20, (4, 4)))
        );
    }

    #[test]
    fn ocr_release_endpoint_without_motion_matches_forward_and_reversed_scale_oracle() {
        for (scale, expected_forward, expected_reversed) in [
            (1.0, (10, 20, (5, 5)), (10, 20, (5, 5))),
            (2.0, (20, 41, (9, 8)), (20, 41, (9, 8))),
            (1.5, (15, 30, (7, 7)), (15, 30, (7, 7))),
        ] {
            let mut forward = ocr_region(scale);
            assert!(forward.begin_selection((10.25, 20.5)));
            assert!(forward.update_endpoint((14.25, 24.5)));
            let forward_rect = forward.selection_rect().unwrap();
            assert_eq!(
                (forward_rect.x(), forward_rect.y(), forward_rect.size()),
                expected_forward,
                "forward at scale {scale}"
            );

            let mut reversed = ocr_region(scale);
            assert!(reversed.begin_selection((14.25, 24.5)));
            assert!(reversed.update_endpoint((10.25, 20.5)));
            let reversed_rect = reversed.selection_rect().unwrap();
            assert_eq!(
                (reversed_rect.x(), reversed_rect.y(), reversed_rect.size()),
                expected_reversed,
                "reversed at scale {scale}"
            );
        }
    }

    #[test]
    fn ocr_release_replaces_the_last_motion_endpoint() {
        let mut region = ocr_region(1.0);
        assert!(region.begin_selection((10.0, 20.0)));
        assert!(region.update_endpoint((18.0, 32.0)));
        assert_eq!(region.selection_rect().unwrap().size(), (8, 12));

        assert!(region.update_endpoint((14.0, 24.0)));
        assert_eq!(region.selection_rect().unwrap().size(), (4, 4));
    }

    #[test]
    fn second_device_press_cannot_replace_an_in_progress_backend_region() {
        let mut region = ocr_region(1.0);
        assert!(region.begin_selection((10.0, 20.0)));
        assert!(region.update_endpoint((18.0, 28.0)));

        assert!(!region.begin_selection((50.0, 60.0)));
        assert_eq!(
            region
                .selection_rect()
                .map(|rect| (rect.x(), rect.y(), rect.size())),
            Some((10, 20, (8, 8)))
        );
    }

    #[test]
    fn ocr_policy_keeps_shift_held_drag_rectangular() {
        let mut region = ocr_region(1.0);
        assert!(region.begin_selection((10.0, 20.0)));
        assert!(region.update_endpoint((14.0, 28.0)));
        let rect = region.selection_rect().unwrap();
        assert_eq!(rect.size(), (4, 8));
        assert!(!RegionPurposeTag::Ocr.selection_policy().allow_square());
    }

    #[test]
    fn production_ocr_event_adapter_uses_release_endpoint_at_every_scale() {
        for (scale, expected) in [
            (1.0, (10, 20, (5, 5))),
            (2.0, (20, 41, (9, 8))),
            (1.5, (15, 30, (7, 7))),
        ] {
            for reversed in [false, true] {
                for has_motion in [false, true] {
                    let (press, release) = if reversed {
                        ((14.25, 24.5), (10.25, 20.5))
                    } else {
                        ((10.25, 20.5), (14.25, 24.5))
                    };
                    let mut backend = Some(ocr_region(scale));
                    let mut input = make_test_input_state();
                    input.activate_region(RegionPurposeTag::Ocr, 1);

                    assert!(begin_region_selection_event(
                        &mut backend,
                        &mut input,
                        RegionInputSource::Pointer,
                        press,
                    ));
                    if has_motion {
                        update_region_selection_event(
                            &mut backend,
                            &mut input,
                            RegionInputSource::Pointer,
                            (30.0, 35.0),
                        );
                    }
                    let RegionSelectionFinalize::Selected(rect) = finalize_region_selection_event(
                        &mut backend,
                        &mut input,
                        RegionInputSource::Pointer,
                        release,
                    ) else {
                        panic!(
                            "release must submit at scale={scale} reversed={reversed} motion={has_motion}"
                        );
                    };

                    assert_eq!(
                        (rect.x(), rect.y(), rect.size()),
                        expected,
                        "scale={scale} reversed={reversed} motion={has_motion}"
                    );
                    assert_eq!(
                        input
                            .region_state()
                            .selection()
                            .map(|selection| selection.end),
                        Some(release),
                        "the UI preview and crop adapter diverged"
                    );
                }
            }
        }
    }

    #[test]
    fn production_ocr_event_adapter_rearms_small_drag_and_ignores_shift_square_policy() {
        let mut backend = Some(ocr_region(1.0));
        let mut input = make_test_input_state();
        input.activate_region(RegionPurposeTag::Ocr, 1);
        input.sync_modifiers(true, false, false, false);

        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (10.0, 20.0),
        ));
        let RegionSelectionFinalize::Selected(rect) = finalize_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (14.0, 28.0),
        ) else {
            panic!("Shift-held OCR drag must submit");
        };
        assert_eq!(rect.size(), (4, 8));

        let mut backend = Some(ocr_region(1.0));
        let mut input = make_test_input_state();
        input.activate_region(RegionPurposeTag::Ocr, 1);
        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (10.0, 20.0),
        ));
        assert_eq!(
            finalize_region_selection_event(
                &mut backend,
                &mut input,
                RegionInputSource::Pointer,
                (13.0, 23.0),
            ),
            RegionSelectionFinalize::Rearmed
        );
        assert!(matches!(
            input.region_state(),
            RegionSelectUiState::Armed { .. }
        ));
    }
}
