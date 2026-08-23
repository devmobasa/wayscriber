use super::*;

pub(super) fn measure_anchor(point: (f64, f64), bounds: (u32, u32)) -> Option<(f64, f64)> {
    if !point.0.is_finite() || !point.1.is_finite() || bounds.0 == 0 || bounds.1 == 0 {
        return None;
    }
    Some((
        point.0.floor().clamp(0.0, f64::from(bounds.0 - 1)),
        point.1.floor().clamp(0.0, f64::from(bounds.1 - 1)),
    ))
}

pub(super) fn measure_edge(
    anchor: (f64, f64),
    point: (f64, f64),
    bounds: (u32, u32),
) -> Option<(f64, f64)> {
    if !point.0.is_finite() || !point.1.is_finite() {
        return None;
    }
    let edge = |origin: f64, value: f64| {
        if value >= origin {
            value.ceil()
        } else {
            value.floor()
        }
    };
    Some((
        edge(anchor.0, point.0).clamp(0.0, f64::from(bounds.0)),
        edge(anchor.1, point.1).clamp(0.0, f64::from(bounds.1)),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MeasureModeTransition {
    Start,
    Cancel,
    Refuse,
}

pub(super) fn measure_mode_transition(
    purpose: Option<RegionPurposeTag>,
    screen_modal_engaged: bool,
) -> MeasureModeTransition {
    if purpose == Some(RegionPurposeTag::Measure) {
        MeasureModeTransition::Cancel
    } else if screen_modal_engaged {
        MeasureModeTransition::Refuse
    } else {
        MeasureModeTransition::Start
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum RegionSelectionFinalize {
    NotOwned,
    Rearmed,
    Reviewed,
    Measured,
    Selected {
        purpose: RegionPurposeTag,
        rect: ImagePixelRect,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum RegionOwnerLoss {
    NotOwned,
    Rearmed,
    Cancel(RegionPurposeTag),
}
