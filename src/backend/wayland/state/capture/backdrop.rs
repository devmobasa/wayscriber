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
    let (physical_width, physical_height) = current_mode_size(info)
        .map(|(width, height)| transformed_output_size(width, height, info.transform))?;
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

fn current_mode_size(info: &smithay_client_toolkit::output::OutputInfo) -> Option<(u32, u32)> {
    info.modes
        .iter()
        .find(|mode| mode.current)
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
        let backdrop_geometry = self.desktop_backdrop_geometry_excluding(exclude);
        let geometry = geometry.map(|geo| geo.with_desktop_backdrop_geometry(backdrop_geometry));
        self.frozen.set_active_geometry(geometry.clone());
        self.zoom.set_active_geometry(geometry);
    }

    pub(in crate::backend::wayland) fn refresh_freeze_zoom_screenshot_origin(&mut self) {
        self.refresh_freeze_zoom_geometry_excluding(None);
    }

    pub(in crate::backend::wayland) fn refresh_freeze_zoom_screenshot_origin_excluding(
        &mut self,
        exclude: Option<&wl_output::WlOutput>,
    ) {
        self.refresh_freeze_zoom_geometry_excluding(exclude);
    }

    pub(in crate::backend::wayland) fn refresh_freeze_zoom_geometry(&mut self) {
        self.refresh_freeze_zoom_geometry_excluding(None);
    }

    fn refresh_freeze_zoom_geometry_excluding(&mut self, exclude: Option<&wl_output::WlOutput>) {
        let Some(output) = self.surface.current_output() else {
            self.set_freeze_zoom_geometry_excluding(None, exclude);
            self.frozen.set_active_output(None, None);
            self.zoom.set_active_output(None, None);
            return;
        };
        if exclude.is_some_and(|destroyed| destroyed == &output) {
            self.set_freeze_zoom_geometry_excluding(None, exclude);
            self.frozen.set_active_output(None, None);
            self.zoom.set_active_output(None, None);
            return;
        }
        let Some(info) = self.output_state.info(&output) else {
            self.set_freeze_zoom_geometry_excluding(None, exclude);
            self.frozen.set_active_output(None, None);
            self.zoom.set_active_output(None, None);
            return;
        };

        let pixel_size = current_mode_size(&info)
            .map(|(width, height)| transformed_output_size(width, height, info.transform));
        let geometry = OutputGeometry::update_from(
            info.logical_position,
            info.logical_size,
            (self.surface.width(), self.surface.height()),
            self.surface.scale(),
            info.transform,
            pixel_size,
        );
        self.set_freeze_zoom_geometry_excluding(geometry, exclude);
        self.frozen
            .set_active_output(Some(output.clone()), Some(info.id));
        self.zoom.set_active_output(Some(output), Some(info.id));
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
