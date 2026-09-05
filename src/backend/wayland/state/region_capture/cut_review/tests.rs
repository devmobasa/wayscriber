use super::actions::apply_cut_history_change;
use super::geometry::{dominant_cut_axis, logical_to_output_point, output_display_for};
use super::model::{CutCommit, CutMode};
use super::*;
use crate::backend::wayland::state::screen_image::ScreenImageKind;
use crate::backend::wayland::state::screen_image::ScreenSourceToken;
use crate::capture::CutAxis;
use crate::capture::CutBand;
use crate::input::state::{RegionInputSource, RegionSelection};
use crate::screen_pixels::ImagePixelRect;
use wayland_client::protocol::wl_output::Transform;

fn token() -> ScreenSourceToken {
    ScreenSourceToken {
        output_id: 1,
        output_layout_generation: 1,
        kind: ScreenImageKind::Frozen,
        image_generation: 1,
        image_size: (8, 8),
        stride: 32,
        surface: (8, 8),
        output_scale: 1,
        output_transform: Transform::Normal,
        zoom_transformed: false,
        zoom_scale: 1.0,
        zoom_view_offset: (0.0, 0.0),
    }
}

fn fingerprint(rect: ImagePixelRect) -> RegionRenderFingerprint {
    RegionRenderFingerprint::Raw {
        correlation: RegionReviewCorrelation {
            generation: 1,
            source: token(),
        },
        source_rect: rect,
    }
}

fn edits() -> RegionReviewEdits {
    let rect = ImagePixelRect::new(0, 0, 8, 8, (8, 8)).unwrap();
    RegionReviewEdits::new(
        RegionReviewCorrelation {
            generation: 1,
            source: token(),
        },
        rect,
    )
}

fn display() -> RegionSelection {
    RegionSelection {
        start: (0.0, 0.0),
        end: (8.0, 8.0),
    }
}

#[test]
fn arming_does_not_change_history() {
    let mut edits = edits();
    edits.toggle_mode();
    assert_eq!(edits.mode, CutMode::Armed);
    assert!(edits.cuts.is_empty());
    edits.toggle_mode();
    assert_eq!(edits.mode, CutMode::Idle);
    assert!(edits.cuts.is_empty());
}

#[test]
fn sub_threshold_drag_commits_nothing() {
    let mut edits = edits();
    edits.toggle_mode();
    assert!(edits.begin_drag(RegionInputSource::Pointer, (1.0, 1.0)));
    assert!(edits.update_drag(RegionInputSource::Pointer, (3.0, 1.0)));
    assert_eq!(
        edits.finish_drag(
            RegionInputSource::Pointer,
            (3.0, 1.0),
            display(),
            fingerprint(edits.source_rect)
        ),
        CutCommit::None
    );
    assert!(edits.cuts.is_empty());
}

#[test]
fn axis_locks_once_past_the_threshold() {
    let mut edits = edits();
    edits.toggle_mode();
    assert!(edits.begin_drag(RegionInputSource::Pointer, (0.0, 0.0)));
    assert!(edits.update_drag(RegionInputSource::Pointer, (6.0, 1.0)));
    assert_eq!(edits.drag.unwrap().axis, Some(CutAxis::Columns));
    assert!(edits.update_drag(RegionInputSource::Pointer, (6.0, 20.0)));
    assert_eq!(edits.drag.unwrap().axis, Some(CutAxis::Columns));
}

#[test]
fn wrong_owner_cannot_update_or_finish_a_drag() {
    let mut edits = edits();
    edits.toggle_mode();
    assert!(edits.begin_drag(RegionInputSource::Pointer, (0.0, 0.0)));
    assert!(!edits.update_drag(RegionInputSource::Touch, (6.0, 0.0)));
    assert_eq!(
        edits.finish_drag(
            RegionInputSource::Touch,
            (6.0, 0.0),
            display(),
            fingerprint(edits.source_rect)
        ),
        CutCommit::None
    );
    assert!(edits.drag.is_some());
}

