//! Verified mapping from a portal desktop raster into one output's native pixels.
use crate::capture::desktop_raster::{LogicalDesktop, RasterCrop};
use crate::capture::{CaptureError, DesktopBackdropOutputGeometry};
use crate::screen_pixels::ScreenImage;

use super::frozen_geometry::OutputGeometry;
use super::portal_capture::crop_argb;

pub(super) fn crop_portal_raster(
    data: &[u8],
    width: u32,
    height: u32,
    geometry: &OutputGeometry,
) -> Result<ScreenImage, CaptureError> {
    let error = |message: &str| CaptureError::ImageError(message.to_string());
    let expected_len = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok());
    if width == 0 || height == 0 || expected_len != Some(data.len()) {
        return Err(error("Portal capture buffer does not match its dimensions"));
    }

    let target = geometry
        .verified_pixel_size()
        .ok_or_else(|| error("Portal capture output dimensions are unavailable"))?;
    if !OutputGeometry::dimensions_have_compatible_aspect(target, geometry.buffer_size()) {
        return Err(error(
            "Portal capture aspect does not match the overlay surface",
        ));
    }

    let packed = geometry
        .portal_crop_origin(width, height)
        .map(|(x, y)| RasterCrop {
            x,
            y,
            width: target.0,
            height: target.1,
        })
        .filter(|crop| contains_crop((width, height), *crop));

    let desktop = geometry
        .portal_outputs
        .as_deref()
        .and_then(LogicalDesktop::from_outputs);
    let active = DesktopBackdropOutputGeometry {
        logical_x: geometry.logical_x,
        logical_y: geometry.logical_y,
        logical_width: geometry.logical_width,
        logical_height: geometry.logical_height,
        physical_width: target.0,
        physical_height: target.1,
    };
    let uniform = desktop
        .as_ref()
        .filter(|_| geometry.active_output_is_in_portal_snapshot())
        .and_then(|desktop| {
            desktop.crop(&active, (width, height), geometry.uniform_desktop_density)
        });
    let crop = match (packed, uniform) {
        (Some(packed), Some(uniform)) if packed != uniform => None,
        (Some(crop), _) | (_, Some(crop)) => Some(crop),
        _ => None,
    };

    log::info!(
        "portal.raster image={}x{} packed_expected={:?} logical_desktop={:?} uniform_density={:?} crop={:?} target={:?}",
        width,
        height,
        geometry.screenshot_size,
        desktop,
        geometry.uniform_desktop_density,
        crop,
        target,
    );

    let crop = crop.ok_or_else(|| {
        error("Portal capture does not match an unambiguous active output layout")
    })?;
    if !contains_crop((width, height), crop) {
        return Err(error("Portal capture does not contain the active output"));
    }

    let (_, _, data) = crop_argb(data, width, height, crop.x, crop.y, crop.width, crop.height)
        .ok_or_else(|| error("Portal capture crop is invalid"))?;

    resample(data, (crop.width, crop.height), target)
}

fn contains_crop(raster: (u32, u32), crop: RasterCrop) -> bool {
    crop.width > 0
        && crop.height > 0
        && crop
            .x
            .checked_add(crop.width)
            .is_some_and(|edge| edge <= raster.0)
        && crop
            .y
            .checked_add(crop.height)
            .is_some_and(|edge| edge <= raster.1)
}

fn resample(
    data: Vec<u8>,
    source: (u32, u32),
    target: (u32, u32),
) -> Result<ScreenImage, CaptureError> {
    let error = |message: &str| CaptureError::ImageError(message.to_string());
    let dimension =
        |value| i32::try_from(value).map_err(|_| error("Portal image dimension is too large"));
    let stride = dimension(
        target
            .0
            .checked_mul(4)
            .ok_or_else(|| error("Portal image stride overflow"))?,
    )?;
    if source == target {
        return Ok(ScreenImage {
            width: target.0,
            height: target.1,
            stride,
            data,
        });
    }

    // Cairo filters premultiplied ARGB directly, avoiding alpha/color corruption.
    let map_error = |err: cairo::Error| {
        CaptureError::ImageError(format!("Portal image resampling failed: {err}"))
    };
    let src = cairo::ImageSurface::create_for_data(
        data,
        cairo::Format::ARgb32,
        dimension(source.0)?,
        dimension(source.1)?,
        dimension(
            source
                .0
                .checked_mul(4)
                .ok_or_else(|| error("Portal image stride overflow"))?,
        )?,
    )
    .map_err(map_error)?;
    let mut dst = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        dimension(target.0)?,
        dimension(target.1)?,
    )
    .map_err(map_error)?;

    {
        let context = cairo::Context::new(&dst).map_err(map_error)?;
        context.scale(
            f64::from(target.0) / f64::from(source.0),
            f64::from(target.1) / f64::from(source.1),
        );
        context
            .set_source_surface(&src, 0.0, 0.0)
            .map_err(map_error)?;
        context.source().set_filter(cairo::Filter::Bilinear);
        context.source().set_extend(cairo::Extend::Pad);
        context.set_operator(cairo::Operator::Source);
        context.paint().map_err(map_error)?;
    }

    let data = dst
        .data()
        .map_err(|err| CaptureError::ImageError(format!("Portal image pixels unavailable: {err}")))?
        .to_vec();

    Ok(ScreenImage {
        width: target.0,
        height: target.1,
        stride,
        data,
    })
}

