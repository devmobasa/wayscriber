//! Verified mapping from a portal desktop raster into one output's native pixels.
use crate::capture::{CaptureError, DesktopBackdropOutputGeometry};
use crate::screen_pixels::ScreenImage;

use super::frozen_geometry::OutputGeometry;
use super::portal_capture::crop_argb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Crop {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct LogicalDesktop {
    x: i64,
    y: i64,
    width: u32,
    height: u32,
    density_min: f64,
    density_max: f64,
}

impl LogicalDesktop {
    fn from_outputs(outputs: &[DesktopBackdropOutputGeometry]) -> Option<Self> {
        let first = outputs.first()?;
        let (mut x, mut y) = (i64::from(first.logical_x), i64::from(first.logical_y));
        let (mut right, mut bottom) = (x, y);
        // KWin CaptureWorkspace uses at least 1x, then the maximum output scale.
        let (mut density_min, mut density_max) = (1.0_f64, 1.0_f64);
        for output in outputs {
            if output.logical_width == 0
                || output.logical_height == 0
                || output.physical_width == 0
                || output.physical_height == 0
            {
                return None;
            }
            x = x.min(i64::from(output.logical_x));
            y = y.min(i64::from(output.logical_y));
            right = right.max(i64::from(output.logical_x) + i64::from(output.logical_width));
            bottom = bottom.max(i64::from(output.logical_y) + i64::from(output.logical_height));
            // xdg-output rounds logical sizes to integers. Intersect both axes'
            // scale intervals instead of treating wl_output's integer scale as native density.
            let lower = (f64::from(output.physical_width)
                / (f64::from(output.logical_width) + 0.5))
                .max(f64::from(output.physical_height) / (f64::from(output.logical_height) + 0.5));
            let upper = (f64::from(output.physical_width)
                / (f64::from(output.logical_width) - 0.5))
                .min(f64::from(output.physical_height) / (f64::from(output.logical_height) - 0.5));
            if lower > upper {
                return None;
            }
            density_min = density_min.max(lower);
            density_max = density_max.max(upper);
        }
        Some(Self {
            x,
            y,
            width: u32::try_from(right.checked_sub(x)?).ok()?,
            height: u32::try_from(bottom.checked_sub(y)?).ok()?,
            density_min,
            density_max,
        })
    }

    fn crop(&self, geometry: &OutputGeometry, raster: (u32, u32)) -> Option<Crop> {
        let (width, height) = raster;
        if self.width == 0 || self.height == 0 || width == 0 || height == 0 {
            return None;
        }
        let lower = ((f64::from(width) - 0.5) / f64::from(self.width))
            .max((f64::from(height) - 0.5) / f64::from(self.height));
        let upper = ((f64::from(width) + 0.5) / f64::from(self.width))
            .min((f64::from(height) + 0.5) / f64::from(self.height));
        if lower > upper || lower > self.density_max || upper < self.density_min {
            return None;
        }
        let x = i64::from(geometry.logical_x).checked_sub(self.x)?;
        let y = i64::from(geometry.logical_y).checked_sub(self.y)?;
        let right = x.checked_add(i64::from(geometry.logical_width))?;
        let bottom = y.checked_add(i64::from(geometry.logical_height))?;
        if x < 0 || y < 0 || right > i64::from(self.width) || bottom > i64::from(self.height) {
            return None;
        }
        let map_x =
            |edge: i64| (edge as f64 * f64::from(width) / f64::from(self.width)).round() as u32;
        let map_y =
            |edge: i64| (edge as f64 * f64::from(height) / f64::from(self.height)).round() as u32;
        Some(Crop {
            x: map_x(x),
            y: map_y(y),
            width: map_x(right).checked_sub(map_x(x))?,
            height: map_y(bottom).checked_sub(map_y(y))?,
        })
    }
}

pub(super) fn crop_portal_raster(
    data: Vec<u8>,
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
        .map(|(x, y)| Crop {
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
    let uniform = desktop
        .as_ref()
        .filter(|_| geometry.active_output_is_in_portal_snapshot())
        .and_then(|desktop| desktop.crop(geometry, (width, height)));
    let crop = match (packed, uniform) {
        (Some(packed), Some(uniform)) if packed != uniform => None,
        (Some(crop), _) | (_, Some(crop)) => Some(crop),
        _ => None,
    };
    log::info!(
        "portal.raster image={}x{} packed_expected={:?} logical_desktop={:?} crop={:?} target={:?}",
        width,
        height,
        geometry.screenshot_size,
        desktop,
        crop,
        target,
    );
    let crop = crop.ok_or_else(|| {
        error("Portal capture does not match an unambiguous active output layout")
    })?;
    if !contains_crop((width, height), crop) {
        return Err(error("Portal capture does not contain the active output"));
    }
    let (_, _, data) = crop_argb(
        &data,
        width,
        height,
        crop.x,
        crop.y,
        crop.width,
        crop.height,
    )
    .ok_or_else(|| error("Portal capture crop is invalid"))?;
    resample(data, (crop.width, crop.height), target)
}

fn contains_crop(raster: (u32, u32), crop: Crop) -> bool {
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
        .with_desktop_backdrop_geometry(crate::capture::DesktopBackdropGeometry::from_outputs(
            active, outputs,
        ))
        .with_portal_outputs(Some(outputs.to_vec()))
        .with_known_output_count(Some(outputs.len() as u32))
    }

    #[test]
    fn negative_vertical_and_gapped_layout_maps_both_crop_edges() {
        let outputs = [
            output(-10, -20, (8, 4), (8, 4)),
            output(-8, -14, (2, 4), (4, 8)),
        ];
        let geo = geometry(outputs[1], &outputs);
        let desktop = LogicalDesktop::from_outputs(&outputs).unwrap();
        assert_eq!((desktop.width, desktop.height), (8, 10));
        assert_eq!(
            desktop.crop(&geo, (16, 20)),
            Some(Crop {
                x: 4,
                y: 12,
                width: 4,
                height: 8
            })
        );
        let mut data = 0xff000000_u32.to_ne_bytes().repeat(16 * 20);
        for y in 12..20 {
            for x in 4..8 {
                data[(y * 16 + x) * 4..(y * 16 + x + 1) * 4]
                    .copy_from_slice(&0xff123456_u32.to_ne_bytes());
            }
        }
        let image = crop_portal_raster(data, 16, 20, &geo).unwrap();
        assert_eq!(image.data, 0xff123456_u32.to_ne_bytes().repeat(4 * 8));
    }

    #[test]
    fn fractional_uniform_raster_accepts_integer_logical_rounding() {
        // 1.3x output with xdg-output's rounded 1477x831 logical dimensions.
        let outputs = [
            output(0, 0, (1477, 831), (1920, 1080)),
            output(1477, 0, (1280, 720), (1280, 720)),
        ];
        let geo = geometry(outputs[1], &outputs);
        let desktop = LogicalDesktop::from_outputs(&outputs).unwrap();
        let crop = desktop.crop(&geo, (3584, 1080)).unwrap();
        assert_eq!(
            crop,
            Crop {
                x: 1920,
                y: 0,
                width: 1664,
                height: 936
            }
        );
        assert_eq!(desktop.crop(&geo, (4000, 1200)), None);
        assert_eq!(desktop.crop(&geo, (3584, 1100)), None);
    }

    #[test]
    fn packed_uniform_ambiguity_fails_closed() {
        // A smaller low-density output overlaps the high-density desktop bounds.
        // Packed and uniform screenshots have identical sizes but different crops.
        let outputs = [output(0, 0, (8, 4), (16, 8)), output(2, 1, (4, 2), (4, 2))];
        let geo = geometry(outputs[1], &outputs);
        assert_eq!(geo.screenshot_size, Some((16, 8)));
        assert!(crop_portal_raster(vec![0; 16 * 8 * 4], 16, 8, &geo).is_err());
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
        assert!(crop_portal_raster(vec![0; 16], 3840, 1440, &geo).is_err());
        assert!(crop_portal_raster(vec![0; 1920 * 720 * 4], 1920, 720, &geo).is_err());
        assert!(
            LogicalDesktop::from_outputs(&[
                output(i32::MIN, 0, (1, 1), (1, 1)),
                output(i32::MAX, 0, (1, 1), (1, 1))
            ])
            .is_none()
        );
    }
}
