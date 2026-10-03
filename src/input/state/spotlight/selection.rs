//! Spotlight queries for selection controls.
use super::InputState;
use crate::draw::{DrawnShape, Shape};

impl InputState {
    /// Highest normalized magnification among selected Spotlights, if any.
    ///
    /// `None` when the selection holds no Spotlight at all. The selection
    /// control reports availability against this rather than the next-shape
    /// default, which differs when an existing shape is selected.
    pub fn selection_spotlight_magnification(&self) -> Option<f64> {
        Self::resolved_selection_spotlight_magnification(&self.resolved_selected_shapes())
    }

    /// Same query over a selection already resolved by the caller.
    pub(crate) fn resolved_selection_spotlight_magnification(
        selected: &[&DrawnShape],
    ) -> Option<f64> {
        selected
            .iter()
            .filter_map(|drawn| match drawn.shape {
                Shape::Spotlight { magnification, .. } => Some(
                    crate::draw::normalize_spotlight_magnification(magnification),
                ),
                _ => None,
            })
            .reduce(f64::max)
    }
}
