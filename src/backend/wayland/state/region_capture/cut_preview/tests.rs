use super::super::ActiveScreenRegion;
use super::super::cut_review::{
    CutPreviewKey, PreviewApply, RegionAnnotatedRenderContext, RegionCutBase, RegionCutPreview,
    RegionRenderFingerprint, RegionReviewCorrelation, RegionReviewEdits,
};
use super::apply::{apply_cut_preview_outcome, visible_effect_for_cut_preview};
use super::job::{CutPreviewInput, CutPreviewJob, CutPreviewOutcome, run_cut_preview};
use super::scheduler::{
    TOAST_SOURCE, cut_preview_from_poll, desired_preview_to_schedule, present_cut_preview_effect,
};
use super::snapshot::{
    CutPreviewSnapshotClass, capture_ready_correlation, classify_cut_preview_snapshot,
};
use crate::backend::wayland::runtime_operation::RuntimeOperationPoll;
use crate::backend::wayland::state::screen_image::ScreenImageKind;
use crate::capture::CutAxis;
use crate::capture::{CaptureError, CutBand};
use crate::input::state::RegionSelection;
use crate::screen_pixels::{ImagePixelRect, PackedArgb32};
use std::sync::Arc;
use wayland_client::protocol::wl_output::Transform;

type ContextMutation = (&'static str, fn(&mut RegionAnnotatedRenderContext));

fn token() -> crate::backend::wayland::state::screen_image::ScreenSourceToken {
    crate::backend::wayland::state::screen_image::ScreenSourceToken {
        output_id: 1,
        output_layout_generation: 1,
        kind: ScreenImageKind::Frozen,
        image_generation: 1,
        image_size: (2, 1),
        stride: 8,
        surface: (2, 1),
        output_scale: 1,
        output_transform: Transform::Normal,
        zoom_transformed: false,
        zoom_scale: 1.0,
        zoom_view_offset: (0.0, 0.0),
    }
}

fn key(revision: u64, generation: u64) -> CutPreviewKey {
    let rect = ImagePixelRect::new(0, 0, 2, 1, (2, 1)).unwrap();
    CutPreviewKey {
        fingerprint: RegionRenderFingerprint::Raw {
            correlation: RegionReviewCorrelation {
                generation,
                source: token(),
            },
            source_rect: rect,
        },
        revision,
        cuts: vec![CutBand::new(CutAxis::Columns, 1, 2).unwrap()],
    }
}

fn annotated_key(revision: u64, generation: u64) -> CutPreviewKey {
    let mut key = key(revision, generation);
    key.fingerprint = RegionRenderFingerprint::Annotated {
        correlation: key.fingerprint.correlation().clone(),
        source_rect: key.fingerprint.source_rect(),
        context: RegionAnnotatedRenderContext {
            board_id: "board-a".to_string(),
            page_index: 2,
            page_generation: 3,
            canvas_content_generation: 4,
            board_view_offset: (5.0, 6.0),
            text_halo_enabled: true,
            spotlight: crate::canvas_export::SpotlightPassSnapshot {
                dim_opacity: 0.7,
                feather: 0.2,
            },
        },
    };
    key
}

fn pixels() -> Arc<PackedArgb32> {
    Arc::new(
        PackedArgb32::new(
            2,
            1,
            8,
            [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88].to_vec(),
        )
        .unwrap(),
    )
}

fn display_selection() -> RegionSelection {
    RegionSelection {
        start: (0.0, 0.0),
        end: (1.0, 1.0),
    }
}

fn display(_: &RegionReviewEdits, _: &[CutBand]) -> Option<RegionSelection> {
    Some(display_selection())
}

#[test]
fn matching_completion_installs_base_and_preview() {
    let rect = ImagePixelRect::new(0, 0, 2, 1, (2, 1)).unwrap();
    let mut edits = Some(RegionReviewEdits::new(
        RegionReviewCorrelation {
            generation: 1,
            source: token(),
        },
        rect,
    ));
    let desired = key(1, 1);
    edits.as_mut().unwrap().cuts = desired.cuts.clone();
    edits.as_mut().unwrap().revision = 1;
    edits.as_mut().unwrap().desired_preview = Some(desired.clone());
    let composed = Arc::new(PackedArgb32::new(1, 1, 4, vec![0x55, 0x66, 0x77, 0x88]).unwrap());
    let applied = apply_cut_preview_outcome(
        &mut edits,
        CutPreviewOutcome::Success {
            key: desired,
            base: pixels(),
            composed: Arc::clone(&composed),
        },
        display,
    );
    assert_eq!(applied, PreviewApply::Changed);
    let edits = edits.unwrap();
    assert!(edits.base_cache.is_some());
    assert_eq!(
        edits
            .ready_preview
            .as_ref()
            .map(|preview| preview.pixels.as_ref()),
        Some(composed.as_ref())
    );
    assert!(edits.preview_is_current());
}

#[test]
fn stale_revision_may_cache_the_base_only() {
    let rect = ImagePixelRect::new(0, 0, 2, 1, (2, 1)).unwrap();
    let mut edits = Some(RegionReviewEdits::new(
        RegionReviewCorrelation {
            generation: 1,
            source: token(),
        },
        rect,
    ));
    let stale = key(1, 1);
    let desired = key(2, 1);
    edits.as_mut().unwrap().cuts = desired.cuts.clone();
    edits.as_mut().unwrap().revision = 2;
    edits.as_mut().unwrap().desired_preview = Some(desired);
    let applied = apply_cut_preview_outcome(
        &mut edits,
        CutPreviewOutcome::Success {
            key: stale,
            base: pixels(),
            composed: Arc::new(PackedArgb32::new(1, 1, 4, vec![0; 4]).unwrap()),
        },
        display,
    );
    assert_eq!(applied, PreviewApply::Changed);
    let edits = edits.unwrap();
    assert!(edits.base_cache.is_some());
    assert!(edits.ready_preview.is_none());
}

#[test]
fn different_generation_rejects_all_output() {
    let rect = ImagePixelRect::new(0, 0, 2, 1, (2, 1)).unwrap();
    let mut edits = Some(RegionReviewEdits::new(
        RegionReviewCorrelation {
            generation: 2,
            source: token(),
        },
        rect,
    ));
    let applied = apply_cut_preview_outcome(
        &mut edits,
        CutPreviewOutcome::Success {
            key: key(1, 1),
            base: pixels(),
            composed: pixels(),
        },
        display,
    );
    assert_eq!(applied, PreviewApply::Ignored);
    assert!(edits.unwrap().ready_preview.is_none());
}

#[test]
fn every_nonmatching_render_fingerprint_field_rejects_completed_output() {
    let desired = annotated_key(1, 1);
    let mut mismatches = Vec::new();

    let mut different_source = desired.clone();
    let RegionRenderFingerprint::Annotated { correlation, .. } = &mut different_source.fingerprint
    else {
        unreachable!("annotated fixture stays annotated");
    };
    correlation.source.image_generation = 9;
    mismatches.push(("source token", different_source));

    let mut different_rect = desired.clone();
    let RegionRenderFingerprint::Annotated { source_rect, .. } = &mut different_rect.fingerprint
    else {
        unreachable!("annotated fixture stays annotated");
    };
    *source_rect = ImagePixelRect::new(0, 0, 1, 1, (2, 1)).unwrap();
    mismatches.push(("source rectangle", different_rect));

    let mut without_drawings = desired.clone();
    without_drawings.fingerprint = RegionRenderFingerprint::Raw {
        correlation: desired.fingerprint.correlation().clone(),
        source_rect: desired.fingerprint.source_rect(),
    };
    mismatches.push(("include drawings", without_drawings));

    let context_mutations: [ContextMutation; 8] = [
        (
            "board identity",
            |context: &mut RegionAnnotatedRenderContext| context.board_id = "board-b".to_string(),
        ),
        ("page identity", |context| context.page_index += 1),
        ("page generation", |context| context.page_generation += 1),
        ("canvas content generation", |context| {
            context.canvas_content_generation += 1;
        }),
        ("board view offset", |context| {
            context.board_view_offset.0 += 1.0;
        }),
        ("text halo", |context| {
            context.text_halo_enabled = !context.text_halo_enabled;
        }),
        ("Spotlight opacity", |context| {
            context.spotlight.dim_opacity += 0.1;
        }),
        ("Spotlight feather", |context| {
            context.spotlight.feather += 0.1;
        }),
    ];
    for (name, mutate) in context_mutations {
        let mut mismatch = desired.clone();
        let RegionRenderFingerprint::Annotated { context, .. } = &mut mismatch.fingerprint else {
            unreachable!("annotated fixture stays annotated");
        };
        mutate(context);
        mismatches.push((name, mismatch));
    }

    for (name, mismatch) in mismatches {
        let mut edits = review_edits(desired.clone());
        let applied = apply_cut_preview_outcome(
            &mut edits,
            CutPreviewOutcome::Success {
                key: mismatch,
                base: pixels(),
                composed: Arc::new(PackedArgb32::new(1, 1, 4, vec![0; 4]).unwrap()),
            },
            display,
        );
        let edits = edits.unwrap();
        assert_eq!(applied, PreviewApply::Ignored, "{name}");
        assert!(edits.base_cache.is_none(), "{name} donated a stale base");
        assert!(
            edits.ready_preview.is_none(),
            "{name} installed a stale preview"
        );
    }
}

#[test]
fn current_failure_records_visible_state() {
    let rect = ImagePixelRect::new(0, 0, 2, 1, (2, 1)).unwrap();
    let mut edits = Some(RegionReviewEdits::new(
        RegionReviewCorrelation {
            generation: 1,
            source: token(),
        },
        rect,
    ));
    let desired = key(3, 1);
    edits.as_mut().unwrap().cuts = desired.cuts.clone();
    edits.as_mut().unwrap().revision = 3;
    edits.as_mut().unwrap().desired_preview = Some(desired.clone());
    let applied = apply_cut_preview_outcome(
        &mut edits,
        CutPreviewOutcome::Failed {
            key: desired,
            message: "boom".to_string(),
        },
        display,
    );
    assert_eq!(applied, PreviewApply::Changed);
    assert_eq!(edits.unwrap().failed_revision, Some(3));
}

#[test]
fn stale_failure_does_not_touch_the_new_review() {
    let rect = ImagePixelRect::new(0, 0, 2, 1, (2, 1)).unwrap();
    let mut edits = Some(RegionReviewEdits::new(
        RegionReviewCorrelation {
            generation: 2,
            source: token(),
        },
        rect,
    ));
    let applied = apply_cut_preview_outcome(
        &mut edits,
        CutPreviewOutcome::Failed {
            key: key(1, 1),
            message: "old".to_string(),
        },
        display,
    );
    assert_eq!(applied, PreviewApply::Ignored);
    assert!(edits.unwrap().failed_revision.is_none());
}

fn ready(purpose: crate::input::state::RegionPurposeTag) -> ActiveScreenRegion {
    ActiveScreenRegion::Ready {
        purpose,
        generation: 7,
        source: token(),
        freeze_ownership: super::super::FreezeOwnership::PreExisting,
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
fn render_snapshot_accepts_both_capture_purposes() {
    use crate::input::state::RegionPurposeTag;
    assert!(capture_ready_correlation(Some(ready(RegionPurposeTag::CaptureDeliver))).is_ok());
    assert!(capture_ready_correlation(Some(ready(RegionPurposeTag::CaptureInteractive))).is_ok());
    assert!(capture_ready_correlation(Some(ready(RegionPurposeTag::Ocr))).is_err());
    assert!(capture_ready_correlation(Some(ready(RegionPurposeTag::Measure))).is_err());
    assert!(capture_ready_correlation(None).is_err());
    let correlation =
        capture_ready_correlation(Some(ready(RegionPurposeTag::CaptureDeliver))).unwrap();
    assert_eq!(correlation.generation, 7);
    let err = capture_ready_correlation(Some(ready(RegionPurposeTag::Ocr))).unwrap_err();
    assert!(
        !err.to_string().contains("review"),
        "direct delivery must not require Review: {err}"
    );
}

#[test]
fn snapshot_classification_routes_cancel_and_current_failures() {
    let desired = key(1, 1);
    assert_eq!(
        classify_cut_preview_snapshot(&desired.fingerprint, Ok(&desired.fingerprint)),
        CutPreviewSnapshotClass::Ready
    );
    let other = key(1, 2);
    assert!(matches!(
        classify_cut_preview_snapshot(&desired.fingerprint, Ok(&other.fingerprint)),
        CutPreviewSnapshotClass::Failed { .. }
    ));
    assert_eq!(
        classify_cut_preview_snapshot(
            &desired.fingerprint,
            Err(&CaptureError::Cancelled("changed".to_string()))
        ),
        CutPreviewSnapshotClass::Cancelled
    );
    assert!(matches!(
        classify_cut_preview_snapshot(
            &desired.fingerprint,
            Err(&CaptureError::ImageError("nope".to_string()))
        ),
        CutPreviewSnapshotClass::Failed { .. }
    ));
    let mut other_token = token();
    other_token.image_generation = 99;
    let other_source = RegionRenderFingerprint::Raw {
        correlation: RegionReviewCorrelation {
            generation: 1,
            source: other_token,
        },
        source_rect: desired.fingerprint.source_rect(),
    };
    assert_eq!(
        classify_cut_preview_snapshot(&desired.fingerprint, Ok(&other_source)),
        CutPreviewSnapshotClass::Cancelled
    );
    let drifted = RegionRenderFingerprint::Raw {
        correlation: desired.fingerprint.correlation().clone(),
        source_rect: ImagePixelRect::new(0, 0, 1, 1, (2, 1)).unwrap(),
    };
    assert!(matches!(
        classify_cut_preview_snapshot(&desired.fingerprint, Ok(&drifted)),
        CutPreviewSnapshotClass::Failed { .. }
    ));
}

#[test]
fn preview_job_composes_from_the_key_cuts() {
    let desired = key(1, 1);
    match run_cut_preview(CutPreviewJob {
        key: desired.clone(),
        input: CutPreviewInput::CachedBase(pixels()),
    }) {
        CutPreviewOutcome::Success { key, composed, .. } => {
            assert_eq!(key.cuts, desired.cuts);
            assert_eq!((composed.width(), composed.height()), (1, 1));
        }
        CutPreviewOutcome::Failed { message, .. } => panic!("{message}"),
    }
}

#[test]
fn empty_cuts_reuse_the_cached_base_raster() {
    let mut desired = key(1, 1);
    desired.cuts.clear();
    let base = pixels();
    match run_cut_preview(CutPreviewJob {
        key: desired,
        input: CutPreviewInput::CachedBase(Arc::clone(&base)),
    }) {
        CutPreviewOutcome::Success {
            base: out_base,
            composed,
            ..
        } => {
            assert!(Arc::ptr_eq(&base, &out_base));
            assert!(Arc::ptr_eq(&out_base, &composed));
            assert_eq!((composed.width(), composed.height()), (2, 1));
        }
        CutPreviewOutcome::Failed { message, .. } => panic!("{message}"),
    }
}

fn review_edits(desired: CutPreviewKey) -> Option<RegionReviewEdits> {
    let rect = desired.fingerprint.source_rect();
    let mut edits = RegionReviewEdits::new(desired.fingerprint.correlation().clone(), rect);
    edits.cuts = desired.cuts.clone();
    edits.revision = desired.revision;
    edits.desired_preview = Some(desired);
    Some(edits)
}

fn poll_until_terminal(
    controller: &mut crate::backend::wayland::runtime_operation::RuntimeOperationController<
        CutPreviewKey,
        CutPreviewOutcome,
    >,
) -> RuntimeOperationPoll<CutPreviewKey, CutPreviewOutcome> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match controller.poll() {
            RuntimeOperationPoll::Pending { .. } => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "cut preview worker did not finish"
                );
                std::thread::yield_now();
            }
            poll => return poll,
        }
    }
}

