use super::capture_selection::{capture_region, capture_region_at_scale, interactive_region};
use super::lifecycle_and_measure::ocr_region;
use super::*;

#[test]
fn capture_hover_motion_requests_a_repaint_while_armed_and_in_review() {
    let mut armed_backend = Some(capture_region());
    let mut armed_input = make_test_input_state();
    armed_input.activate_region(RegionPurposeTag::CaptureDeliver, 1);
    let _ = armed_input.dirty_tracker.take_region_report(100, 80);
    armed_input.needs_redraw = false;

    update_region_selection_event(
        &mut armed_backend,
        &mut armed_input,
        RegionInputSource::Pointer,
        (35.0, 45.0),
    );

    assert!(armed_input.needs_redraw, "armed crosshair follows hover");
    assert_eq!(
        armed_input
            .dirty_tracker
            .take_region_report(100, 80)
            .regions,
        vec![crate::util::Rect::new(0, 0, 100, 80).unwrap()]
    );

    let mut review_region = interactive_region();
    let rect = ImagePixelRect::new(20, 20, 30, 25, (100, 80)).unwrap();
    let display = review_region.enter_review(rect).unwrap();
    let mut review_backend = Some(review_region);
    let mut review_input = make_test_input_state();
    review_input.activate_region_review(RegionPurposeTag::CaptureInteractive, 1, display);
    let _ = review_input.dirty_tracker.take_region_report(100, 80);
    review_input.needs_redraw = false;

    update_region_selection_event(
        &mut review_backend,
        &mut review_input,
        RegionInputSource::Pointer,
        (70.0, 70.0),
    );

    assert!(
        review_input.needs_redraw,
        "review hover, action bar, and loupe follow the pointer"
    );
    assert_eq!(
        review_input
            .dirty_tracker
            .take_region_report(100, 80)
            .regions,
        vec![crate::util::Rect::new(0, 0, 100, 80).unwrap()]
    );
}

#[test]
fn review_nudge_and_move_clamp_without_resizing_and_owner_loss_keeps_review() {
    let mut region = interactive_region();
    let rect = ImagePixelRect::new(70, 60, 20, 15, (100, 80)).unwrap();
    let display = region.enter_review(rect).unwrap();
    let mut backend = Some(region);
    let mut input = make_test_input_state();
    input.activate_region_review(RegionPurposeTag::CaptureInteractive, 1, display);

    let nudged = backend
        .as_mut()
        .and_then(|region| region.nudge_review(50, 50))
        .unwrap();
    input.update_region_review_display(nudged);
    let clamped = backend
        .and_then(ActiveScreenRegion::selection_rect)
        .unwrap();
    assert_eq!(
        (clamped.x(), clamped.y(), clamped.size()),
        (80, 65, (20, 15))
    );

    assert!(begin_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Stylus,
        (85.0, 70.0),
    ));
    update_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Stylus,
        (20.0, 20.0),
    );
    assert_eq!(
        region_owner_lost_event(&mut backend, &mut input, RegionInputSource::Stylus),
        RegionOwnerLoss::Rearmed
    );
    assert!(input.region_state().is_review());
    assert!(input.region_state().selection_owner().is_none());
    assert!(
        backend
            .and_then(ActiveScreenRegion::selection_rect)
            .is_some()
    );
}

#[test]
fn review_move_preserves_subpixel_motion_until_it_reaches_a_pixel() {
    let mut region = interactive_region();
    let rect = ImagePixelRect::new(20, 20, 30, 25, (100, 80)).unwrap();
    region.enter_review(rect).unwrap();
    assert!(region.begin_review_move((25.0, 25.0)));

    for x in [25.6, 26.2, 26.8, 27.4, 28.0] {
        region.update_review_move((x, 25.0));
    }

    let moved = region.stored_review_rect().unwrap();
    assert_eq!(
        (moved.x(), moved.y(), moved.size()),
        (23, 20, (30, 25)),
        "three logical pixels of motion must move exactly three image pixels"
    );
}

#[test]
fn second_device_press_cannot_replace_an_in_progress_review_move() {
    let mut region = interactive_region();
    let rect = ImagePixelRect::new(20, 20, 30, 25, (100, 80)).unwrap();
    let display = region.enter_review(rect).unwrap();
    let mut backend = Some(region);
    let mut input = make_test_input_state();
    input.activate_region_review(RegionPurposeTag::CaptureInteractive, 1, display);

    assert!(begin_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Pointer,
        (25.0, 25.0),
    ));
    assert!(!begin_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Touch,
        (70.0, 70.0),
    ));

    assert!(input.region_state().is_review());
    assert!(input.region_selection_is_owned_by(RegionInputSource::Pointer));
    assert_eq!(
        backend.and_then(ActiveScreenRegion::selection_rect),
        Some(rect)
    );
}

