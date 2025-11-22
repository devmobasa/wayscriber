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
    pub fn update_from(
        logical_pos: Option<(i32, i32)>,
        logical_size: Option<(i32, i32)>,
        fallback_size: (u32, u32),
        scale: i32,
    ) -> Option<Self> {
        let (lx, ly) = logical_pos.unwrap_or((0, 0));
        let (lw, lh) = logical_size.unwrap_or((fallback_size.0 as i32, fallback_size.1 as i32));
        if lw <= 0 || lh <= 0 || scale <= 0 {
            return None;
        }
        Some(Self {
            logical_x: lx,
            logical_y: ly,
            logical_width: lw as u32,
            logical_height: lh as u32,
            scale,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_from_uses_logical_and_scale() {
        let geo = OutputGeometry::update_from(Some((10, 20)), Some((1920, 1080)), (800, 600), 2)
            .expect("geometry");
        assert_eq!(geo.logical_x, 10);
        assert_eq!(geo.logical_y, 20);
        assert_eq!(geo.logical_width, 1920);
        assert_eq!(geo.logical_height, 1080);
        assert_eq!(geo.physical_size(), (3840, 2160));
        assert_eq!(geo.physical_origin(), (20, 40));
    }

    #[test]
    fn update_from_uses_fallback_when_missing_logical_size() {
        let geo = OutputGeometry::update_from(None, None, (800, 600), 1).expect("geometry");
        assert_eq!(geo.logical_width, 800);
        assert_eq!(geo.logical_height, 600);
    }

    #[test]
    fn update_from_rejects_invalid_scale_or_size() {
        assert!(OutputGeometry::update_from(None, Some((0, 600)), (800, 600), 1).is_none());
        assert!(OutputGeometry::update_from(None, Some((800, 0)), (800, 600), 1).is_none());
        assert!(OutputGeometry::update_from(None, None, (800, 600), 0).is_none());
    }
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
