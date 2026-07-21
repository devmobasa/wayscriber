//! Fixed compass slice table for the radial menu's primary ring.
//!
//! The compass is fixed forever by design: eight slices, N straight up,
//! clockwise, wedge centers exactly on N/NE/E/SE/S/SW/W/NW. Slices hold
//! [`Action`] references resolved through the `ActionMeta` registry at
//! render/dispatch time (label, icon, binding hint), so the ring can never
//! drift from the rest of the action surfaces. There are deliberately no
//! layout config keys; only `radial_menu_mouse_binding` exists.

use std::sync::OnceLock;

use crate::domain::Action;
use crate::input::tool::Tool;

/// Number of segments in the primary compass ring.
pub const TOOL_SEGMENT_COUNT: usize = 8;

/// Compass direction of a primary-ring slice. Declaration order is clockwise
/// from North, so `dir as u8` is the slice's segment index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompassDir {
    /// Straight up (wedge spans -22.5 deg..+22.5 deg around vertical).
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl CompassDir {
    /// All directions, clockwise from North; array position matches the
    /// primary-ring segment index used by hit-testing and rendering.
    pub const ALL: [CompassDir; TOOL_SEGMENT_COUNT] = [
        CompassDir::N,
        CompassDir::NE,
        CompassDir::E,
        CompassDir::SE,
        CompassDir::S,
        CompassDir::SW,
        CompassDir::W,
        CompassDir::NW,
    ];

    /// Primary-ring segment index of this direction.
    pub fn index(self) -> u8 {
        self as u8
    }
}

/// A sub-ring-owning parent slice on the compass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialParent {
    /// E: the shape family (children derived from the toolbar's
    /// `shape_tools()` catalog).
    Shapes,
    /// SW: numbered step markers and sticky notes.
    Notes,
}

impl RadialParent {
    /// Wedge label. Parents are not actions, so their labels live here
    /// rather than in the `ActionMeta` registry.
    pub fn label(self) -> &'static str {
        match self {
            RadialParent::Shapes => "Shapes",
            RadialParent::Notes => "Notes",
        }
    }

    /// Sub-ring children in slice order (child 0 sits at the parent wedge's
    /// leading edge).
    pub fn children(self) -> &'static [Action] {
        match self {
            RadialParent::Shapes => shapes_children(),
            RadialParent::Notes => &NOTES_CHILDREN,
        }
    }
}

/// SW sub-ring: step markers first, then sticky notes.
const NOTES_CHILDREN: [Action; 2] = [Action::SelectStepMarkerTool, Action::EnterStickyNoteMode];

/// E sub-ring, derived from the toolbar's `shape_tools()` order (the shapes
/// source of truth) filtered to the radial-eligible members: Arrow already
/// owns the SE compass slice, and the exotic polygon variants stay behind
/// the toolbar's shape picker — the Polygon wedge is the family's radial
/// entry point.
fn shapes_children() -> &'static [Action] {
    static CHILDREN: OnceLock<Vec<Action>> = OnceLock::new();
    CHILDREN.get_or_init(|| {
        crate::ui::toolbar::model::shape_tools()
            .iter()
            .filter(|tool| {
                matches!(
                    tool,
                    Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Blur | Tool::RegularPolygon
                )
            })
            .filter_map(|tool| tool.action())
            .collect()
    })
}

/// What a compass slice does when selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialSliceKind {
    /// Dispatches this action through `InputState::handle_action` and
    /// closes the menu.
    Action(Action),
    /// Expands a sub-ring of the parent's action children.
    Parent(RadialParent),
}

/// One fixed slice of the compass ring.
#[derive(Debug, Clone, Copy)]
pub struct RadialSlice {
    /// Fixed compass direction; `dir.index()` is the segment index.
    pub dir: CompassDir,
    /// What selecting the slice does.
    pub kind: RadialSliceKind,
}

/// The fixed compass: N Pen, NE Marker, E Shapes, SE Arrow, S Select,
/// SW Notes (step marker + sticky note), W Text, NW Eraser. Standalone Line
/// lives in the Shapes sub-ring; history/clear actions are not on the ring.
pub const COMPASS_SLICES: [RadialSlice; TOOL_SEGMENT_COUNT] = [
    RadialSlice {
        dir: CompassDir::N,
        kind: RadialSliceKind::Action(Action::SelectPenTool),
    },
    RadialSlice {
        dir: CompassDir::NE,
        kind: RadialSliceKind::Action(Action::SelectMarkerTool),
    },
    RadialSlice {
        dir: CompassDir::E,
        kind: RadialSliceKind::Parent(RadialParent::Shapes),
    },
    RadialSlice {
        dir: CompassDir::SE,
        kind: RadialSliceKind::Action(Action::SelectArrowTool),
    },
    RadialSlice {
        dir: CompassDir::S,
        kind: RadialSliceKind::Action(Action::SelectSelectionTool),
    },
    RadialSlice {
        dir: CompassDir::SW,
        kind: RadialSliceKind::Parent(RadialParent::Notes),
    },
    RadialSlice {
        dir: CompassDir::W,
        kind: RadialSliceKind::Action(Action::EnterTextMode),
    },
    RadialSlice {
        dir: CompassDir::NW,
        kind: RadialSliceKind::Action(Action::SelectEraserTool),
    },
];

/// Slice at a primary-ring segment index.
pub fn compass_slice(idx: u8) -> Option<&'static RadialSlice> {
    COMPASS_SLICES.get(idx as usize)
}

/// Parent at a segment index, if that slice expands a sub-ring.
pub fn slice_parent(idx: u8) -> Option<RadialParent> {
    match compass_slice(idx)?.kind {
        RadialSliceKind::Parent(parent) => Some(parent),
        RadialSliceKind::Action(_) => None,
    }
}

/// Sub-ring children for a segment index (empty for non-parent slices).
pub fn sub_ring_children(parent_idx: u8) -> &'static [Action] {
    slice_parent(parent_idx)
        .map(RadialParent::children)
        .unwrap_or(&[])
}

/// Number of sub-ring children for a segment index, or 0 if none.
pub fn sub_ring_child_count(parent_idx: u8) -> usize {
    sub_ring_children(parent_idx).len()
}
