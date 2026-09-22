//! Checked raster geometry for the main Wayland surface.

use anyhow::{Result, anyhow, ensure};

use crate::util::Rect;

/// Bounds the main SHM pool independently of the compositor's requested size.
const MAX_POOL_BYTES: usize = 1024 * 1024 * 1024;
const CAIRO_MAX_DIM: u64 = 32_767;
const FRACTIONAL_DENOMINATOR: u128 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SurfaceGeometry {
    pub logical_width: u32,
    pub logical_height: u32,
    pub preferred_scale: Option<u32>,
    pub buffer_scale: i32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub stride: i32,
    pub byte_len: usize,
    pub slot_len: usize,
    pub pool_len: usize,
}

impl SurfaceGeometry {
    /// The single raster-rounding policy for configured and pending surfaces.
    /// Pending surfaces may still have zero dimensions, so callers that need
    /// allocated storage must validate the result through `new`.
    pub(super) fn raster_dimensions(
        width: u32,
        height: u32,
        integer_scale: i32,
        preferred_scale: Option<u32>,
    ) -> (u32, u32) {
        let dimension = |logical: u32| {
            let pixels = if let Some(scale) = preferred_scale.filter(|scale| *scale > 0) {
                (u128::from(logical) * u128::from(scale) + FRACTIONAL_DENOMINATOR / 2)
                    / FRACTIONAL_DENOMINATOR
            } else {
                u128::from(logical) * integer_scale.max(1) as u128
            };
            pixels.min(u128::from(u32::MAX)) as u32
        };
        (dimension(width), dimension(height))
    }

    pub fn new(
        logical_width: u32,
        logical_height: u32,
        integer_scale: i32,
        preferred_scale: Option<u32>,
        buffer_count: usize,
    ) -> Result<Self> {
        ensure!(
            logical_width > 0 && logical_height > 0,
            "surface dimensions must be positive"
        );
        ensure!(
            logical_width <= i32::MAX as u32 && logical_height <= i32::MAX as u32,
            "surface dimensions exceed protocol limits"
        );
        ensure!((1..=4).contains(&buffer_count), "unsupported buffer count");

        let preferred_scale = preferred_scale.filter(|scale| *scale > 0);
        let buffer_scale = if preferred_scale.is_some() {
            1
        } else {
            integer_scale.max(1)
        };
        let (pixel_width, pixel_height) = Self::raster_dimensions(
            logical_width,
            logical_height,
            integer_scale,
            preferred_scale,
        );
        ensure!(
            pixel_width > 0
                && pixel_height > 0
                && u64::from(pixel_width) <= CAIRO_MAX_DIM
                && u64::from(pixel_height) <= CAIRO_MAX_DIM,
            "raster size exceeds Cairo limits"
        );
        let stride = i32::try_from(u64::from(pixel_width) * 4)
            .map_err(|_| anyhow!("surface stride exceeds Cairo limits"))?;
        let byte_len = usize::try_from(u64::from(pixel_height) * stride as u64)
            .map_err(|_| anyhow!("surface byte length exceeds address space"))?;
        let slot_len = byte_len
            .checked_add(63)
            .map(|value| value & !63)
            .ok_or_else(|| anyhow!("surface slot length overflow"))?;
        let pool_len = slot_len
            .checked_mul(buffer_count)
            .ok_or_else(|| anyhow!("surface pool length overflow"))?;
        ensure!(
            pool_len <= MAX_POOL_BYTES,
            "surface pool exceeds 1 GiB allocation limit"
        );

        Ok(Self {
            logical_width,
            logical_height,
            preferred_scale,
            buffer_scale,
            pixel_width,
            pixel_height,
            stride,
            byte_len,
            slot_len,
            pool_len,
        })
    }

