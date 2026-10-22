#[cfg(test)]
use crate::input::SelectionHandle;
#[cfg(test)]
use crate::input::state::{
    RegionInputSource, RegionPurposeTag, RegionSelectUiState, RegionSelection, ScreenCaptureSource,
};
#[cfg(test)]
use crate::screen_pixels::{ImagePixelRect, ImagePoint};

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
pub(in crate::backend::wayland) use runtime::RegionCaptureRuntime;
mod selection_state;
mod source_guard;
mod window_snap;

#[cfg(test)]
use active_state::RegionInteractionPhase;
pub(super) use active_state::{ActiveScreenRegion, FreezeOwnership};
pub(in crate::backend::wayland) use board::{
    board_bounds_for_world_rect, world_rect_for_composed_region, world_rect_for_image_rect_exact,
};
pub(in crate::backend::wayland) use cut_review::RegionReviewPress;
#[cfg(test)]
pub(super) use events::finalize_region_selection_with_review_edits;
#[cfg(test)]
use events::{
    begin_region_selection_event, initial_square_modifier, region_owner_lost_event,
    sync_region_square_modifier_event, update_region_selection_event,
};
pub(in crate::backend::wayland) use geometry::RegionPickerMeasurement;
pub(in crate::backend::wayland) use intent::{RegionCaptureIntent, RegionPickerOptions};
#[cfg(test)]
use measure::{MeasureModeTransition, measure_anchor, measure_edge, measure_mode_transition};
pub(in crate::backend::wayland) use measure::{RegionOwnerLoss, RegionSelectionFinalize};
#[cfg(test)]
use review_state::resized_review_rect;
pub(super) use source_guard::owned_generation_is_current;
#[cfg(test)]
use source_guard::{
    active_eyedropper_source_changed, active_region_source_changed, screen_region_invariant,
};
pub(in crate::backend::wayland) use window_snap::WindowSnapDirection;

#[cfg(test)]
use super::screen_image::ScreenSourceToken;

#[cfg(test)]
mod tests;
