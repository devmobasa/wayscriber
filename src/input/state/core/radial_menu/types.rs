/// Tracks the radial menu lifecycle.
#[derive(Debug, Clone, Default)]
pub enum RadialMenuState {
    #[default]
    Hidden,
    Open {
        /// Screen position where menu was opened (middle-click location)
        anchor: (i32, i32),
        /// Currently hovered segment index (None if in center or outside)
        hover_index: Option<usize>,
        /// Number of preset slots available
        slot_count: usize,
    },
}

/// Layout metadata for rendering and hit-testing the radial menu.
#[derive(Debug, Clone, Copy)]
pub struct RadialMenuLayout {
    pub center_x: f64,
    pub center_y: f64,
    pub inner_radius: f64,
    pub outer_radius: f64,
    pub segment_count: usize,
    /// Angle offset to rotate the menu (first segment at top)
    pub start_angle: f64,
}

/// Inner radius of the radial menu (cancel zone)
pub const RADIAL_INNER_RADIUS: f64 = 35.0;

/// Outer radius of the radial menu
pub const RADIAL_OUTER_RADIUS: f64 = 100.0;