#[cfg(test)]
mod tests {
    use crate::capture::DesktopBackdropGeometry;
    use crate::capture::desktop_raster::UniformDesktopDensity;

    use super::*;
    use wayland_client::protocol::wl_output;

    fn output(
        x: i32,
        y: i32,
        logical: (u32, u32),
        native: (u32, u32),
    ) -> DesktopBackdropOutputGeometry {
        DesktopBackdropOutputGeometry {
            logical_x: x,
            logical_y: y,
            logical_width: logical.0,
            logical_height: logical.1,
            physical_width: native.0,
            physical_height: native.1,
        }
    }

    fn geometry(
        active: DesktopBackdropOutputGeometry,
        outputs: &[DesktopBackdropOutputGeometry],
    ) -> OutputGeometry {
        OutputGeometry::update_from(
            Some((active.logical_x, active.logical_y)),
            Some((active.logical_width as i32, active.logical_height as i32)),
            (active.physical_width, active.physical_height),
            1,
            wl_output::Transform::Normal,
            Some((active.physical_width, active.physical_height)),
        )
        .unwrap()
        .with_desktop_backdrop_geometry(DesktopBackdropGeometry::from_outputs(active, outputs))
        .with_portal_outputs(Some(outputs.to_vec()))
        .with_known_output_count(Some(outputs.len() as u32))
    }

    fn cosmic_geometry(
        active: DesktopBackdropOutputGeometry,
        outputs: &[DesktopBackdropOutputGeometry],
    ) -> OutputGeometry {
        geometry(active, outputs)
            .with_uniform_desktop_density(UniformDesktopDensity::HighestOrLogical)
    }

    #[test]
    fn negative_vertical_and_gapped_layout_maps_both_crop_edges() {
        let outputs = [
            output(-10, -20, (8, 4), (8, 4)),
            output(-8, -14, (2, 4), (4, 8)),
        ];
        let geo = geometry(outputs[1], &outputs);
        let mut data = 0xff000000_u32.to_ne_bytes().repeat(16 * 20);
        for y in 12..20 {
            for x in 4..8 {
                data[(y * 16 + x) * 4..(y * 16 + x + 1) * 4]
                    .copy_from_slice(&0xff123456_u32.to_ne_bytes());
            }
        }

        let image = crop_portal_raster(&data, 16, 20, &geo).unwrap();

        assert_eq!(image.data, 0xff123456_u32.to_ne_bytes().repeat(4 * 8));
    }

    #[test]
    fn rotated_output_uses_transformed_native_dimensions_without_rotating_pixels_again() {
        let outputs = [output(0, 0, (8, 4), (8, 4)), output(8, 0, (1, 3), (2, 6))];
        let mut geo = geometry(outputs[1], &outputs);
        geo.transform = wl_output::Transform::_90;
        geo.scale = 2;
        geo.overlay_buffer_size = (2, 6);
        let mut data = 0xff000000_u32.to_ne_bytes().repeat(18 * 8);
        let mut expected = Vec::new();
        for y in 0..6 {
            for x in 16..18 {
                let pixel = (0xff000000_u32 | ((y + 1) << 8) | x).to_ne_bytes();
                data[((y * 18 + x) * 4) as usize..((y * 18 + x + 1) * 4) as usize]
                    .copy_from_slice(&pixel);
                expected.extend_from_slice(&pixel);
            }
        }

        let image = crop_portal_raster(&data, 18, 8, &geo).unwrap();

        assert_eq!((image.width, image.height), (2, 6));
        assert_eq!(image.data, expected);
    }

    #[test]
    fn packed_mixed_density_crop_uses_screenshot_origin_instead_of_logical_times_scale() {
        let outputs = [output(0, 0, (4, 2), (4, 2)), output(4, 0, (4, 2), (8, 4))];
        let mut geo = geometry(outputs[1], &outputs);
        geo.scale = 2;
        let mut pixels = Vec::new();
        let mut expected = Vec::new();
        for y in 0..4 {
            for x in 0..12 {
                let pixel = (0xff000000_u32 | (y << 8) | x).to_ne_bytes();
                pixels.extend_from_slice(&pixel);
                if x >= 4 {
                    expected.extend_from_slice(&pixel);
                }
            }
        }

        let image = crop_portal_raster(&pixels, 12, 4, &geo).unwrap();

        assert_eq!((image.width, image.height, image.stride), (8, 4, 32));
        assert_eq!(image.data, expected);
    }