fn apply_poll(
    edits: &mut Option<RegionReviewEdits>,
    input: &mut crate::input::InputState,
    poll: RuntimeOperationPoll<CutPreviewKey, CutPreviewOutcome>,
) {
    if let Some(outcome) = cut_preview_from_poll(poll) {
        let effect = visible_effect_for_cut_preview(edits, outcome, display);
        present_cut_preview_effect(input, effect);
    }
}

fn preview_controller() -> crate::backend::wayland::runtime_operation::RuntimeOperationController<
    CutPreviewKey,
    CutPreviewOutcome,
> {
    let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
    crate::backend::wayland::runtime_operation::RuntimeOperationController::new(
        crate::backend::wayland::runtime_operation::RuntimeOperationIdSource::new(),
        wake.handle(),
    )
}

#[test]
fn worker_error_toasts_once_and_marks_dirty_for_the_current_revision() {
    let desired = key(3, 1);
    let mut edits = review_edits(desired.clone());
    let mut input = crate::input::state::test_support::make_test_input_state();
    input.needs_redraw = false;
    let _ = input.dirty_tracker.take_region_report(2, 1);

    let mut controller = preview_controller();
    let job_key = desired.clone();
    controller
        .try_submit(desired.clone(), "test-cut-preview-error", move || {
            CutPreviewOutcome::Failed {
                key: job_key,
                message: "pixels".to_string(),
            }
        })
        .unwrap();
    apply_poll(&mut edits, &mut input, poll_until_terminal(&mut controller));

    assert_eq!(edits.as_ref().unwrap().failed_revision, Some(3));
    assert!(edits.as_ref().unwrap().current_preview_failed());
    assert_eq!(input.test_toast_count(), 1);
    assert_eq!(
        input.test_active_toast_message(),
        Some("Could not update the cut preview.")
    );
    assert_eq!(input.test_active_toast_key(), Some(TOAST_SOURCE));
    assert!(input.needs_redraw);
    assert_eq!(
        input.dirty_tracker.take_region_report(2, 1).regions.len(),
        1
    );

    present_cut_preview_effect(
        &mut input,
        visible_effect_for_cut_preview(
            &mut edits,
            CutPreviewOutcome::Failed {
                key: desired,
                message: "pixels again".to_string(),
            },
            display,
        ),
    );
    assert_eq!(
        input.test_toast_count(),
        1,
        "same capture key must not stack a second preview-failure toast"
    );
}

