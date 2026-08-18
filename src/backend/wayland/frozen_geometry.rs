use wayland_client::protocol::wl_output;

use crate::capture::DesktopBackdropGeometry;

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
    /// Integer-scale buffer dimensions of the overlay surface. These are
    /// independent of the output's logical/native size on xdg-shell fallbacks.
    pub(super) overlay_buffer_size: (u32, u32),
    /// Physical pixels belonging to this output after applying its transform.
    /// This may differ from `logical * scale` under fractional scaling.
    pub(super) pixel_size: Option<(u32, u32)>,
    /// Origin of this output inside a full-desktop screenshot, walking every
    /// known output so mixed-DPI layouts are not `logical * this_output.scale`.
    pub screenshot_origin: Option<(u32, u32)>,
    /// Expected full-desktop screenshot dimensions for the captured layout.
    pub(super) screenshot_size: Option<(u32, u32)>,
}

impl OutputGeometry {
    pub fn update_from(
        logical_pos: Option<(i32, i32)>,
        logical_size: Option<(i32, i32)>,
        fallback_size: (u32, u32),
        scale: i32,
        transform: wl_output::Transform,
        pixel_size: Option<(u32, u32)>,
    ) -> Option<Self> {
        let (lx, ly) = logical_pos.unwrap_or((0, 0));
        let fallback_width = i32::try_from(fallback_size.0).ok()?;
        let fallback_height = i32::try_from(fallback_size.1).ok()?;
        let (lw, lh) = logical_size.unwrap_or((fallback_width, fallback_height));
        if lw <= 0 || lh <= 0 || scale <= 0 {
            return None;
        }
        let buffer_scale = u32::try_from(scale).ok()?;
        let overlay_buffer_size = (
            fallback_size.0.checked_mul(buffer_scale)?,
            fallback_size.1.checked_mul(buffer_scale)?,
        );
        if overlay_buffer_size.0 == 0 || overlay_buffer_size.1 == 0 {
            return None;
        }
        if pixel_size.is_some_and(|(width, height)| width == 0 || height == 0) {
            return None;
        }
        Some(Self {
            logical_x: lx,
            logical_y: ly,
            logical_width: lw as u32,
            logical_height: lh as u32,
            scale,
            transform,
            overlay_buffer_size,
            pixel_size,
            screenshot_origin: None,
            screenshot_size: None,
        })
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::capture::DesktopBackdropGeometry;

    #[test]
    fn update_from_uses_logical_and_scale() {
        let geo = OutputGeometry::update_from(
            Some((10, 20)),
            Some((1920, 1080)),
            (800, 600),
            2,
            wl_output::Transform::_270,
            Some((3840, 2160)),
        )
        .expect("geometry");
        assert_eq!(geo.logical_x, 10);
        assert_eq!(geo.logical_y, 20);
        assert_eq!(geo.logical_width, 1920);
        assert_eq!(geo.logical_height, 1080);
        assert_eq!(geo.transform, wl_output::Transform::_270);
        assert_eq!(geo.verified_pixel_size(), Some((3840, 2160)));
        assert_eq!(geo.buffer_size(), Some((1600, 1200)));
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
        let geo = OutputGeometry::update_from(
            None,
            None,
            (800, 600),
            1,
            wl_output::Transform::Normal,
            None,
        )
        .expect("geometry");
        assert_eq!(geo.logical_width, 800);
        assert_eq!(geo.logical_height, 600);
        assert_eq!(geo.verified_pixel_size(), None);
        assert_eq!(geo.portal_crop_origin(800, 600), None);
    }

    #[test]
    fn update_from_preserves_fractional_output_pixel_size() {
        let geo = OutputGeometry::update_from(
            Some((0, 0)),
            Some((2048, 1152)),
            (2048, 1152),
            2,
            wl_output::Transform::Normal,
            Some((3200, 1800)),
        )
        .expect("fractional output geometry");

        assert_eq!(geo.verified_pixel_size(), Some((3200, 1800)));
    }

    #[test]
    fn buffer_aspect_allows_rounding_but_rejects_a_different_viewport() {
        assert!(OutputGeometry::dimensions_have_compatible_aspect(
            (5, 3),
            (6, 4)
        ));
        assert!(!OutputGeometry::dimensions_have_compatible_aspect(
            (3200, 1800),
            (3200, 1760)
        ));
    }

    #[test]
    fn desktop_backdrop_geometry_supplies_the_portal_output_pixel_size() {
        let geo = OutputGeometry::update_from(
            Some((0, 0)),
            Some((3, 2)),
            (3, 2),
            2,
            wl_output::Transform::Normal,
            None,
        )
        .expect("base geometry")
        .with_desktop_backdrop_geometry(Some(DesktopBackdropGeometry {
            logical_x: 0,
            logical_y: 0,
            logical_width: 3,
            logical_height: 2,
            scale: 2,
            physical_width: Some(5),
            physical_height: Some(3),
            crop_x: Some(0),
            crop_y: Some(0),
            screenshot_width: Some(5),
            screenshot_height: Some(3),
        }));

        assert_eq!(geo.verified_pixel_size(), Some((5, 3)));
    }

    #[test]
    fn unavailable_desktop_layout_clears_stale_screenshot_bounds() {
        let geo = OutputGeometry::update_from(
            Some((0, 0)),
            Some((3, 2)),
            (3, 2),
            2,
            wl_output::Transform::Normal,
            Some((5, 3)),
        )
        .expect("base geometry")
        .with_desktop_backdrop_geometry(Some(DesktopBackdropGeometry {
            logical_x: 0,
            logical_y: 0,
            logical_width: 3,
            logical_height: 2,
            scale: 2,
            physical_width: Some(5),
            physical_height: Some(3),
            crop_x: Some(7),
            crop_y: Some(0),
            screenshot_width: Some(12),
            screenshot_height: Some(3),
        }))
        .with_desktop_backdrop_geometry(None);

        assert_eq!(geo.verified_pixel_size(), Some((5, 3)));
        assert_eq!(geo.portal_crop_origin(12, 3), None);
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
                None,
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
                None,
            )
            .is_none()
        );
        assert!(
            OutputGeometry::update_from(
                None,
                None,
                (800, 600),
                0,
                wl_output::Transform::Normal,
                None,
            )
            .is_none()
        );
    }
}

