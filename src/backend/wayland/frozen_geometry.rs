use wayland_client::protocol::wl_output;

/// Geometry and scale details for the active output, used for cropping fallback captures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputGeometry {
    #[allow(dead_code)] // part of the output snapshot; tests read origin via physical_origin
    pub logical_x: i32,
    #[allow(dead_code)] // part of the output snapshot; tests read origin via physical_origin
    pub logical_y: i32,
    pub logical_width: u32,
    pub logical_height: u32,
    pub scale: i32,
    pub transform: wl_output::Transform,
    /// Origin of this output inside a full-desktop screenshot, walking every
    /// known output so mixed-DPI layouts are not `logical * this_output.scale`.
    pub screenshot_origin: Option<(u32, u32)>,
}

impl OutputGeometry {
    pub fn update_from(
        logical_pos: Option<(i32, i32)>,
        logical_size: Option<(i32, i32)>,
        fallback_size: (u32, u32),
        scale: i32,
        transform: wl_output::Transform,
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
            transform,
            screenshot_origin: None,
        })
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn update_from_uses_logical_and_scale() {
        let geo = OutputGeometry::update_from(
            Some((10, 20)),
            Some((1920, 1080)),
            (800, 600),
            2,
            wl_output::Transform::_270,
        )
        .expect("geometry");
        assert_eq!(geo.logical_x, 10);
        assert_eq!(geo.logical_y, 20);
        assert_eq!(geo.logical_width, 1920);
        assert_eq!(geo.logical_height, 1080);
        assert_eq!(geo.transform, wl_output::Transform::_270);
        assert_eq!(geo.physical_size(), (3840, 2160));
        assert_eq!(geo.physical_origin(), (20, 40));
        assert_eq!(geo.portal_crop_origin(3840, 2160), Some((0, 0)));
        assert_eq!(geo.portal_crop_origin(8000, 2160), None);
        assert_eq!(
            geo.with_screenshot_origin(Some((6, 0)))
                .portal_crop_origin(8000, 2160),
            Some((6, 0))
        );
    }

    #[test]
    fn update_from_uses_fallback_when_missing_logical_size() {
        let geo =
            OutputGeometry::update_from(None, None, (800, 600), 1, wl_output::Transform::Normal)
                .expect("geometry");
        assert_eq!(geo.logical_width, 800);
        assert_eq!(geo.logical_height, 600);
    }

    #[test]
    fn update_from_rejects_invalid_scale_or_size() {
        assert!(
            OutputGeometry::update_from(
                None,
                Some((0, 600)),
                (800, 600),
                1,
                wl_output::Transform::Normal,
            )
            .is_none()
        );
        assert!(
            OutputGeometry::update_from(
                None,
                Some((800, 0)),
                (800, 600),
                1,
                wl_output::Transform::Normal,
            )
            .is_none()
        );
        assert!(
            OutputGeometry::update_from(None, None, (800, 600), 0, wl_output::Transform::Normal)
                .is_none()
        );
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

    /// Returns physical pixel origin of the logical position on this output.
    ///
    /// Portal desktop screenshots must use [`Self::portal_crop_origin`] instead:
    /// mixed-scale layouts are not `logical * this output's scale`, and an
    /// unknown origin must not be treated as `(0, 0)` unless the capture is
    /// already this output's physical size.
    #[allow(dead_code)] // used by geometry tests; portal crop uses screenshot_origin
    pub fn physical_origin(&self) -> (i32, i32) {
        (
            self.logical_x.saturating_mul(self.scale),
            self.logical_y.saturating_mul(self.scale),
        )
    }

    pub fn with_screenshot_origin(mut self, origin: Option<(u32, u32)>) -> Self {
        self.screenshot_origin = origin;
        self
    }

    /// Crop origin inside a portal/desktop screenshot of `image_width` ×
    /// `image_height`.
    ///
    /// Unknown origin is `(0, 0)` only when those dimensions are this output's
    /// physical size (a single-output capture). Otherwise the origin stays
    /// unknown so a multi-output shot is not cropped from the first monitor.
    pub fn portal_crop_origin(&self, image_width: u32, image_height: u32) -> Option<(u32, u32)> {
        if let Some(origin) = self.screenshot_origin {
            return Some(origin);
        }
        let (phys_width, phys_height) = self.physical_size();
        (image_width == phys_width && image_height == phys_height).then_some((0, 0))
    }
}