#[test]
fn valid_commit_appends_clears_redo_and_increments_revision() {
    let mut edits = edits();
    edits.toggle_mode();
    assert!(edits.begin_drag(RegionInputSource::Pointer, (2.0, 0.0)));
    assert!(edits.update_drag(RegionInputSource::Pointer, (7.0, 0.0)));
    assert_eq!(
        edits.finish_drag(
            RegionInputSource::Pointer,
            (7.0, 0.0),
            display(),
            fingerprint(edits.source_rect)
        ),
        CutCommit::Applied
    );
    assert_eq!(edits.cuts.len(), 1);
    assert!(edits.redo.is_empty());
    assert_eq!(edits.revision, 1);
    assert!(!edits.preview_is_current());
}

#[test]
fn full_axis_commit_is_rejected_without_a_revision_change() {
    let mut edits = edits();
    edits.toggle_mode();
    assert!(edits.begin_drag(RegionInputSource::Pointer, (0.0, 0.0)));
    assert!(edits.update_drag(RegionInputSource::Pointer, (8.0, 0.0)));
    assert_eq!(
        edits.finish_drag(
            RegionInputSource::Pointer,
            (8.0, 0.0),
            display(),
            fingerprint(edits.source_rect)
        ),
        CutCommit::RejectedFullAxis
    );
    assert!(edits.cuts.is_empty());
    assert_eq!(edits.revision, 0);
}

#[test]
fn undo_redo_and_new_commit_clear_redo() {
    let mut edits = edits();
    let fingerprint = fingerprint(edits.source_rect);
    edits
        .cuts
        .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
    edits.revision = 1;
    assert!(edits.undo(fingerprint.clone()));
    assert!(edits.cuts.is_empty());
    assert_eq!(edits.redo.len(), 1);
    assert!(edits.redo(fingerprint.clone()));
    assert_eq!(edits.cuts.len(), 1);
    assert!(edits.undo(fingerprint.clone()));
    edits.toggle_mode();
    assert!(edits.begin_drag(RegionInputSource::Pointer, (2.0, 0.0)));
    assert!(edits.update_drag(RegionInputSource::Pointer, (7.0, 0.0)));
    assert_eq!(
        edits.finish_drag(
            RegionInputSource::Pointer,
            (7.0, 0.0),
            display(),
            fingerprint
        ),
        CutCommit::Applied
    );
    assert!(edits.redo.is_empty());
}

#[test]
fn undo_and_redo_abandon_an_in_flight_drag() {
    let mut edits = edits();
    let fingerprint = fingerprint(edits.source_rect);
    edits
        .cuts
        .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
    edits.revision = 1;
    edits.set_desired_from(fingerprint.clone());
    let desired = edits.desired_preview.clone().unwrap();
    edits.ready_preview = Some(RegionCutPreview {
        key: desired,
        pixels: std::sync::Arc::new(
            crate::screen_pixels::PackedArgb32::new(7, 8, 28, vec![0; 28 * 8]).unwrap(),
        ),
        display: display(),
    });
    edits.toggle_mode();
    assert!(edits.begin_drag(RegionInputSource::Pointer, (1.0, 1.0)));
    assert!(edits.undo(fingerprint.clone()));
    assert!(edits.drag.is_none());
    assert_eq!(
        edits.finish_drag(
            RegionInputSource::Pointer,
            (7.0, 0.0),
            display(),
            fingerprint.clone()
        ),
        CutCommit::None
    );

    assert!(edits.begin_drag(RegionInputSource::Pointer, (1.0, 1.0)));
    assert!(edits.redo(fingerprint.clone()));
    assert!(edits.drag.is_none());
    assert_eq!(
        edits.finish_drag(
            RegionInputSource::Pointer,
            (7.0, 0.0),
            display(),
            fingerprint
        ),
        CutCommit::None
    );
}

#[test]
fn undo_with_nothing_to_undo_leaves_an_in_flight_drag() {
    let mut edits = edits();
    edits.toggle_mode();
    assert!(edits.begin_drag(RegionInputSource::Pointer, (1.0, 1.0)));
    assert!(!edits.undo(fingerprint(edits.source_rect)));
    assert!(edits.drag.is_some());
}

