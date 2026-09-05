use super::super::cut_review::{
    CutPreviewKey, PreviewApply, RegionCutBase, RegionCutPreview, RegionReviewEdits,
};
use super::job::CutPreviewOutcome;
use crate::capture::CutBand;
use std::sync::Arc;

fn preview_matches_review(edits: &RegionReviewEdits, key: &CutPreviewKey) -> bool {
    key.fingerprint.correlation() == &edits.correlation
        && key.fingerprint.source_rect() == edits.source_rect
}

pub(super) fn apply_cut_preview_outcome(
    edits: &mut Option<RegionReviewEdits>,
    outcome: CutPreviewOutcome,
    display_for: impl FnOnce(
        &RegionReviewEdits,
        &[CutBand],
    ) -> Option<crate::input::state::RegionSelection>,
) -> PreviewApply {
    let Some(edits) = edits.as_mut() else {
        return PreviewApply::Ignored;
    };
    match outcome {
        CutPreviewOutcome::Failed { key, .. } => {
            if !preview_matches_review(edits, &key) {
                return PreviewApply::Ignored;
            }
            if edits.mark_preview_failed(&key) {
                PreviewApply::Changed
            } else {
                PreviewApply::Ignored
            }
        }
        CutPreviewOutcome::Success {
            key,
            base,
            composed,
        } => {
            if !preview_matches_review(edits, &key) {
                return PreviewApply::Ignored;
            }
            let mut changed = false;
            if edits
                .desired_preview
                .as_ref()
                .is_some_and(|desired| desired.fingerprint == key.fingerprint)
            {
                edits.base_cache = Some(RegionCutBase {
                    fingerprint: key.fingerprint.clone(),
                    pixels: Arc::clone(&base),
                });
                changed = true;
            }
            if edits.desired_preview.as_ref() == Some(&key) {
                let Some(display) = display_for(edits, &key.cuts) else {
                    return if changed {
                        PreviewApply::Changed
                    } else {
                        PreviewApply::Ignored
                    };
                };
                if (composed.width(), composed.height())
                    != output_size_from_key(&key).unwrap_or((0, 0))
                {
                    return if changed {
                        PreviewApply::Changed
                    } else {
                        PreviewApply::Ignored
                    };
                }
                edits.ready_preview = Some(RegionCutPreview {
                    key,
                    pixels: composed,
                    display,
                });
                edits.failed_revision = None;
                changed = true;
            }
            if changed {
                PreviewApply::Changed
            } else {
                PreviewApply::Ignored
            }
        }
    }
}

fn output_size_from_key(key: &CutPreviewKey) -> Option<(u32, u32)> {
    crate::capture::output_size(
        (
            key.fingerprint.source_rect().width(),
            key.fingerprint.source_rect().height(),
        ),
        &key.cuts,
    )
    .ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CutPreviewVisibleEffect {
    pub(super) toast_current_failure: bool,
    pub(super) dirty: bool,
}

pub(super) fn visible_effect_for_cut_preview(
    edits: &mut Option<RegionReviewEdits>,
    outcome: CutPreviewOutcome,
    display_for: impl FnOnce(
        &RegionReviewEdits,
        &[CutBand],
    ) -> Option<crate::input::state::RegionSelection>,
) -> CutPreviewVisibleEffect {
    let desired = edits
        .as_ref()
        .and_then(|edits| edits.desired_preview.clone());
    let toast_current_failure = matches!(
        &outcome,
        CutPreviewOutcome::Failed { key, .. } if desired.as_ref() == Some(key)
    );
    if let CutPreviewOutcome::Failed { message, .. } = &outcome
        && !toast_current_failure
    {
        log::debug!("Ignoring stale cut preview failure: {message}");
    }
    let applied = apply_cut_preview_outcome(edits, outcome, display_for);
    CutPreviewVisibleEffect {
        toast_current_failure,
        dirty: applied == PreviewApply::Changed,
    }
}