#[test]
fn worker_panic_and_disconnect_fail_the_current_preview() {
    let desired = key(4, 1);
    let mut edits = review_edits(desired.clone());
    let mut input = crate::input::state::test_support::make_test_input_state();
    let mut controller = preview_controller();
    controller
        .try_submit(desired.clone(), "test-cut-preview-panic", || {
            panic!("expected cut preview panic")
        })
        .unwrap();
    apply_poll(&mut edits, &mut input, poll_until_terminal(&mut controller));
    assert_eq!(edits.as_ref().unwrap().failed_revision, Some(4));
    assert_eq!(input.test_toast_count(), 1);

    let desired = key(5, 1);
    let mut edits = review_edits(desired.clone());
    let mut input = crate::input::state::test_support::make_test_input_state();
    let mut controller = preview_controller();
    controller
        .try_submit_with_spawner_for_test(
            desired.clone(),
            || panic!("must not run"),
            |job| {
                drop(job);
                Ok(())
            },
        )
        .unwrap();
    apply_poll(&mut edits, &mut input, controller.poll());
    assert_eq!(edits.as_ref().unwrap().failed_revision, Some(5));
    assert_eq!(input.test_toast_count(), 1);
}

#[test]
fn submit_failure_is_visible_and_busy_is_not() {
    let desired = key(6, 1);
    let mut edits = review_edits(desired.clone());
    let mut input = crate::input::state::test_support::make_test_input_state();
    let mut controller = preview_controller();
    let failure = controller
        .try_submit_with_spawner_for_test(
            desired.clone(),
            || panic!("must not run"),
            |_job| Err(std::io::Error::other("injected spawn failure")),
        )
        .unwrap_err();
    let (error, failed_key) = failure.into_parts();
    assert!(matches!(
        error,
        crate::backend::wayland::runtime_operation::RuntimeOperationSubmitError::SpawnFailed { .. }
    ));
    present_cut_preview_effect(
        &mut input,
        visible_effect_for_cut_preview(
            &mut edits,
            CutPreviewOutcome::Failed {
                key: failed_key,
                message: error.to_string(),
            },
            display,
        ),
    );
    assert_eq!(edits.as_ref().unwrap().failed_revision, Some(6));
    assert_eq!(input.test_toast_count(), 1);

    let desired = key(7, 1);
    let edits = review_edits(desired.clone());
    let input = crate::input::state::test_support::make_test_input_state();
    let mut controller = preview_controller();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocked_key = desired.clone();
    controller
        .try_submit(desired.clone(), "test-cut-preview-busy", move || {
            release_rx.recv().unwrap();
            CutPreviewOutcome::Failed {
                key: blocked_key,
                message: "late".to_string(),
            }
        })
        .unwrap();
    let busy = controller
        .try_submit(desired.clone(), "test-cut-preview-busy-2", || {
            panic!("must not run")
        })
        .unwrap_err()
        .into_parts()
        .0;
    assert!(matches!(
        busy,
        crate::backend::wayland::runtime_operation::RuntimeOperationSubmitError::Busy { .. }
    ));
    assert!(edits.as_ref().unwrap().failed_revision.is_none());
    assert_eq!(input.test_toast_count(), 0);
    release_tx.send(()).unwrap();
    let _ = poll_until_terminal(&mut controller);
}

