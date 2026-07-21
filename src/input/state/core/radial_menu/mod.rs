pub(crate) mod compass;
pub(crate) mod hit_test;
mod layout;
pub(crate) mod size_ring;
pub(crate) mod state;

use std::time::{Duration, Instant};

pub use compass::{
    COMPASS_SLICES, CompassDir, RadialParent, RadialSlice, RadialSliceKind, TOOL_SEGMENT_COUNT,
    compass_slice, slice_parent, sub_ring_child_count, sub_ring_children,
};
pub use size_ring::{
    SIZE_RING_ARC_SPAN, SIZE_RING_ARC_START, size_ring_angle_for_value, size_ring_value_for_angle,
};

/// How long an open radial menu stays unpainted. Layout and hit-testing are
/// live from the moment of opening so a press-flick-release commits without
/// the menu ever appearing; only painting waits out this window. This is an
/// interaction-latency feature, deliberately not gated on reduced-motion.
pub const RADIAL_PAINT_DELAY: Duration = Duration::from_millis(220);

/// State of the radial menu overlay.
#[derive(Debug, Clone, Default)]
pub enum RadialMenuState {
    /// Menu is not visible.
    #[default]
    Hidden,
    /// Menu is open at a given center position.
    Open {
        /// Center X in surface coordinates.
        center_x: f64,
        /// Center Y in surface coordinates.
        center_y: f64,
        /// Currently hovered segment (if any).
        hover: Option<RadialSegmentId>,
        /// Expanded sub-ring parent segment index (compass index:
        /// Shapes = 2 / E, Notes = 5 / SW).
        expanded_sub_ring: Option<u8>,
        /// When the menu opened; painting starts at
        /// `opened_at + RADIAL_PAINT_DELAY`.
        opened_at: Instant,
        /// Whether the menu has actually been painted. Blind (pre-paint)
        /// flick releases commit by direction only; sighted releases may
        /// commit the hovered sub-ring child or color swatch.
        painted: bool,
        /// Whether the pointer has left the center deadzone since opening.
        /// Armed flicks commit (or cancel) on toggle-button release; an
        /// unarmed release keeps the menu open (click-to-open mode).
        flick_armed: bool,
        /// Whether a size-ring drag is capturing pointer motion.
        size_dragging: bool,
    },
}

/// Identifies a segment in the radial menu for hit-testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialSegmentId {
    /// Primary compass-ring segment (index 0..TOOL_SEGMENT_COUNT, N = 0,
    /// clockwise).
    Tool(u8),
    /// Sub-ring child segment (parent index, child index).
    SubTool(u8, u8),
    /// Color ring segment (combined quick palette + recents index).
    Color(u8),
    /// Thin outermost thickness gauge band (drag along the arc).
    SizeRing,
    /// Center circle (dismiss).
    Center,
}

/// One swatch on the radial color ring: the quick palette first, then the
/// session's recent colors appended as a visually separated arc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadialRingSwatch {
    /// Swatch fill color.
    pub color: crate::draw::Color,
    /// True for the appended recent-color segments.
    pub recent: bool,
}

/// Cached layout metrics for the radial menu.
#[derive(Debug, Clone, Copy)]
pub struct RadialMenuLayout {
    /// Clamped center X.
    pub center_x: f64,
    /// Clamped center Y.
    pub center_y: f64,
    /// Radius of the center circle.
    pub center_radius: f64,
    /// Inner radius of the tool ring.
    pub tool_inner: f64,
    /// Outer radius of the tool ring.
    pub tool_outer: f64,
    /// Inner radius of the sub-ring band.
    pub sub_inner: f64,
    /// Outer radius of the sub-ring band.
    pub sub_outer: f64,
    /// Inner radius of the color ring.
    pub color_inner: f64,
    /// Outer radius of the color ring.
    pub color_outer: f64,
    /// Inner radius of the size (thickness gauge) ring.
    pub size_inner: f64,
    /// Outer radius of the size ring.
    pub size_outer: f64,
}
