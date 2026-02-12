pub(crate) mod hit_test;
mod layout;
pub(crate) mod state;

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
        /// Expanded sub-ring parent index (Shapes=4, Text=5).
        expanded_sub_ring: Option<u8>,
    },
}

/// Identifies a segment in the radial menu for hit-testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialSegmentId {
    /// Primary tool ring segment (index 0..TOOL_SEGMENT_COUNT).
    Tool(u8),
    /// Sub-ring child segment (parent index, child index).
    SubTool(u8, u8),
    /// Color ring segment (index 0..8).
    Color(u8),
    /// Center circle (dismiss).
    Center,
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
}

/// Number of segments in the primary tool ring.
pub const TOOL_SEGMENT_COUNT: usize = 9;
/// Number of segments in the color ring.
pub const COLOR_SEGMENT_COUNT: usize = 8;

/// Sub-ring children for the Shapes segment (index 4).
pub const SHAPES_CHILDREN: &[&str] = &["Rect", "Ellipse"];
/// Sub-ring children for the Text segment (index 5).
pub const TEXT_CHILDREN: &[&str] = &["Text", "Sticky", "Step"];

/// Tool labels for the primary ring (clockwise from top).
pub const TOOL_LABELS: [&str; TOOL_SEGMENT_COUNT] = [
    "Pen", "Marker", "Line", "Arrow", "Shapes", "Text", "Eraser", "Select", "Clear",
];
