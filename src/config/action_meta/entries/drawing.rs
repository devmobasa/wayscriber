use super::ActionMeta;

pub const ENTRIES: &[ActionMeta] = &[
    meta!(
        IncreaseThickness,
        "Increase Thickness",
        None,
        "Make strokes thicker",
        Drawing,
        true,
        true,
        false
    ),
    meta!(
        DecreaseThickness,
        "Decrease Thickness",
        None,
        "Make strokes thinner",
        Drawing,
        true,
        true,
        false
    ),
    meta!(
        IncreaseMarkerOpacity,
        "Increase Marker Opacity",
        None,
        "Increase marker opacity",
        Drawing,
        false,
        false,
        false
    ),
    meta!(
        DecreaseMarkerOpacity,
        "Decrease Marker Opacity",
        None,
        "Decrease marker opacity",
        Drawing,
        false,
        false,
        false
    ),
    meta!(
        IncreaseFontSize,
        "Increase Font Size",
        None,
        "Make text larger",
        Drawing,
        false,
        true,
        false
    ),
    meta!(
        DecreaseFontSize,
        "Decrease Font Size",
        None,
        "Make text smaller",
        Drawing,
        false,
        true,
        false
    ),
    meta!(
        ResetArrowLabelCounter,
        "Reset Arrow Labels",
        None,
        "Reset arrow label counter",
        Drawing,
        false,
        false,
        false
    ),
    meta!(
        ResetStepMarkerCounter,
        "Reset Step Markers",
        None,
        "Reset step marker counter",
        Drawing,
        false,
        false,
        false
    ),
    meta!(
        ToggleFill,
        "Toggle Fill",
        Some("Fill"),
        "Enable/disable shape fill",
        Drawing,
        true,
        true,
        true
    ),
];