    pub fn map_damage(
        logical: Rect,
        logical_size: (u32, u32),
        pixel_size: (u32, u32),
    ) -> Option<Rect> {
        if logical_size.0 == 0 || logical_size.1 == 0 {
            return None;
        }
        fn bounds(start: i32, extent: i32, logical: u32, pixels: u32) -> Option<(i32, i32)> {
            let start = i128::from(start);
            let end = start + i128::from(extent);
            let numerator = i128::from(pixels);
            let denominator = i128::from(logical);
            let min = (start * numerator).div_euclid(denominator);
            let max = -(-end * numerator).div_euclid(denominator);
            let min = min.clamp(0, i128::from(pixels));
            let max = max.clamp(0, i128::from(pixels));
            Some((i32::try_from(min).ok()?, i32::try_from(max).ok()?))
        }

        let (x0, x1) = bounds(logical.x, logical.width, logical_size.0, pixel_size.0)?;
        let (y0, y1) = bounds(logical.y, logical.height, logical_size.1, pixel_size.1)?;
        Rect::from_min_max(x0, y0, x1, y1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_geometry_uses_rounded_pixel_extent_and_one_wire_scale() {
        let geometry = SurfaceGeometry::new(2304, 1296, 2, Some(200), 3).unwrap();
        assert_eq!((geometry.pixel_width, geometry.pixel_height), (3840, 2160));
        assert_eq!(geometry.buffer_scale, 1);
        assert_eq!(geometry.pool_len, 99_532_800);
    }

    #[test]
    fn odd_fractional_size_rounds_half_up_and_damage_covers_negative_edges() {
        let geometry = SurfaceGeometry::new(5, 3, 2, Some(150), 3).unwrap();
        assert_eq!((geometry.pixel_width, geometry.pixel_height), (6, 4));
        assert_eq!(
            SurfaceGeometry::map_damage(Rect::new(-1, -1, 3, 3).unwrap(), (5, 3), (6, 4)),
            Rect::new(0, 0, 3, 3)
        );
        assert_eq!(
            SurfaceGeometry::map_damage(Rect::new(4, 2, 2, 2).unwrap(), (5, 3), (6, 4)),
            Rect::new(4, 2, 2, 2)
        );
    }

    #[test]
    fn preferred_scale_sizes_cover_supported_native_factors() {
        for (logical, numerator) in [
            ((1680, 1050), 120),
            ((1344, 840), 150),
            ((1120, 700), 180),
            ((1008, 630), 200),
            ((960, 600), 210),
            ((840, 525), 240),
        ] {
            let geometry =
                SurfaceGeometry::new(logical.0, logical.1, 2, Some(numerator), 3).unwrap();
            assert_eq!((geometry.pixel_width, geometry.pixel_height), (1680, 1050));
            assert_eq!(geometry.buffer_scale, 1);
            assert_eq!(geometry.pool_len, 21_168_000);
        }
        assert_eq!(
            SurfaceGeometry::new(2, 2, 1, Some(150), 1)
                .unwrap()
                .pixel_width,
            3
        );
    }

    #[test]
    fn integer_fallback_and_allocation_bounds() {
        let geometry = SurfaceGeometry::new(101, 79, 2, None, 3).unwrap();
        assert_eq!((geometry.pixel_width, geometry.pixel_height), (202, 158));
        assert_eq!(geometry.buffer_scale, 2);
        assert_eq!(geometry.slot_len, 127_680);
        assert_eq!(
            SurfaceGeometry::raster_dimensions(0, 0, 2, Some(150)),
            (0, 0)
        );
        assert_eq!(
            SurfaceGeometry::raster_dimensions(81, 63, 2, Some(150)),
            (101, 79)
        );
        assert_eq!(
            SurfaceGeometry::raster_dimensions(81, 63, 2, None),
            (162, 126)
        );
        assert!(SurfaceGeometry::new(0, 10, 1, None, 3).is_err());
        assert!(SurfaceGeometry::new(500_000, 500_000, 2, None, 3).is_err());
    }
}