#[test]
fn identity_mismatch_and_stale_failure_follow_current_or_silent_rules() {
    let desired = key(8, 1);
    let mut edits = review_edits(desired.clone());
    let mut input = crate::input::state::test_support::make_test_input_state();
    apply_poll(
        &mut edits,
        &mut input,
        RuntimeOperationPoll::ProducerFailed {
            id: crate::backend::wayland::runtime_operation::RuntimeOperationId::from_test(3),
            context: desired.clone(),
            reason: "runtime operation worker reported transport identity 4, expected 3"
                .to_string(),
        },
    );
    assert_eq!(edits.as_ref().unwrap().failed_revision, Some(8));
    assert_eq!(input.test_toast_count(), 1);
    assert!(input.needs_redraw);

    let mut next = review_edits(key(9, 2));
    let mut input = crate::input::state::test_support::make_test_input_state();
    input.needs_redraw = false;
    let _ = input.dirty_tracker.take_region_report(2, 1);
    apply_poll(
        &mut next,
        &mut input,
        RuntimeOperationPoll::ProducerFailed {
            id: crate::backend::wayland::runtime_operation::RuntimeOperationId::from_test(3),
            context: desired,
            reason: "old picker".to_string(),
        },
    );
    assert!(next.as_ref().unwrap().failed_revision.is_none());
    assert_eq!(input.test_toast_count(), 0);
    assert!(!input.needs_redraw);
    assert!(
        input
            .dirty_tracker
            .take_region_report(2, 1)
            .regions
            .is_empty()
    );
}

