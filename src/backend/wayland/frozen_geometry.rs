/// Geometry and scale details for the active output, used for cropping fallback captures.
#[derive(Clone, Debug)]
pub struct OutputGeometry {
    pub logical_x: i32,
    pub logical_y: i32,
    pub logical_width: u32,
    pub logical_height: u32,
    pub scale: i32,
}

impl OutputGeometry {
    /// Returns physical pixel dimensions.
    pub fn physical_size(&self) -> (u32, u32) {
        (
            self.logical_width.saturating_mul(self.scale as u32),
            self.logical_height.saturating_mul(self.scale as u32),
        )
    }

    /// Returns physical pixel origin.
    pub fn physical_origin(&self) -> (i32, i32) {
        (
            self.logical_x.saturating_mul(self.scale),
            self.logical_y.saturating_mul(self.scale),
        )
    }
}
