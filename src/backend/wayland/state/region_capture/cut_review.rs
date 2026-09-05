//! Region cut values, edit history, geometry, and backend actions.

mod actions;
mod geometry;
mod history;
mod model;

pub(in crate::backend::wayland) use actions::RegionReviewPress;
pub(super) use actions::review_edits_for_active_region;
pub(super) use geometry::native_extent_display;
#[cfg(test)]
pub(super) use model::{CutCommit, CutMode};
pub(in crate::backend::wayland) use model::{
    CutPreviewKey, RegionCutBase, RegionCutPreview, RegionRenderFingerprint,
    RegionReviewCorrelation, RegionReviewEdits,
};
pub(super) use model::{PreviewApply, RegionAnnotatedRenderContext};

#[cfg(test)]
mod tests;
