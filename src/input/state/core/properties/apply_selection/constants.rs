pub(super) const SELECTION_THICKNESS_STEP: f64 = 1.0;
pub(super) const SELECTION_FONT_SIZE_STEP: f64 = 2.0;
pub(super) const SELECTION_ARROW_LENGTH_STEP: f64 = 2.0;
pub(super) const SELECTION_ARROW_ANGLE_STEP: f64 = 2.0;
pub(super) const MIN_FONT_SIZE: f64 = 8.0;
pub(super) const MAX_FONT_SIZE: f64 = 72.0;
pub(super) const MIN_ARROW_LENGTH: f64 = 5.0;
pub(super) const MAX_ARROW_LENGTH: f64 = 50.0;
pub(super) const MIN_ARROW_ANGLE: f64 = 15.0;
pub(super) const MAX_ARROW_ANGLE: f64 = 60.0;
/// Opacity steps in 5% increments from 5%; a marker stays at or below 90%,
/// the marker tool's own ceiling.
pub(in crate::input::state::core::properties) const OPACITY_STEP: f64 = 0.05;
pub(in crate::input::state::core::properties) const MIN_OPACITY: f64 = 0.05;
pub(in crate::input::state::core::properties) const MAX_MARKER_OPACITY: f64 = 0.9;