    #[test]
    fn native_sized_raster_requires_a_proven_single_output_when_bounds_are_missing() {
        let geometry = OutputGeometry::update_from(
            Some((10, 20)),
            Some((2, 1)),
            (2, 1),
            1,
            wl_output::Transform::Normal,
            Some((2, 1)),
        )
        .unwrap();
        let pixels = [0xff123456_u32, 0xff654321_u32]
            .into_iter()
            .flat_map(u32::to_ne_bytes)
            .collect::<Vec<_>>();

        assert!(
            crop_portal_raster(
                &pixels,
                2,
                1,
                &geometry.clone().with_known_output_count(Some(2))
            )
            .is_err()
        );

        let image =
            crop_portal_raster(&pixels, 2, 1, &geometry.with_known_output_count(Some(1))).unwrap();

        assert_eq!(image.data, pixels);
    }

    #[test]
    fn cosmic_logical_composite_zooms_the_scaled_output() {
        // Issue #442: two 3840x2160 outputs at 1.75 and 1.5. COSMIC's portal
        // resizes each to its logical size, giving a 4754x1440 image.
        let outputs = [
            output(0, 0, (2194, 1234), (3840, 2160)),
            output(2194, 0, (2560, 1440), (3840, 2160)),
        ];
        let raster = vec![0; 4754 * 1440 * 4];
        let geo = geometry(outputs[0], &outputs);
        assert_eq!(
            geo.screenshot_size,
            Some((7680, 2469)),
            "the packed size from the report's log"
        );

        assert!(
            crop_portal_raster(&raster, 4754, 1440, &geo).is_err(),
            "density 1 is only trusted from COSMIC's portal"
        );

        let image = crop_portal_raster(&raster, 4754, 1440, &cosmic_geometry(outputs[0], &outputs))
            .unwrap();

        assert_eq!(
            (image.width, image.height),
            (3840, 2160),
            "the logical crop is resampled to the output's native pixels"
        );
    }

    #[test]
    fn logical_composite_resamples_only_the_active_output() {
        let outputs = [output(0, 0, (4, 2), (8, 4)), output(4, 0, (4, 2), (6, 3))];
        let left = 0xff102030_u32.to_ne_bytes();
        let right = 0xff405060_u32.to_ne_bytes();
        let mut pixels = Vec::new();
        for _ in 0..2 {
            for x in 0..8 {
                pixels.extend_from_slice(if x < 4 { &left } else { &right });
            }
        }

        let image =
            crop_portal_raster(&pixels, 8, 2, &cosmic_geometry(outputs[1], &outputs)).unwrap();

        assert_eq!(
            (image.width, image.height),
            (6, 3),
            "the crop is resampled to the 1.5x output's native size"
        );
        assert_eq!(
            image.data,
            right.repeat(6 * 3),
            "no pixel of the left output reaches the right one"
        );
    }

    #[test]
    fn packed_uniform_ambiguity_fails_closed() {
        // A smaller low-density output overlaps the high-density desktop bounds.
        // Packed and uniform screenshots have identical sizes but different crops.
        let outputs = [output(0, 0, (8, 4), (16, 8)), output(2, 1, (4, 2), (4, 2))];
        let geo = geometry(outputs[1], &outputs);
        assert_eq!(geo.screenshot_size, Some((16, 8)));
        assert!(crop_portal_raster(&vec![0; 16 * 8 * 4], 16, 8, &geo).is_err());
    }

    #[test]
    fn resampling_preserves_premultiplied_alpha_and_interpolates_pixels() {
        let data: Vec<u8> = [0x80202020_u32, 0x80606060_u32]
            .into_iter()
            .flat_map(u32::to_ne_bytes)
            .collect();

        let image = resample(data, (2, 1), (1, 1)).unwrap();

        assert_eq!(image.data, 0x80404040_u32.to_ne_bytes());
    }

    #[test]
    fn malformed_or_unexpected_raster_is_rejected() {
        let outputs = [
            output(0, 0, (1280, 720), (1280, 720)),
            output(1280, 0, (640, 360), (1280, 720)),
        ];
        let geo = geometry(outputs[0], &outputs);
        assert!(crop_portal_raster(&[0; 16], 3840, 1440, &geo).is_err());
        assert!(crop_portal_raster(&vec![0; 1920 * 720 * 4], 1920, 720, &geo).is_err());
    }
}
