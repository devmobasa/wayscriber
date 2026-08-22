use super::lifecycle_and_measure::ocr_region;
use super::*;

pub(super) fn capture_region() -> ActiveScreenRegion {
    capture_region_at_scale(1.0)
}

fn capture_region_with_drawings(include_drawings: bool) -> ActiveScreenRegion {
    let mut region = capture_region();
    if let ActiveScreenRegion::Ready {
        include_drawings: value,
        ..
    } = &mut region
    {
        *value = include_drawings;
    }
    region
}

pub(super) fn interactive_region() -> ActiveScreenRegion {
    let mut region = capture_region();
    if let ActiveScreenRegion::Ready { purpose, .. } = &mut region {
        *purpose = RegionPurposeTag::CaptureInteractive;
    }
    region
}

pub(super) fn capture_region_at_scale(scale: f64) -> ActiveScreenRegion {
    let ActiveScreenRegion::Ready {
        generation,
        source,
        freeze_ownership,
        ..
    } = ocr_region(scale)
    else {
        unreachable!("OCR fixture is ready")
    };
    ActiveScreenRegion::Ready {
        purpose: RegionPurposeTag::CaptureDeliver,
        generation,
        source,
        freeze_ownership,
        anchor: None,
        raw_edge: None,
        logical_anchor: None,
        logical_edge: None,
        square_modifier: false,
        legend_dismissed: false,
        include_drawings: false,
    }
}

#[test]
fn include_drawings_is_session_local_and_survives_review_reselection() {
    let mut region = capture_region_with_drawings(true);
    if let ActiveScreenRegion::Ready { purpose, .. } = &mut region {
        *purpose = RegionPurposeTag::CaptureInteractive;
    }
    assert!(region.include_drawings());
    assert_eq!(region.toggle_include_drawings(), Some(false));
    assert!(!region.include_drawings());

    region.reset_review_for_selection();
    assert!(!region.include_drawings());

    let mut direct = capture_region_with_drawings(true);
    assert_eq!(direct.toggle_include_drawings(), None);
    assert!(direct.include_drawings());
    assert!(!interactive_region().include_drawings());
}

#[test]
fn capture_press_snaps_its_anchor_and_starts_with_a_zero_pixel_span() {
    let mut region = capture_region();

    assert!(region.begin_selection((10.8, 20.6)));

    let geometry = region.selection_geometry().expect("fresh drag geometry");
    assert_eq!(geometry.image_span().size(), (0, 0));
    assert_eq!(geometry.display_selection().start, (10.0, 20.0));
    assert_eq!(geometry.display_selection().end, (10.0, 20.0));
    assert_eq!(geometry.image_rect(), None);
}

#[test]
fn interactive_release_enters_integer_authoritative_review() {
    let mut backend = Some(interactive_region());
    let mut input = make_test_input_state();
    input.activate_region(RegionPurposeTag::CaptureInteractive, 1);

    assert!(begin_region_selection_event(
        &mut backend,
        &mut input,
        RegionInputSource::Pointer,
        (10.2, 20.7),
    ));
    assert_eq!(
        finalize_region_selection_event(
            &mut backend,
            &mut input,
            RegionInputSource::Pointer,
            (30.1, 42.2),
        ),
        RegionSelectionFinalize::Reviewed
    );
    assert!(input.region_state().is_review());
    let rect = backend
        .and_then(ActiveScreenRegion::selection_rect)
        .expect("review owns a non-empty pixel rectangle");
    assert_eq!(
        (rect.x(), rect.y(), rect.width(), rect.height()),
        (10, 20, 21, 23)
    );
    assert_eq!(
        input.region_state().selection(),
        backend
            .and_then(ActiveScreenRegion::selection_geometry)
            .map(|geometry| geometry.display_selection())
    );
}
