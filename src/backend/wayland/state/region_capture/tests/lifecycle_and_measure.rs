use super::capture_selection::capture_region;
use super::*;

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
            kind: crate::backend::wayland::state::screen_image::ScreenImageKind::Frozen,
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
        square_modifier: false,
        legend_dismissed: false,
        include_drawings: false,
        review_resize: None,
    };
    assert_eq!(ready.generation(), pending.generation());
    assert_eq!(ready.owned_frozen_generation(), Some(44));
}

#[test]
fn measure_mode_owns_logical_geometry_without_a_screen_image() {
    let mut backend = Some(ActiveScreenRegion::Measure {
        generation: 7,
        bounds: (800, 600),
        anchor: None,
        edge: None,
    });
    let mut input = make_test_input_state();
    input.activate_measure_mode(7);

    assert!(begin_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Pointer,
        (10.2, 20.7),
    ));
    update_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Pointer,
        (35.2, 60.1),
    );
    assert_eq!(
        finalize_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (35.2, 60.1),
        ),
        RegionSelectionFinalize::Measured
    );

    let expected = RegionSelection {
        start: (10.0, 20.0),
        end: (36.0, 61.0),
    };
    assert_eq!(input.region_state().selection(), Some(expected));
    assert!(input.region_state().is_measured());
    assert_eq!(
        backend.and_then(ActiveScreenRegion::measure_selection),
        Some(expected)
    );
    assert!(screen_region_invariant(backend, input.region_state()));
    assert_eq!(
        backend.and_then(|region| region.picker_measurement((0.0, 0.0))),
        Some(RegionPickerMeasurement::Size {
            width: 26,
            height: 41,
        })
    );
}

#[test]
fn measure_mode_action_toggles_itself_and_refuses_other_modals() {
    assert_eq!(
        measure_mode_transition(None, false),
        MeasureModeTransition::Start
    );
    assert_eq!(
        measure_mode_transition(Some(RegionPurposeTag::Measure), true),
        MeasureModeTransition::Cancel
    );
    for purpose in [
        RegionPurposeTag::Ocr,
        RegionPurposeTag::CaptureDeliver,
        RegionPurposeTag::CaptureInteractive,
    ] {
        assert_eq!(
            measure_mode_transition(Some(purpose), true),
            MeasureModeTransition::Refuse
        );
    }
}

#[test]
fn lost_measure_drag_rearms_for_another_device() {
    let mut backend = Some(ActiveScreenRegion::Measure {
        generation: 8,
        bounds: (800, 600),
        anchor: None,
        edge: None,
    });
    let mut input = make_test_input_state();
    input.activate_measure_mode(8);
    assert!(begin_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Stylus,
        (12.0, 14.0),
    ));

    assert_eq!(
        region_owner_lost_event(&mut backend, &mut input, RegionInputSource::Stylus),
        RegionOwnerLoss::Rearmed
    );
    assert!(matches!(
        input.region_state(),
        RegionSelectUiState::Armed { .. }
    ));
    assert!(
        backend
            .and_then(ActiveScreenRegion::measure_selection)
            .is_none()
    );
}

#[test]
fn reversed_measure_drag_uses_outward_integer_edges() {
    let mut backend = Some(ActiveScreenRegion::Measure {
        generation: 9,
        bounds: (800, 600),
        anchor: None,
        edge: None,
    });
    let mut input = make_test_input_state();
    input.activate_measure_mode(9);
    assert!(begin_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Touch,
        (50.8, 60.2),
    ));
    update_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Touch,
        (9.8, 19.9),
    );

    assert_eq!(
        input.region_state().selection(),
        Some(RegionSelection {
            start: (50.0, 60.0),
            end: (9.0, 19.0),
        })
    );
    assert_eq!(
        backend.and_then(|region| region.picker_measurement((0.0, 0.0))),
        Some(RegionPickerMeasurement::Size {
            width: 41,
            height: 41,
        })
    );
}

#[test]
fn measure_points_and_edges_clamp_to_the_logical_surface() {
    assert_eq!(
        measure_anchor((-20.0, 900.0), (800, 600)),
        Some((0.0, 599.0))
    );
    assert_eq!(
        measure_edge((0.0, 599.0), (900.0, 900.0), (800, 600)),
        Some((800.0, 600.0))
    );
    assert_eq!(measure_anchor((1.0, 1.0), (0, 600)), None);
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

pub(super) fn ocr_region(scale: f64) -> ActiveScreenRegion {
    ActiveScreenRegion::Ready {
        purpose: RegionPurposeTag::Ocr,
        generation: 1,
        source: ScreenSourceToken {
            output_id: 1,
            output_layout_generation: 0,
            kind: crate::backend::wayland::state::screen_image::ScreenImageKind::Frozen,
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
        square_modifier: false,
        legend_dismissed: false,
        include_drawings: false,
        review_resize: None,
    }
}

#[test]
fn ready_regions_detect_replaced_or_missing_source_but_pending_regions_keep_waiting() {
    let ready = ocr_region(1.0);
    let ActiveScreenRegion::Ready { source, .. } = ready else {
        unreachable!("OCR fixture is ready")
    };
    let mut replacement = source;
    replacement.image_generation += 1;

    assert!(!active_region_source_changed(
        Some(ready),
        (100, 80),
        &|expected| { expected == source }
    ));
    assert!(active_region_source_changed(
        Some(ready),
        (100, 80),
        &|expected| { expected == replacement }
    ));
    assert!(active_region_source_changed(
        Some(ready),
        (100, 80),
        &|_| false
    ));
    assert!(!active_region_source_changed(
        Some(ActiveScreenRegion::PendingZoom {
            purpose: RegionPurposeTag::Ocr,
            generation: 1,
        }),
        (100, 80),
        &|_| false,
    ));

    let capture = capture_region();
    assert!(active_region_source_changed(
        Some(capture),
        (100, 80),
        &|_| false
    ));
}

#[test]
fn measure_detects_a_real_surface_resize_without_needing_a_screen_source() {
    let measure = ActiveScreenRegion::Measure {
        generation: 1,
        bounds: (100, 80),
        anchor: Some((10.0, 20.0)),
        edge: Some((30.0, 40.0)),
    };

    assert!(!active_region_source_changed(
        Some(measure),
        (100, 80),
        &|_| false,
    ));
    assert!(active_region_source_changed(
        Some(measure),
        (101, 80),
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
