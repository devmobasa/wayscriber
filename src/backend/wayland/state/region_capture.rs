use crate::backend::wayland::acquisition::ScreenAcquisitionId;
use crate::input::SelectionHandle;
use crate::input::state::{
    RegionInputSource, RegionPurposeTag, RegionSelectUiState, RegionSelection, ScreenCaptureSource,
};
use crate::screen_pixels::{ImagePixelRect, ImagePoint, clamp_edge};

mod active_state;
mod board;
mod cut_preview;
mod cut_review;
mod delivery;
mod events;
mod geometry;
mod intent;
mod measure;
mod picker;
mod render;
mod review_state;
mod runtime;
mod selection_state;
mod source_guard;
mod window_snap;

pub(super) use active_state::{ActiveScreenRegion, FreezeOwnership};
pub(in crate::backend::wayland) use board::{
    board_bounds_for_world_rect, world_rect_for_composed_region, world_rect_for_image_rect_exact,
};
pub(in crate::backend::wayland) use cut_preview::CutPreviewOutcome;
pub(in crate::backend::wayland) use cut_review::RegionReviewPress;
pub(in crate::backend::wayland) use cut_review::{CutPreviewKey, RegionReviewEdits};
#[cfg(test)]
pub(super) use events::finalize_region_selection_event;
pub(super) use events::finalize_region_selection_with_review_edits;
use events::*;
pub(in crate::backend::wayland) use geometry::{RegionPickerMeasurement, RegionSelectionGeometry};
pub(in crate::backend::wayland) use intent::{RegionCaptureIntent, RegionPickerOptions};
use measure::*;
pub(in crate::backend::wayland) use measure::{RegionOwnerLoss, RegionSelectionFinalize};
use review_state::ReviewResizeGrip;
#[cfg(test)]
use review_state::resized_review_rect;
pub(super) use source_guard::owned_generation_is_current;
use source_guard::*;
pub(in crate::backend::wayland) use window_snap::WindowSnapDirection;
pub(super) use window_snap::{WindowSnapQuery, WindowSnapSession};

use super::WaylandState;
use super::screen_image::{
    ScreenSourceToken, current_screen_source_token, image_point_for_screen_point, screen_source_is,
};

#[cfg(test)]
mod tests;
