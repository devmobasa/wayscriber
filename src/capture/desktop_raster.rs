//! Where one output lies in a full-desktop screenshot drawn at one density.
//!
//! A screenshot of several outputs is either packed, with each output at its
//! native density (`DesktopBackdropGeometry::screenshot_size`), or uniform,
//! with the whole logical desktop drawn at one density. Zoom, Freeze, and the
//! PDF desktop background accept a uniform screenshot only at a density that a
//! known screenshot backend produces.
use super::types::DesktopBackdropOutputGeometry;

/// A rectangle of screenshot pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterCrop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// The densities a uniform full-desktop screenshot may have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UniformDesktopDensity {
    /// The highest output density, as KWin's `CaptureWorkspace` and grim draw
    /// the desktop. An area screenshot can only have this size if it covers
    /// the whole desktop. The size must be within half a pixel of the scaled
    /// desktop, so a grim image whose truncated size differs is rejected.
    #[default]
    Highest,
    /// The highest output density, or density 1. COSMIC's portal resizes each
    /// output to its logical size when it combines several. Other portals can
    /// return an area screenshot, and an area captured at a higher density can
    /// have the logical desktop's size, so only COSMIC uses this.
    HighestOrLogical,
}

/// The logical bounding box of every output, and the density range of a
/// uniform screenshot drawn at the highest output density.
#[derive(Debug)]
pub struct LogicalDesktop {
    x: i64,
    y: i64,
    width: u32,
    height: u32,
    density_min: f64,
    density_max: f64,
}

