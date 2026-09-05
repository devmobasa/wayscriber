use super::capture_selection::{capture_region, interactive_region};
use super::lifecycle_and_measure::ocr_region;
use super::*;

fn active_region_for(purpose: RegionPurposeTag) -> ActiveScreenRegion {
    match purpose {
        RegionPurposeTag::Ocr => ocr_region(1.0),
        RegionPurposeTag::CaptureDeliver => capture_region(),
        RegionPurposeTag::CaptureInteractive => interactive_region(),
        RegionPurposeTag::Measure => ActiveScreenRegion::Measure {
            generation: 1,
            bounds: (100, 80),
            anchor: None,
            edge: None,
        },
    }
}

#[test]
fn each_purpose_keeps_its_event_geometry_and_terminal_ownership_contract() {
    for purpose in [
        RegionPurposeTag::Ocr,
        RegionPurposeTag::CaptureDeliver,
        RegionPurposeTag::CaptureInteractive,
        RegionPurposeTag::Measure,
    ] {
        let mut backend = Some(active_region_for(purpose));
        let mut input = make_test_input_state();
        if purpose == RegionPurposeTag::Measure {
            input.activate_measure_mode(1);
        } else {
            input.activate_region(purpose, 1);
        }
        assert_eq!(
            input.region_state(),
            RegionSelectUiState::Armed {
                purpose,
                generation: 1
            }
        );
        assert!(begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (10.0, 20.0)
        ));
        assert_eq!(
            input.region_state(),
            RegionSelectUiState::Selecting {
                purpose,
                generation: 1,
                owner: RegionInputSource::Pointer,
                start: (10.0, 20.0),
                current: (10.0, 20.0),
            }
        );
        let assert_mirror = |backend: Option<ActiveScreenRegion>, ui: RegionSelectUiState| {
            let active = backend.expect("active event backend");
            assert_eq!(active.purpose(), purpose);
            assert_eq!(active.generation(), 1);
            assert_eq!(active.display_selection(), ui.selection());
        };
        assert_mirror(backend, input.region_state());
        update_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (25.0, 35.0),
        );
        assert_mirror(backend, input.region_state());
        let moving = input.region_state();
        let active = backend;
        assert_eq!(
            moving.selection(),
            Some(RegionSelection {
                start: (10.0, 20.0),
                end: (25.0, 35.0)
            })
        );
        assert!(!begin_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Touch,
            (70.0, 70.0)
        ));
        update_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Touch,
            (80.0, 75.0),
        );
        assert_eq!(
            finalize_region_selection_with_review_edits(
                &mut backend,
                &mut input,
                &mut None,
                RegionInputSource::Touch,
                (90.0, 79.0)
            ),
            RegionSelectionFinalize::NotOwned
        );
        assert_eq!(input.region_state(), moving);
        assert_eq!(backend, active);

        let result = finalize_region_selection_with_review_edits(
            &mut backend,
            &mut input,
            &mut None,
            RegionInputSource::Pointer,
            (30.0, 40.0),
        );
        let display = RegionSelection {
            start: (10.0, 20.0),
            end: (30.0, 40.0),
        };
        let expected_ui = match purpose {
            RegionPurposeTag::Measure => {
                assert_eq!(result, RegionSelectionFinalize::Measured);
                assert_eq!(
                    backend.and_then(ActiveScreenRegion::measure_selection),
                    Some(display)
                );
                RegionSelectUiState::Measured {
                    purpose,
                    generation: 1,
                    display,
                }
            }
            RegionPurposeTag::CaptureInteractive => {
                assert_eq!(result, RegionSelectionFinalize::Reviewed);
                RegionSelectUiState::Review {
                    purpose,
                    generation: 1,
                    display,
                    move_owner: None,
                }
            }
            RegionPurposeTag::Ocr | RegionPurposeTag::CaptureDeliver => {
                assert_eq!(
                    result,
                    RegionSelectionFinalize::Selected {
                        purpose,
                        rect: ImagePixelRect::new(10, 20, 20, 20, (100, 80)).unwrap(),
                    }
                );
                // Delivery consumes this selection after the event adapter returns.
                RegionSelectUiState::Selecting {
                    purpose,
                    generation: 1,
                    owner: RegionInputSource::Pointer,
                    start: display.start,
                    current: display.end,
                }
            }
        };
        assert_eq!(input.region_state(), expected_ui);
        assert_mirror(backend, input.region_state());
        if purpose != RegionPurposeTag::Measure {
            assert_eq!(
                backend.and_then(ActiveScreenRegion::selection_rect),
                ImagePixelRect::new(10, 20, 20, 20, (100, 80))
            );
        }
        assert!(screen_region_invariant(backend, input.region_state()));
    }
}
