use super::super::cut_review::{CutPreviewKey, RegionReviewEdits, native_extent_display};
use super::apply::{CutPreviewVisibleEffect, visible_effect_for_cut_preview};
use super::job::{CutPreviewInput, CutPreviewJob, CutPreviewOutcome, run_cut_preview};
use super::snapshot::{CutPreviewSnapshotClass, classify_cut_preview_snapshot};
use crate::backend::wayland::runtime_operation::{
    RuntimeOperationPoll, RuntimeOperationSubmitError,
};
use crate::backend::wayland::state::WaylandState;
use crate::input::state::{Toast, ToastPriority};
use std::sync::Arc;

pub(super) const TOAST_SOURCE: &str = "capture";

pub(super) fn desired_preview_to_schedule(
    edits: Option<&RegionReviewEdits>,
    worker_active: bool,
) -> Option<CutPreviewKey> {
    let edits = edits?;
    let desired = edits.desired_preview.clone()?;
    if edits.current_preview_failed()
        || worker_active
        || edits
            .ready_preview
            .as_ref()
            .is_some_and(|ready| ready.key == desired)
    {
        return None;
    }
    Some(desired)
}

pub(super) fn cut_preview_from_poll(
    poll: RuntimeOperationPoll<CutPreviewKey, CutPreviewOutcome>,
) -> Option<CutPreviewOutcome> {
    match poll {
        RuntimeOperationPoll::Idle | RuntimeOperationPoll::Pending { .. } => None,
        RuntimeOperationPoll::Ready { outcome, .. } => Some(outcome),
        RuntimeOperationPoll::ProducerFailed {
            context, reason, ..
        } => Some(CutPreviewOutcome::Failed {
            key: context,
            message: reason,
        }),
        RuntimeOperationPoll::Disconnected { context, .. } => Some(CutPreviewOutcome::Failed {
            key: context,
            message: "Cut preview worker disconnected.".to_string(),
        }),
    }
}

pub(super) fn present_cut_preview_effect(
    input: &mut crate::input::InputState,
    effect: CutPreviewVisibleEffect,
) {
    if effect.toast_current_failure {
        input.push_toast(
            ToastPriority::Info,
            TOAST_SOURCE,
            Toast::warning("Could not update the cut preview."),
        );
    }
    if effect.dirty {
        input.dirty_tracker.mark_full();
        input.needs_redraw = true;
    }
}

impl WaylandState {
    pub(in crate::backend::wayland::state::region_capture) fn schedule_region_cut_preview(
        &mut self,
    ) {
        let Some(desired) = desired_preview_to_schedule(
            self.region_capture.review_edits(),
            self.region_capture.cut_preview_active(),
        ) else {
            return;
        };
        let Some(edits) = self.region_capture.review_edits() else {
            return;
        };
        let cached_base = edits
            .base_cache
            .as_ref()
            .filter(|cache| cache.fingerprint == desired.fingerprint)
            .map(|cache| Arc::clone(&cache.pixels));
        let input = if let Some(base) = cached_base {
            CutPreviewInput::CachedBase(base)
        } else {
            match self.snapshot_region_render(
                desired.fingerprint.source_rect(),
                desired.fingerprint.include_drawings(),
            ) {
                Ok(snapshot) => match classify_cut_preview_snapshot(
                    &desired.fingerprint,
                    Ok(&snapshot.fingerprint),
                ) {
                    CutPreviewSnapshotClass::Ready => {
                        CutPreviewInput::RenderSource(snapshot.source)
                    }
                    CutPreviewSnapshotClass::Cancelled => {
                        self.cancel_region_capture_for_source_change();
                        return;
                    }
                    CutPreviewSnapshotClass::Failed { message } => {
                        self.finish_cut_preview_poll(CutPreviewOutcome::Failed {
                            key: desired,
                            message,
                        });
                        return;
                    }
                },
                Err(error) => {
                    match classify_cut_preview_snapshot(&desired.fingerprint, Err(&error)) {
                        CutPreviewSnapshotClass::Cancelled => {
                            self.cancel_region_capture_for_source_change();
                        }
                        CutPreviewSnapshotClass::Failed { message } => {
                            self.finish_cut_preview_poll(CutPreviewOutcome::Failed {
                                key: desired,
                                message,
                            });
                        }
                        CutPreviewSnapshotClass::Ready => {
                            unreachable!("a snapshot error cannot match the desired fingerprint")
                        }
                    }
                    return;
                }
            }
        };
        let job = CutPreviewJob {
            key: desired.clone(),
            input,
        };
        if let Err(failure) = self.region_capture.cut_preview_mut().try_submit(
            desired,
            "wayscriber-region-cut-preview",
            move || run_cut_preview(job),
        ) {
            let (error, key) = failure.into_parts();
            if !matches!(error, RuntimeOperationSubmitError::Busy { .. }) {
                log::debug!("Cut preview worker unavailable: {error}");
                self.finish_cut_preview_poll(CutPreviewOutcome::Failed {
                    key,
                    message: error.to_string(),
                });
            }
        }
    }

    pub(in crate::backend::wayland) fn poll_region_cut_preview_completion(&mut self) {
        if let Some(outcome) = cut_preview_from_poll(self.region_capture.cut_preview_mut().poll()) {
            self.finish_cut_preview_poll(outcome);
            self.schedule_region_cut_preview();
        }
    }

    fn finish_cut_preview_poll(&mut self, outcome: CutPreviewOutcome) {
        let effect = visible_effect_for_cut_preview(
            self.region_capture.review_edits_slot_mut(),
            outcome,
            |edits, cuts| {
                native_extent_display(
                    &edits.correlation.source,
                    edits.source_rect,
                    crate::capture::output_size(
                        (edits.source_rect.width(), edits.source_rect.height()),
                        cuts,
                    )
                    .ok()?,
                )
            },
        );
        present_cut_preview_effect(&mut self.input_state, effect);
    }
}