impl LogicalDesktop {
    #[must_use]
    pub fn from_outputs(outputs: &[DesktopBackdropOutputGeometry]) -> Option<Self> {
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

    /// Maps `active`'s logical rectangle into a `raster`-sized uniform
    /// screenshot of this desktop, or returns `None` when the raster's aspect
    /// or density does not fit.
    #[must_use]
    pub fn crop(
        &self,
        active: &DesktopBackdropOutputGeometry,
        raster: (u32, u32),
        density: UniformDesktopDensity,
    ) -> Option<RasterCrop> {
        let (width, height) = raster;
        if self.width == 0 || self.height == 0 || width == 0 || height == 0 {
            return None;
        }

        // A rounded size is within half a pixel of the scaled desktop. A wider
        // tolerance would let packed captures and area screenshots pass.
        let lower = ((f64::from(width) - 0.5) / f64::from(self.width))
            .max((f64::from(height) - 0.5) / f64::from(self.height));
        let upper = ((f64::from(width) + 0.5) / f64::from(self.width))
            .min((f64::from(height) + 0.5) / f64::from(self.height));
        if lower > upper || !self.accepts_density(lower, upper, density) {
            return None;
        }

        let x = i64::from(active.logical_x).checked_sub(self.x)?;
        let y = i64::from(active.logical_y).checked_sub(self.y)?;
        let right = x.checked_add(i64::from(active.logical_width))?;
        let bottom = y.checked_add(i64::from(active.logical_height))?;
        if x < 0 || y < 0 || right > i64::from(self.width) || bottom > i64::from(self.height) {
            return None;
        }

        let map_x =
            |edge: i64| (edge as f64 * f64::from(width) / f64::from(self.width)).round() as u32;
        let map_y =
            |edge: i64| (edge as f64 * f64::from(height) / f64::from(self.height)).round() as u32;

        Some(RasterCrop {
            x: map_x(x),
            y: map_y(y),
            width: map_x(right).checked_sub(map_x(x))?,
            height: map_y(bottom).checked_sub(map_y(y))?,
        })
    }

    fn accepts_density(&self, lower: f64, upper: f64, density: UniformDesktopDensity) -> bool {
        let overlaps = |min: f64, max: f64| lower <= max && upper >= min;

        overlaps(self.density_min, self.density_max)
            || (density == UniformDesktopDensity::HighestOrLogical && overlaps(1.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn negative_vertical_and_gapped_layout_maps_both_crop_edges() {
        let outputs = [
            output(-10, -20, (8, 4), (8, 4)),
            output(-8, -14, (2, 4), (4, 8)),
        ];
        let desktop = LogicalDesktop::from_outputs(&outputs).unwrap();

        assert_eq!(
            (desktop.width, desktop.height),
            (8, 10),
            "bounds span both outputs and the gap between them"
        );
        assert_eq!(
            desktop.crop(&outputs[1], (16, 20), UniformDesktopDensity::Highest),
            Some(RasterCrop {
                x: 4,
                y: 12,
                width: 4,
                height: 8
            }),
            "the 2x output maps to its 4x8 pixels below the gap"
        );
    }

    #[test]
    fn fractional_uniform_raster_accepts_integer_logical_rounding() {
        // 1.3x output with xdg-output's rounded 1477x831 logical dimensions.
        let outputs = [
            output(0, 0, (1477, 831), (1920, 1080)),
            output(1477, 0, (1280, 720), (1280, 720)),
        ];
        let desktop = LogicalDesktop::from_outputs(&outputs).unwrap();
        let density = UniformDesktopDensity::Highest;

        assert_eq!(
            desktop.crop(&outputs[1], (3584, 1080), density),
            Some(RasterCrop {
                x: 1920,
                y: 0,
                width: 1664,
                height: 936
            }),
            "the 1x output starts where the 1.3x output's 1920 pixels end"
        );
        assert_eq!(
            desktop.crop(&outputs[1], (4000, 1200), density),
            None,
            "the width and height give different densities"
        );
        assert_eq!(
            desktop.crop(&outputs[1], (3584, 1100), density),
            None,
            "the height does not match the desktop aspect"
        );
    }

    #[test]
    fn packed_capture_does_not_also_fit_the_uniform_rule() {
        // Each packed capture is within a pixel of the uniform desktop at the
        // highest density. Accepting it as uniform as well would make the
        // active output's crop ambiguous and reject a valid capture.
        let layouts = [
            // 175%: packed 6000x1600.
            (
                [
                    output(0, 0, (1463, 914), (2560, 1600)),
                    output(1463, 0, (1966, 823), (3440, 1440)),
                ],
                (6000, 1600),
            ),
            // 130%: packed 6400x2160, also grim's truncated size.
            (
                [
                    output(0, 0, (985, 554), (1280, 720)),
                    output(985, 0, (3938, 1662), (5120, 2160)),
                ],
                (6400, 2160),
            ),
        ];

        for (outputs, packed) in layouts {
            let desktop = LogicalDesktop::from_outputs(&outputs).unwrap();

            assert_eq!(
                desktop.crop(&outputs[1], packed, UniformDesktopDensity::Highest),
                None,
                "the packed {packed:?} capture keeps a single interpretation"
            );
        }
    }

    #[test]
    fn area_one_pixel_short_of_the_desktop_is_rejected() {
        // Two 1x outputs: an area from x = 1 is 3839x1080, and its right
        // output starts at 1919, not 1920.
        let outputs = [
            output(0, 0, (1920, 1080), (1920, 1080)),
            output(1920, 0, (1920, 1080), (1920, 1080)),
        ];
        let desktop = LogicalDesktop::from_outputs(&outputs).unwrap();

        assert_eq!(
            desktop.crop(&outputs[1], (3839, 1080), UniformDesktopDensity::Highest),
            None,
            "a 3839x1080 image is not the 3840x1080 desktop"
        );
    }

    #[test]
    fn cosmic_logical_composite_crops_each_scaled_output() {
        // Issue #442: two 3840x2160 outputs at 1.75 and 1.5. COSMIC's portal
        // resizes each to its logical size, giving a 4754x1440 image.
        let outputs = [
            output(0, 0, (2194, 1234), (3840, 2160)),
            output(2194, 0, (2560, 1440), (3840, 2160)),
        ];
        let desktop = LogicalDesktop::from_outputs(&outputs).unwrap();
        let density = UniformDesktopDensity::HighestOrLogical;

        assert_eq!(
            desktop.crop(&outputs[0], (4754, 1440), density),
            Some(RasterCrop {
                x: 0,
                y: 0,
                width: 2194,
                height: 1234
            }),
            "the 1.75 output is resized to its logical 2194x1234"
        );
        assert_eq!(
            desktop.crop(&outputs[1], (4754, 1440), density),
            Some(RasterCrop {
                x: 2194,
                y: 0,
                width: 2560,
                height: 1440
            }),
            "the 1.5 output is resized to its logical 2560x1440"
        );
    }

    #[test]
    fn logical_size_raster_needs_a_backend_known_to_combine_at_density_one() {
        // Two 3840x2160 outputs at scale 2. An area of 1920x540 logical pixels
        // captured at density 2 has the logical desktop's 3840x1080 size.
        let outputs = [
            output(0, 0, (1920, 1080), (3840, 2160)),
            output(1920, 0, (1920, 1080), (3840, 2160)),
        ];
        let desktop = LogicalDesktop::from_outputs(&outputs).unwrap();

        assert_eq!(
            desktop.crop(&outputs[1], (3840, 1080), UniformDesktopDensity::Highest),
            None,
            "outside COSMIC, an area at density 2 must not pass as the desktop"
        );
        assert!(
            desktop
                .crop(
                    &outputs[1],
                    (3840, 1080),
                    UniformDesktopDensity::HighestOrLogical
                )
                .is_some(),
            "COSMIC's combined image has the logical desktop's size"
        );
        assert!(
            desktop
                .crop(&outputs[1], (7680, 2160), UniformDesktopDensity::Highest)
                .is_some(),
            "the highest density stays accepted everywhere"
        );
    }

    #[test]
    fn logical_density_does_not_admit_other_densities() {
        let outputs = [
            output(0, 0, (1280, 720), (1280, 720)),
            output(1280, 0, (640, 360), (1280, 720)),
        ];
        let desktop = LogicalDesktop::from_outputs(&outputs).unwrap();

        // The desktop's aspect at density 1.25: neither logical nor the highest density.
        assert_eq!(
            desktop.crop(
                &outputs[0],
                (2400, 900),
                UniformDesktopDensity::HighestOrLogical
            ),
            None,
            "COSMIC adds density 1, not any density"
        );
    }

    #[test]
    fn layout_bounds_beyond_supported_coordinates_are_rejected() {
        assert!(
            LogicalDesktop::from_outputs(&[
                output(i32::MIN, 0, (1, 1), (1, 1)),
                output(i32::MAX, 0, (1, 1), (1, 1))
            ])
            .is_none(),
            "the bounds do not fit in u32"
        );
    }
}
