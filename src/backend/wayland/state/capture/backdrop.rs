use super::super::*;

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
