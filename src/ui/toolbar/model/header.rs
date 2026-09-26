use std::borrow::Cow;

use crate::config::ToolbarLayoutMode;

use super::super::ToolbarEvent;
use super::activation::ToolbarControlId;
use super::control::{
    ToolbarControl, ToolbarControlKind, ToolbarControlPresentation, ToolbarControlRole,
    ToolbarPresentationPayload, ToolbarSegment, ToolbarSegmentedControl, ToolbarTooltip,
};

/// The preset's name as the toolbar shows it.
pub(crate) const fn layout_mode_label(mode: ToolbarLayoutMode) -> &'static str {
    match mode {
        ToolbarLayoutMode::Simple => "Simple",
        ToolbarLayoutMode::Regular => "Regular",
        ToolbarLayoutMode::Advanced => "Advanced",
    }
}

/// One line saying what the preset puts on the strip, shown under its name
/// in the layout menu. Kept short enough for a single line at menu width.
pub(crate) const fn layout_mode_description(mode: ToolbarLayoutMode) -> &'static str {
    match mode {
        ToolbarLayoutMode::Simple => "Core pens and one Shapes picker",
        ToolbarLayoutMode::Regular => "Adds Shape Pen, Laser, Line/Arrow, presets",
        ToolbarLayoutMode::Advanced => "Adds inline shapes and multi-step undo",
    }
}

/// Layout menu geometry in spec units, shared by both frontends so the menu
/// has the same footprint in each: the row width inside the panel padding,
/// one row's height (name line plus description line), the gap between
/// rows, and the panel padding.
pub(crate) const LAYOUT_MENU_ROW_W: f64 = 264.0;
pub(crate) const LAYOUT_MENU_ROW_H: f64 = 40.0;
pub(crate) const LAYOUT_MENU_ROW_GAP: f64 = 4.0;
pub(crate) const LAYOUT_MENU_PAD: f64 = 6.0;

/// One row of the chrome island's layout-preset menu.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutMenuEntry {
    pub(crate) mode: ToolbarLayoutMode,
    pub(crate) label: &'static str,
    pub(crate) description: &'static str,
    /// The preset on screen now; the menu marks it.
    pub(crate) current: bool,
    pub(crate) event: ToolbarEvent,
}

/// The layout menu's rows, simplest preset first. Both frontends render
/// exactly these rows so their wording and order cannot drift.
pub(crate) fn layout_menu_entries(current: ToolbarLayoutMode) -> [LayoutMenuEntry; 3] {
    ToolbarLayoutMode::ALL.map(|mode| LayoutMenuEntry {
        mode,
        label: layout_mode_label(mode),
        description: layout_mode_description(mode),
        current: mode == current,
        event: ToolbarEvent::SetToolbarLayoutMode(mode),
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_menu_lists_every_preset_once_and_marks_the_current_one() {
        for current in ToolbarLayoutMode::ALL {
            let entries = layout_menu_entries(current);

            let modes: Vec<_> = entries.iter().map(|entry| entry.mode).collect();
            assert_eq!(modes, ToolbarLayoutMode::ALL);
            let marked: Vec<_> = entries
                .iter()
                .filter(|entry| entry.current)
                .map(|entry| entry.mode)
                .collect();
            assert_eq!(marked, vec![current]);
            for entry in &entries {
                assert_eq!(entry.event, ToolbarEvent::SetToolbarLayoutMode(entry.mode));
                assert!(!entry.description.is_empty());
            }
        }
    }

    /// Descriptions must stay one line at the menu's width in both frontends.
    #[test]
    fn layout_menu_descriptions_stay_short() {
        for mode in ToolbarLayoutMode::ALL {
            assert!(
                layout_mode_description(mode).chars().count() <= 42,
                "{mode:?}"
            );
        }
    }
}