#[test]
fn busy_controller_runs_only_the_newest_desired_key_after_terminal_poll() {
    let first = key(10, 1);
    let second = key(11, 1);
    let newest = key(12, 1);
    let mut edits = review_edits(first.clone());
    let mut controller = preview_controller();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let first_job_key = first.clone();
    controller
        .try_submit(first, "test-cut-preview-queued-first", move || {
            release_rx.recv().unwrap();
            CutPreviewOutcome::Failed {
                key: first_job_key,
                message: "stale".to_string(),
            }
        })
        .unwrap();

    edits.as_mut().unwrap().desired_preview = Some(second);
    assert!(
        desired_preview_to_schedule(edits.as_ref(), controller.is_active()).is_none(),
        "busy preview work leaves the current desired key queued in Review state"
    );
    edits.as_mut().unwrap().desired_preview = Some(newest.clone());
    assert!(
        desired_preview_to_schedule(edits.as_ref(), controller.is_active()).is_none(),
        "a later edit replaces the queued desired key instead of appending work"
    );

    release_tx.send(()).unwrap();
    assert!(cut_preview_from_poll(poll_until_terminal(&mut controller)).is_some());
    let scheduled = desired_preview_to_schedule(edits.as_ref(), controller.is_active())
        .expect("terminal consumption schedules the newest desired key");
    assert_eq!(scheduled, newest);

    let scheduled_job_key = scheduled.clone();
    controller
        .try_submit(scheduled, "test-cut-preview-queued-newest", move || {
            CutPreviewOutcome::Failed {
                key: scheduled_job_key,
                message: "newest".to_string(),
            }
        })
        .unwrap();
    assert!(matches!(
        poll_until_terminal(&mut controller),
        RuntimeOperationPoll::Ready {
            context,
            outcome: CutPreviewOutcome::Failed { key, .. },
            ..
        } if context == newest && key == newest
    ));
}