#[test]
fn capture_pixel_span_reports_one_axis_empty_without_submitting_it() {
    let mut region = capture_region();
    assert!(region.begin_selection((10.8, 20.6)));
    assert!(region.update_endpoint((10.0, 40.0)));

    let geometry = region.selection_geometry().expect("drag geometry");
    assert_eq!(geometry.image_span().size(), (0, 20));
    assert_eq!(geometry.image_rect(), None);
    assert_eq!(region.selection_rect(), None);
}

#[test]
fn capture_finalize_is_purpose_aware_and_one_axis_empty_rearms() {
    let mut backend = Some(capture_region());
    let mut input = make_test_input_state();
    input.activate_region(RegionPurposeTag::CaptureDeliver, 1);
    assert!(begin_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Pointer,
        (10.8, 20.6),
    ));
    assert_eq!(
        finalize_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (18.2, 35.8),
        ),
        RegionSelectionFinalize::Selected {
            purpose: RegionPurposeTag::CaptureDeliver,
            rect: ImagePixelRect::new(10, 20, 9, 16, (100, 80)).unwrap(),
        }
    );

    let mut backend = Some(capture_region());
    let mut input = make_test_input_state();
    input.activate_region(RegionPurposeTag::CaptureDeliver, 1);
    assert!(begin_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Touch,
        (10.8, 20.6),
    ));
    assert_eq!(
        finalize_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Touch,
            (10.0, 40.0),
        ),
        RegionSelectionFinalize::Rearmed
    );
    assert!(matches!(
        input.region_state(),
        RegionSelectUiState::Armed {
            purpose: RegionPurposeTag::CaptureDeliver,
            generation: 1,
        }
    ));
    assert_eq!(backend.unwrap().selection_geometry(), None);
}

#[test]
fn capture_shift_square_uses_the_dominant_image_axis_and_restores_the_raw_edge() {
    let mut region = capture_region();
    assert!(region.begin_selection((10.8, 20.6)));
    assert!(region.update_endpoint((18.2, 35.8)));
    assert_eq!(
        region.selection_geometry().unwrap().image_span().size(),
        (9, 16)
    );

    assert!(region.set_square_modifier(true));
    assert_eq!(
        region.selection_geometry().unwrap().image_span().size(),
        (16, 16)
    );

    assert!(region.set_square_modifier(false));
    assert_eq!(
        region.selection_geometry().unwrap().image_span().size(),
        (9, 16),
        "releasing Shift must recompute from the canonical raw edge"
    );
}

#[test]
fn capture_shift_square_caps_the_side_to_the_first_image_edge() {
    let mut region = capture_region();
    assert!(region.begin_selection((90.8, 70.4)));
    assert!(region.update_endpoint((10.0, 40.0)));
    assert!(region.set_square_modifier(true));

    let geometry = region.selection_geometry().unwrap();
    assert_eq!(geometry.image_span().size(), (70, 70));
    assert_eq!(geometry.image_span().x(), 20);
    assert_eq!(geometry.image_span().y(), 0);
}

#[test]
fn capture_square_and_readout_share_image_pixels_at_integer_fractional_and_zoom_views() {
    let mut cases = [
        ("scale-1", capture_region_at_scale(1.0)),
        ("scale-2", capture_region_at_scale(2.0)),
        ("fractional-1.5", capture_region_at_scale(1.5)),
        ("zoom-2", capture_region_at_scale(1.0)),
    ];
    if let ActiveScreenRegion::Ready { source, .. } = &mut cases[3].1 {
        source.zoom_transformed = true;
        source.zoom_scale = 2.0;
        source.zoom_view_offset = (10.0, 5.0);
    }

    for (name, mut region) in cases {
        assert!(region.begin_selection((20.25, 20.5)), "{name}");
        assert!(region.update_endpoint((40.25, 50.5)), "{name}");
        assert!(region.set_square_modifier(true), "{name}");

        let geometry = region.selection_geometry().expect("square geometry");
        let (width, height) = geometry.image_span().size();
        assert_eq!(
            width, height,
            "square must be square in image pixels: {name}"
        );
        assert!(width > 0, "test drag must cover pixels: {name}");
        assert_eq!(
            region.picker_measurement((0.0, 0.0)),
            Some(RegionPickerMeasurement::Size { width, height }),
            "readout must describe the exact square crop: {name}"
        );
        assert_eq!(
            geometry.image_rect().map(ImagePixelRect::size),
            Some((width, height)),
            "submitted crop must share the readout's pixel span: {name}"
        );
    }
}

