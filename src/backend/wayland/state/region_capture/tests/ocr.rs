use super::lifecycle_and_measure::ocr_region;
use super::*;

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
fn held_shift_seeds_capture_but_not_ocr_before_the_first_press() {
    assert!(initial_square_modifier(
        RegionPurposeTag::CaptureDeliver,
        true
    ));
    assert!(initial_square_modifier(
        RegionPurposeTag::CaptureInteractive,
        true
    ));
    assert!(!initial_square_modifier(RegionPurposeTag::Ocr, true));
    assert!(!initial_square_modifier(
        RegionPurposeTag::CaptureDeliver,
        false
    ));
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
                let RegionSelectionFinalize::Selected {
                    purpose: RegionPurposeTag::Ocr,
                    rect,
                } = finalize_region_selection_event(
                    &mut backend,
                    &mut input,
                    RegionInputSource::Pointer,
                    release,
                )
                else {
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
    let RegionSelectionFinalize::Selected {
        purpose: RegionPurposeTag::Ocr,
        rect,
    } = finalize_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Pointer,
        (14.0, 28.0),
    )
    else {
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