#[test]
fn reset_while_worker_is_active_releases_buffers_and_ignores_completion() {
    let active_key = key(1, 1);
    let mut edits = review_edits(active_key.clone());
    let cached_base = pixels();
    let cached_base_weak = Arc::downgrade(&cached_base);
    let cached_preview = Arc::new(PackedArgb32::new(1, 1, 4, vec![0; 4]).unwrap());
    let cached_preview_weak = Arc::downgrade(&cached_preview);
    edits.as_mut().unwrap().base_cache = Some(RegionCutBase {
        fingerprint: active_key.fingerprint.clone(),
        pixels: cached_base,
    });
    edits.as_mut().unwrap().ready_preview = Some(RegionCutPreview {
        key: active_key.clone(),
        pixels: cached_preview,
        display: display_selection(),
    });

    let worker_base = pixels();
    let worker_base_weak = Arc::downgrade(&worker_base);
    let worker_preview = Arc::new(PackedArgb32::new(1, 1, 4, vec![1; 4]).unwrap());
    let worker_preview_weak = Arc::downgrade(&worker_preview);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let mut controller = preview_controller();
    let worker_key = active_key.clone();
    controller
        .try_submit(active_key, "test-cut-preview-reset-active", move || {
            release_rx.recv().unwrap();
            CutPreviewOutcome::Success {
                key: worker_key,
                base: worker_base,
                composed: worker_preview,
            }
        })
        .unwrap();

    assert!(edits.as_mut().unwrap().reset());
    assert!(cached_base_weak.upgrade().is_none());
    assert!(cached_preview_weak.upgrade().is_none());
    release_tx.send(()).unwrap();
    let outcome = cut_preview_from_poll(poll_until_terminal(&mut controller)).unwrap();
    assert_eq!(
        apply_cut_preview_outcome(&mut edits, outcome, display),
        PreviewApply::Ignored
    );
    let edits = edits.unwrap();
    assert!(edits.base_cache.is_none());
    assert!(edits.ready_preview.is_none());
    assert!(edits.desired_preview.is_none());
    assert!(worker_base_weak.upgrade().is_none());
    assert!(worker_preview_weak.upgrade().is_none());
}