#[test]
fn capture_measurement_maps_armed_pointer_and_reports_exact_selecting_span() {
    let mut region = capture_region_at_scale(2.0);
    assert_eq!(
        region.picker_measurement((10.25, 20.5)),
        Some(RegionPickerMeasurement::Point { x: 20, y: 41 })
    );

    assert!(region.begin_selection((10.25, 20.5)));
    assert_eq!(
        region.picker_measurement((999.0, 999.0)),
        Some(RegionPickerMeasurement::Size {
            width: 0,
            height: 0,
        })
    );
    assert!(region.update_endpoint((14.25, 24.5)));
    assert_eq!(
        region.picker_measurement((0.0, 0.0)),
        Some(RegionPickerMeasurement::Size {
            width: 9,
            height: 8,
        })
    );
    assert_eq!(ocr_region(2.0).picker_measurement((10.25, 20.5)), None);
}

#[test]
fn compositor_shift_sync_recomputes_capture_preview_without_changing_ownership() {
    let mut backend = Some(capture_region());
    let mut input = make_test_input_state();
    input.activate_region(RegionPurposeTag::CaptureDeliver, 1);

    assert!(sync_region_square_modifier_event(
        &mut backend,
        &mut input,
        true,
    ));
    assert!(begin_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Touch,
        (10.8, 20.6),
    ));
    update_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Touch,
        (18.2, 35.8),
    );
    let square = backend.unwrap().selection_geometry().unwrap();
    assert_eq!(square.image_span().size(), (16, 16));
    assert_eq!(
        input.region_state().selection().unwrap(),
        square.display_selection()
    );
    assert!(input.region_selection_is_owned_by(RegionInputSource::Touch));

    assert!(sync_region_square_modifier_event(
        &mut backend,
        &mut input,
        false,
    ));
    let raw = backend.unwrap().selection_geometry().unwrap();
    assert_eq!(raw.image_span().size(), (9, 16));
    assert_eq!(
        input.region_state().selection().unwrap(),
        raw.display_selection()
    );
    assert!(input.region_selection_is_owned_by(RegionInputSource::Touch));
}

#[test]
fn capture_owner_loss_rearms_without_releasing_backend_ownership() {
    let mut backend = Some(capture_region());
    let mut input = make_test_input_state();
    input.activate_region(RegionPurposeTag::CaptureDeliver, 1);
    assert!(begin_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Stylus,
        (10.0, 20.0),
    ));
    assert!(
        backend.is_some_and(ActiveScreenRegion::legend_dismissed),
        "the first press permanently dismisses this picker's legend"
    );
    update_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Stylus,
        (30.0, 40.0),
    );

    assert_eq!(
        region_owner_lost_event(&mut backend, &mut input, RegionInputSource::Touch),
        RegionOwnerLoss::NotOwned
    );
    assert!(input.region_selection_is_owned_by(RegionInputSource::Stylus));

    assert_eq!(
        region_owner_lost_event(&mut backend, &mut input, RegionInputSource::Stylus),
        RegionOwnerLoss::Rearmed
    );
    assert!(matches!(
        input.region_state(),
        RegionSelectUiState::Armed {
            purpose: RegionPurposeTag::CaptureDeliver,
            generation: 1,
        }
    ));
    assert!(backend.is_some(), "capture source ownership was released");
    let backend = backend.unwrap();
    assert_eq!(backend.selection_geometry(), None);
    assert!(
        backend.legend_dismissed(),
        "rearming must not show the first-use legend again"
    );
}

#[test]
fn ocr_owner_loss_requests_its_existing_terminal_cancel_path() {
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
        region_owner_lost_event(&mut backend, &mut input, RegionInputSource::Pointer),
        RegionOwnerLoss::Cancel(RegionPurposeTag::Ocr)
    );
    assert!(
        input.region_selection_is_owned_by(RegionInputSource::Pointer),
        "the lifecycle owner must perform OCR cleanup"
    );
}

#[test]
fn whole_image_is_available_only_to_capture_purposes() {
    let capture = capture_region();
    let RegionSelectionFinalize::Selected { purpose, rect } = capture
        .whole_image_selection()
        .expect("capture whole image")
    else {
        panic!("whole image must be a selected result")
    };
    assert_eq!(purpose, RegionPurposeTag::CaptureDeliver);
    assert_eq!((rect.x(), rect.y(), rect.size()), (0, 0, (100, 80)));
    assert_eq!(ocr_region(1.0).whole_image_selection(), None);
}
