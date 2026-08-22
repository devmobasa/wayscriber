use crate::backend::wayland::acquisition::ScreenAcquisitionId;
use crate::input::state::{
    RegionInputSource, RegionPurposeTag, RegionSelectUiState, RegionSelection, ScreenCaptureSource,
};
use crate::screen_pixels::{ImagePixelRect, ImagePoint, clamp_edge};

mod active_state;
mod board;
mod delivery;
mod events;
mod geometry;
mod intent;
mod measure;
mod picker;
mod review_state;
mod runtime;
mod selection_state;
mod source_guard;

pub(super) use active_state::{ActiveScreenRegion, FreezeOwnership};
pub(in crate::backend::wayland) use board::{
    world_rect_for_image_rect_exact, world_rect_for_screen_rect,
};
pub(super) use events::finalize_region_selection_event;
use events::*;
pub(in crate::backend::wayland) use geometry::{RegionPickerMeasurement, RegionSelectionGeometry};
pub(in crate::backend::wayland) use intent::{RegionCaptureIntent, RegionPickerOptions};
use measure::*;
pub(in crate::backend::wayland) use measure::{RegionOwnerLoss, RegionSelectionFinalize};
pub(super) use source_guard::owned_generation_is_current;
use source_guard::*;

use super::WaylandState;
use super::screen_image::{
    ScreenSourceToken, current_screen_source_token, image_point_for_screen_point, screen_source_is,
};

#[cfg(test)]
mod tests;