fn edits_with_current_preview() -> RegionReviewEdits {
    let mut edits = edits();
    let fingerprint = fingerprint(edits.source_rect);
    edits
        .cuts
        .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
    edits.revision = 1;
    edits.set_desired_from(fingerprint);
    let desired = edits.desired_preview.clone().unwrap();
    edits.ready_preview = Some(RegionCutPreview {
        key: desired,
        pixels: std::sync::Arc::new(
            crate::screen_pixels::PackedArgb32::new(7, 8, 28, vec![0; 28 * 8]).unwrap(),
        ),
        display: display(),
    });
    edits
}

#[test]
fn undo_and_redo_retire_pointer_touch_and_tablet_cut_drags_before_release() {
    for owner in [
        RegionInputSource::Pointer,
        RegionInputSource::Touch,
        RegionInputSource::Stylus,
    ] {
        let mut edits = Some(edits_with_current_preview());
        edits.as_mut().unwrap().toggle_mode();
        assert!(edits.as_mut().unwrap().begin_drag(owner, (1.0, 1.0)));

        let fingerprint = fingerprint(edits.as_ref().unwrap().source_rect);
        assert!(apply_cut_history_change(&mut edits, |edits| {
            edits.undo(fingerprint.clone())
        }));
        assert!(edits.as_ref().unwrap().drag.is_none());
        assert_eq!(
            edits
                .as_mut()
                .unwrap()
                .finish_drag(owner, (7.0, 0.0), display(), fingerprint.clone()),
            CutCommit::None,
            "{owner:?} release must not commit after undo"
        );

        assert!(edits.as_mut().unwrap().begin_drag(owner, (1.0, 1.0)));
        assert!(apply_cut_history_change(&mut edits, |edits| {
            edits.redo(fingerprint.clone())
        }));
        assert!(edits.as_ref().unwrap().drag.is_none());
        assert_eq!(
            edits
                .as_mut()
                .unwrap()
                .finish_drag(owner, (7.0, 0.0), display(), fingerprint),
            CutCommit::None
        );
    }
}

#[test]
fn toggling_cut_mode_off_during_a_drag_returns_the_owner() {
    let mut edits = edits();
    edits.toggle_mode();
    assert!(edits.begin_drag(RegionInputSource::Pointer, (1.0, 1.0)));
    assert_eq!(edits.toggle_mode(), Some(RegionInputSource::Pointer));
    assert_eq!(edits.mode, CutMode::Idle);
    assert!(edits.drag.is_none());
}

#[test]
fn revision_exhaustion_leaves_reset_and_invalidate_untouched() {
    let mut edits = edits();
    let cut = CutBand::new(CutAxis::Columns, 1, 2).unwrap();
    edits.cuts.push(cut);
    edits.revision = u64::MAX;
    edits.failed_revision = Some(u64::MAX);
    assert!(!edits.reset());
    assert_eq!(edits.cuts, [cut]);
    assert_eq!(edits.failed_revision, Some(u64::MAX));

    let fingerprint = fingerprint(edits.source_rect);
    edits.invalidate_base(fingerprint);
    assert_eq!(edits.cuts, [cut]);
    assert_eq!(edits.failed_revision, Some(u64::MAX));
    assert_eq!(edits.revision, u64::MAX);
}

#[test]
fn undoing_the_last_cut_unlocks_the_crop() {
    let mut edits = edits();
    let fingerprint = fingerprint(edits.source_rect);
    edits
        .cuts
        .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
    edits.revision = 1;
    assert!(edits.crop_locked());
    assert!(edits.undo(fingerprint));
    assert!(!edits.crop_locked());
}

#[test]
fn a_failed_current_preview_stays_failed_until_the_revision_changes() {
    let mut edits = edits();
    let desired = CutPreviewKey {
        fingerprint: fingerprint(edits.source_rect),
        revision: 3,
        cuts: vec![CutBand::new(CutAxis::Columns, 1, 2).unwrap()],
    };
    edits.cuts = desired.cuts.clone();
    edits.revision = 3;
    edits.desired_preview = Some(desired.clone());
    assert!(edits.mark_preview_failed(&desired));
    assert!(edits.current_preview_failed());
    edits.failed_revision = None;
    edits.revision = 4;
    edits.desired_preview = Some(CutPreviewKey {
        revision: 4,
        ..desired
    });
    assert!(!edits.current_preview_failed());
}

