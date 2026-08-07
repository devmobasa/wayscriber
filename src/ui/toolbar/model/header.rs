use std::borrow::Cow;

use crate::config::ToolbarLayoutMode;

use super::super::ToolbarEvent;
use super::activation::ToolbarControlId;
use super::control::{
    ToolbarControl, ToolbarControlKind, ToolbarControlPresentation, ToolbarControlRole,
    ToolbarPresentationPayload, ToolbarSegment, ToolbarSegmentedControl, ToolbarTooltip,
};

pub(crate) fn layout_mode_control(mode: ToolbarLayoutMode) -> ToolbarControl {
    let segment = |id, label: &'static str, target: ToolbarLayoutMode, tooltip: &'static str| {
        ToolbarSegment {
            id,
            label: Cow::Borrowed(label),
            activation: ToolbarEvent::SetToolbarLayoutMode(target),
            action: None,
            tooltip: ToolbarTooltip::text(tooltip),
            enabled: true,
        }
    };
    // Modes are non-destructive presets: switching changes the baseline,
    // never the user's explicit section overrides.
    let segments = vec![
        segment(
            ToolbarControlId::LayoutModeSimple,
            "Simple",
            ToolbarLayoutMode::Simple,
            "Simple preset",
        ),
        segment(
            ToolbarControlId::LayoutModeRegular,
            "Regular",
            ToolbarLayoutMode::Regular,
            "Regular preset",
        ),
        segment(
            ToolbarControlId::LayoutModeAdvanced,
            "Advanced",
            ToolbarLayoutMode::Advanced,
            "Advanced preset",
        ),
    ];
    let active = match mode {
        ToolbarLayoutMode::Simple => ToolbarControlId::LayoutModeSimple,
        ToolbarLayoutMode::Regular => ToolbarControlId::LayoutModeRegular,
        ToolbarLayoutMode::Advanced => ToolbarControlId::LayoutModeAdvanced,
    };
    segmented_control(
        ToolbarControlId::LayoutModeSimple,
        active,
        "Toolbar layout",
        segments,
    )
}

fn segmented_control(
    id: ToolbarControlId,
    active: ToolbarControlId,
    label: &'static str,
    segments: Vec<ToolbarSegment>,
) -> ToolbarControl {
    ToolbarControl {
        id,
        kind: ToolbarControlKind::Segmented(
            ToolbarSegmentedControl::try_new(Some(active), segments)
                .expect("static segmented toolbar control is valid"),
        ),
        enabled: true,
        active: true,
        presentation: ToolbarControlPresentation {
            label: Cow::Borrowed(label),
            tooltip: ToolbarTooltip::None,
            icon: None,
            role: ToolbarControlRole::Segmented,
            payload: ToolbarPresentationPayload::None,
        },
    }
}