impl OutputGeometry {
    /// Returns compositor-reported output pixels without guessing from an
    /// integer buffer scale.
    pub fn verified_pixel_size(&self) -> Option<(u32, u32)> {
        self.pixel_size
    }

    /// Returns whether scaling source pixels into a buffer preserves aspect,
    /// allowing at most one-pixel rounding from fractional-scale dimensions.
    pub fn dimensions_have_compatible_aspect(source: (u32, u32), target: (u32, u32)) -> bool {
        if source.0 == 0 || source.1 == 0 || target.0 == 0 || target.1 == 0 {
            return false;
        }
        let horizontal = u64::from(source.0) * u64::from(target.1);
        let vertical = u64::from(source.1) * u64::from(target.0);
        let rounding_tolerance = u64::from(source.0.max(source.1).max(target.0).max(target.1));
        horizontal.abs_diff(vertical) <= rounding_tolerance
    }

    /// Returns the integer-scale buffer dimensions used by the overlay surface.
    pub fn buffer_size(&self) -> Option<(u32, u32)> {
        Some(self.overlay_buffer_size)
    }

    /// Validate post-transform pixels when the compositor's physical mode is known.
    pub fn accepts_transformed_pixel_size(&self, width: u32, height: u32) -> bool {
        self.pixel_size
            .is_none_or(|pixel_size| pixel_size == (width, height))
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

    #[cfg(test)]
    fn with_screenshot_origin(mut self, origin: Option<(u32, u32)>) -> Self {
        self.screenshot_origin = origin;
        self
    }

    pub fn with_desktop_backdrop_geometry(
        mut self,
        geometry: Option<DesktopBackdropGeometry>,
    ) -> Self {
        self.screenshot_origin = None;
        self.screenshot_size = None;
        if let Some(geometry) = geometry {
            self.pixel_size = geometry.verified_physical_size();
            self.screenshot_origin = geometry.physical_origin();
            self.screenshot_size = geometry.screenshot_size();
        }
        self
    }

    /// Crop origin inside a portal/desktop screenshot of `image_width` ×
    /// `image_height`.
    ///
    /// Unknown origin is `(0, 0)` only when those dimensions are this output's
    /// physical size (a single-output capture). Otherwise the origin stays
    /// unknown so a multi-output shot is not cropped from the first monitor.
    pub fn portal_crop_origin(&self, image_width: u32, image_height: u32) -> Option<(u32, u32)> {
        if self
            .screenshot_size
            .is_some_and(|size| size != (image_width, image_height))
        {
            return None;
        }
        if let Some(origin) = self.screenshot_origin {
            return Some(origin);
        }
        let (phys_width, phys_height) = self.verified_pixel_size()?;
        (image_width == phys_width && image_height == phys_height).then_some((0, 0))
    }
}