#[test]
fn reset_clears_history_and_unlocks_the_crop() {
    let mut edits = edits();
    edits.cuts.push(CutBand::new(CutAxis::Rows, 1, 2).unwrap());
    edits
        .redo
        .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
    edits.mode = CutMode::Armed;
    assert!(edits.reset());
    assert!(edits.cuts.is_empty());
    assert!(edits.redo.is_empty());
    assert!(!edits.crop_locked());
    assert!(edits.preview_is_current());
}

#[test]
fn source_rect_cannot_change_while_cuts_exist() {
    let mut edits = edits();
    let next = ImagePixelRect::new(1, 1, 4, 4, (8, 8)).unwrap();
    assert!(edits.set_source_rect(next));
    edits
        .cuts
        .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
    assert!(!edits.set_source_rect(ImagePixelRect::new(0, 0, 4, 4, (8, 8)).unwrap()));
    assert_eq!(edits.source_rect, next);
}

#[test]
fn cut_start_is_rejected_while_preview_is_pending() {
    let mut edits = edits();
    edits.toggle_mode();
    edits
        .cuts
        .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
    edits.revision = 1;
    edits.set_desired_from(fingerprint(edits.source_rect));
    assert!(!edits.preview_is_current());
    assert!(!edits.can_start_cut_drag());
}

#[test]
fn loupe_is_suppressed_when_armed_or_cuts_exist() {
    let mut edits = edits();
    assert!(!edits.loupe_suppressed());
    edits.toggle_mode();
    assert!(edits.loupe_suppressed());
    edits.toggle_mode();
    edits
        .cuts
        .push(CutBand::new(CutAxis::Columns, 1, 2).unwrap());
    assert!(edits.loupe_suppressed());
}

#[test]
fn column_cut_preserves_top_left_and_height() {
    let token = token();
    let rect = ImagePixelRect::new(0, 0, 8, 8, (8, 8)).unwrap();
    let full = output_display_for(&token, rect, &[]).unwrap();
    let cut = output_display_for(
        &token,
        rect,
        &[CutBand::new(CutAxis::Columns, 2, 4).unwrap()],
    )
    .unwrap();
    assert_eq!(cut.start, full.start);
    assert_eq!(cut.end.1, full.end.1);
    assert!(cut.end.0 < full.end.0);
}

#[test]
fn dominant_axis_ties_choose_columns() {
    assert_eq!(dominant_cut_axis(4.0, 4.0), CutAxis::Columns);
    assert_eq!(dominant_cut_axis(4.0, 5.0), CutAxis::Rows);
}

#[test]
fn pointer_edges_map_to_the_inclusive_pixel_edge_domain() {
    let display = display();
    assert_eq!(
        logical_to_output_point(display, (8, 8), (0.0, 0.0)).map(|point| (point.x, point.y)),
        Some((0.0, 0.0))
    );
    assert_eq!(
        logical_to_output_point(display, (8, 8), (8.0, 8.0)).map(|point| (point.x, point.y)),
        Some((8.0, 8.0))
    );
    let clamped = logical_to_output_point(display, (8, 8), (-2.0, 20.0)).unwrap();
    assert_eq!((clamped.x, clamped.y), (0.0, 8.0));
}

#[test]
fn composed_board_origin_stays_put_while_size_contracts() {
    let source = crate::canvas_export::CanvasExportRect::new(10.0, 20.0, 80.0, 40.0).unwrap();
    let composed = crate::backend::wayland::state::region_capture::world_rect_for_composed_region(
        source,
        (8, 8),
        (6, 4),
    )
    .unwrap();
    assert_eq!(composed.x, 10.0);
    assert_eq!(composed.y, 20.0);
    assert_eq!(composed.width, 60.0);
    assert_eq!(composed.height, 20.0);
}