#[test]
fn review_exit_and_reopen_releases_buffers_and_rejects_old_completion() {
    let old_key = key(1, 1);
    let mut edits = review_edits(old_key.clone());
    let cached_base = pixels();
    let cached_base_weak = Arc::downgrade(&cached_base);
    let cached_preview = Arc::new(PackedArgb32::new(1, 1, 4, vec![0; 4]).unwrap());
    let cached_preview_weak = Arc::downgrade(&cached_preview);
    edits.as_mut().unwrap().base_cache = Some(RegionCutBase {
        fingerprint: old_key.fingerprint.clone(),
        pixels: cached_base,
    });
    edits.as_mut().unwrap().ready_preview = Some(RegionCutPreview {
        key: old_key.clone(),
        pixels: cached_preview,
        display: display_selection(),
    });

    let worker_base = pixels();
    let worker_base_weak = Arc::downgrade(&worker_base);
    let worker_preview = Arc::new(PackedArgb32::new(1, 1, 4, vec![1; 4]).unwrap());
    let worker_preview_weak = Arc::downgrade(&worker_preview);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let mut controller = preview_controller();
    let worker_key = old_key.clone();
    controller
        .try_submit(old_key, "test-cut-preview-reopen", move || {
            release_rx.recv().unwrap();
            CutPreviewOutcome::Success {
                key: worker_key,
                base: worker_base,
                composed: worker_preview,
            }
        })
        .unwrap();

    edits = None;
    assert!(
        edits.is_none(),
        "Review exit clears the transient edit state"
    );
    assert!(cached_base_weak.upgrade().is_none());
    assert!(cached_preview_weak.upgrade().is_none());
    edits = review_edits(key(1, 2));
    release_tx.send(()).unwrap();
    let outcome = cut_preview_from_poll(poll_until_terminal(&mut controller)).unwrap();
    assert_eq!(
        apply_cut_preview_outcome(&mut edits, outcome, display),
        PreviewApply::Ignored
    );
    let edits = edits.unwrap();
    assert!(edits.base_cache.is_none());
    assert!(edits.ready_preview.is_none());
    assert!(worker_base_weak.upgrade().is_none());
    assert!(worker_preview_weak.upgrade().is_none());
}

