use super::super::*;
use crate::backend::wayland::frozen_geometry::OutputGeometry;

pub(super) fn desktop_backdrop_output_geometry_from_info(
    info: &smithay_client_toolkit::output::OutputInfo,
) -> Option<DesktopBackdropOutputGeometry> {
    let (logical_x, logical_y) = info.logical_position?;
    let (logical_width, logical_height) = info.logical_size?;
    if logical_width <= 0 || logical_height <= 0 {
        return None;
    }
    let (physical_width, physical_height) = current_or_preferred_mode_size(info)
        .map(|(width, height)| transformed_output_size(width, height, info.transform))
        .or_else(|| {
            let scale = u32::try_from(info.scale_factor.max(1)).ok()?;
            Some((
                u32::try_from(logical_width).ok()?.checked_mul(scale)?,
                u32::try_from(logical_height).ok()?.checked_mul(scale)?,
            ))
        })?;
    if physical_width == 0 || physical_height == 0 {
        return None;
    }

    Some(DesktopBackdropOutputGeometry {
        logical_x,
        logical_y,
        logical_width: logical_width as u32,
        logical_height: logical_height as u32,
        physical_width,
        physical_height,
    })
}

fn current_or_preferred_mode_size(
    info: &smithay_client_toolkit::output::OutputInfo,
) -> Option<(u32, u32)> {
    info.modes
        .iter()
        .find(|mode| mode.current)
        .or_else(|| info.modes.iter().find(|mode| mode.preferred))
        .and_then(|mode| {
            Some((
                u32::try_from(mode.dimensions.0).ok()?,
                u32::try_from(mode.dimensions.1).ok()?,
            ))
        })
        .filter(|(width, height)| *width > 0 && *height > 0)
}

fn transformed_output_size(width: u32, height: u32, transform: wl_output::Transform) -> (u32, u32) {
    if matches!(
        transform,
        wl_output::Transform::_90
            | wl_output::Transform::_270
            | wl_output::Transform::Flipped90
            | wl_output::Transform::Flipped270
    ) {
        (height, width)
    } else {
        (width, height)
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn desktop_backdrop_geometry(
        &self,
    ) -> Option<DesktopBackdropGeometry> {
        self.desktop_backdrop_geometry_excluding(None)
    }

    pub(in crate::backend::wayland) fn desktop_backdrop_geometry_excluding(
        &self,
        exclude: Option<&wl_output::WlOutput>,
    ) -> Option<DesktopBackdropGeometry> {
        let output = self.surface.current_output()?;
        if exclude.is_some_and(|destroyed| destroyed == &output) {
            return None;
        }
        let active_info = self.output_state.info(&output)?;
        let active = desktop_backdrop_output_geometry_from_info(&active_info)?;
        let mut outputs = Vec::new();
        for candidate in self.output_state.outputs() {
            if exclude.is_some_and(|destroyed| destroyed == &candidate) {
                continue;
            }
            let info = self.output_state.info(&candidate)?;
            outputs.push(desktop_backdrop_output_geometry_from_info(&info)?);
        }

        DesktopBackdropGeometry::from_outputs(active, &outputs, active_info.scale_factor.max(1))
    }

    pub(in crate::backend::wayland) fn set_freeze_zoom_geometry(
        &mut self,
        geometry: Option<OutputGeometry>,
    ) {
        self.set_freeze_zoom_geometry_excluding(geometry, None);
    }

    pub(in crate::backend::wayland) fn set_freeze_zoom_geometry_excluding(
        &mut self,
        geometry: Option<OutputGeometry>,
        exclude: Option<&wl_output::WlOutput>,
    ) {
        let screenshot_origin = self
            .desktop_backdrop_geometry_excluding(exclude)
            .and_then(DesktopBackdropGeometry::physical_origin);
        let geometry = geometry.map(|geo| geo.with_screenshot_origin(screenshot_origin));
        self.frozen.set_active_geometry(geometry.clone());
        self.zoom.set_active_geometry(geometry);
    }

    pub(in crate::backend::wayland) fn refresh_freeze_zoom_screenshot_origin(&mut self) {
        self.refresh_freeze_zoom_screenshot_origin_excluding(None);
    }

    pub(in crate::backend::wayland) fn refresh_freeze_zoom_screenshot_origin_excluding(
        &mut self,
        exclude: Option<&wl_output::WlOutput>,
    ) {
        let Some(geometry) = self
            .frozen
            .active_geometry()
            .cloned()
            .or_else(|| self.zoom.active_geometry().cloned())
        else {
            return;
        };
        self.set_freeze_zoom_geometry_excluding(Some(geometry), exclude);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transformed_output_size_keeps_unrotated_transforms() {
        assert_eq!(
            transformed_output_size(3840, 2160, wl_output::Transform::Normal),
            (3840, 2160)
        );
        assert_eq!(
            transformed_output_size(3840, 2160, wl_output::Transform::_180),
            (3840, 2160)
        );
        assert_eq!(
            transformed_output_size(3840, 2160, wl_output::Transform::Flipped),
            (3840, 2160)
        );
        assert_eq!(
            transformed_output_size(3840, 2160, wl_output::Transform::Flipped180),
            (3840, 2160)
        );
    }

    #[test]
    fn transformed_output_size_swaps_rotated_transforms() {
        for transform in [
            wl_output::Transform::_90,
            wl_output::Transform::_270,
            wl_output::Transform::Flipped90,
            wl_output::Transform::Flipped270,
        ] {
            assert_eq!(transformed_output_size(3840, 2160, transform), (2160, 3840));
        }
    }
}