#[test]
fn render_source_jobs_paint_annotations_before_applying_key_cuts_on_the_worker() {
    use super::super::render::RegionPixelSource;
    use crate::canvas_export::{CanvasExportRect, CanvasRegionExportSnapshot, CanvasRegionSource};
    use crate::draw::{Frame, RED, Shape};
    use crate::screen_pixels::ScreenImage;

    let image = Arc::new(ScreenImage {
        data: 0xff00_00ff_u32.to_ne_bytes().repeat(64),
        width: 8,
        height: 8,
        stride: 32,
    });
    let selection = ImagePixelRect::new(0, 0, 8, 8, (8, 8)).unwrap();
    for annotated in [false, true] {
        let mut desired = key(3, 7);
        let mut source_token = token();
        source_token.image_size = (8, 8);
        source_token.surface = (8, 8);
        source_token.stride = 32;
        let correlation = RegionReviewCorrelation {
            generation: 7,
            source: source_token,
        };
        let spotlight = crate::canvas_export::SpotlightPassSnapshot {
            dim_opacity: 0.0,
            feather: 0.0,
        };
        desired.fingerprint = if annotated {
            RegionRenderFingerprint::Annotated {
                correlation,
                source_rect: selection,
                context: RegionAnnotatedRenderContext {
                    board_id: "board-a".into(),
                    page_index: 0,
                    page_generation: 1,
                    canvas_content_generation: 1,
                    board_view_offset: (0.0, 0.0),
                    text_halo_enabled: true,
                    spotlight,
                },
            }
        } else {
            RegionRenderFingerprint::Raw {
                correlation,
                source_rect: selection,
            }
        };
        desired.cuts = vec![CutBand::new(CutAxis::Columns, 4, 8).unwrap()];
        let source = if annotated {
            let mut frame = Frame::new();
            frame.add_shape(Shape::Rect {
                x: 0,
                y: 0,
                w: 8,
                h: 8,
                fill: true,
                color: RED,
                thick: 1.0,
            });
            RegionPixelSource::Annotated(Box::new(CanvasRegionExportSnapshot {
                source: CanvasRegionSource {
                    image: Arc::clone(&image),
                    logical_bounds: CanvasExportRect::new(0.0, 0.0, 8.0, 8.0).unwrap(),
                },
                selection,
                frame,
                text_halo_enabled: true,
                spotlight,
            }))
        } else {
            RegionPixelSource::Raw {
                image: Arc::clone(&image),
                selection,
            }
        };
        let job = CutPreviewJob {
            key: desired.clone(),
            input: CutPreviewInput::RenderSource(source),
        };
        let outcome = std::thread::spawn(move || run_cut_preview(job))
            .join()
            .unwrap();
        let CutPreviewOutcome::Success {
            key,
            base,
            composed,
        } = outcome
        else {
            panic!("valid source job failed");
        };
        assert_eq!(key, desired);
        assert_eq!((base.width(), base.height()), (8, 8));
        assert_eq!((composed.width(), composed.height()), (4, 8));
        let expected = if annotated {
            0xffff_0000_u32
        } else {
            0xff00_00ff_u32
        };
        for pixels in [&base, &composed] {
            let offset = 4 * pixels.stride() as usize + 2 * 4;
            assert_eq!(
                u32::from_ne_bytes(pixels.data()[offset..offset + 4].try_into().unwrap()),
                expected
            );
        }
    }
}

#[test]
fn render_source_failure_retains_the_job_correlation() {
    use super::super::render::RegionPixelSource;
    use crate::screen_pixels::ScreenImage;

    let desired = key(4, 9);
    let job = CutPreviewJob {
        key: desired.clone(),
        input: CutPreviewInput::RenderSource(RegionPixelSource::Raw {
            image: Arc::new(ScreenImage {
                data: 0xff00_00ff_u32.to_ne_bytes().to_vec(),
                width: 1,
                height: 1,
                stride: 4,
            }),
            selection: desired.fingerprint.source_rect(),
        }),
    };
    let CutPreviewOutcome::Failed { key, message } = run_cut_preview(job) else {
        panic!("out-of-image source should fail");
    };
    assert_eq!(key, desired);
    assert_eq!(
        message,
        "Image processing error: Could not copy the selected screen pixels."
    );
}
