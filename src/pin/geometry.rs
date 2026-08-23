use super::{PinOutputHint, PinPlacementHint, PinRefusal};
use crate::pin::limits::MAX_PIN_SURFACE_PIXELS;

const MIN_LONG_EDGE: f64 = 160.0;
const MIN_SHORT_EDGE: f64 = 64.0;
const MIN_VISIBLE: f64 = 32.0;
const OUTPUT_FRACTION: f64 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PinFrame {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl PinFrame {
    pub(crate) const fn new(x: i32, y: i32, width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 {
            None
        } else {
            Some(Self {
                x,
                y,
                width,
                height,
            })
        }
    }

    #[cfg(test)]
    pub(crate) const fn right(self) -> i64 {
        self.x as i64 + self.width as i64
    }

    #[cfg(test)]
    pub(crate) const fn bottom(self) -> i64 {
        self.y as i64 + self.height as i64
    }
}

pub(crate) fn initial_frame(
    image_size: (u32, u32),
    placement: PinPlacementHint,
    output: &PinOutputHint,
) -> Result<PinFrame, PinRefusal> {
    let (image_width, image_height) = image_size;
    if image_width == 0 || image_height == 0 || !placement.is_valid() {
        return Err(PinRefusal::InvalidPlacement);
    }
    let aspect = f64::from(image_width) / f64::from(image_height);
    let preferred_width = placement.width.min(placement.height * aspect);
    let preferred_height = preferred_width / aspect;
    let (width, height) = constrained_size(
        preferred_width,
        preferred_height,
        aspect,
        output.logical_size(),
        output.scale,
    )?;
    let center_x = placement.x + placement.width / 2.0;
    let center_y = placement.y + placement.height / 2.0;
    let origin = clamp_origin(
        center_x - width / 2.0,
        center_y - height / 2.0,
        width,
        height,
        output.logical_size(),
    );
    frame_from_f64(origin.0, origin.1, width, height, aspect, output.scale)
}

pub(crate) fn dragged_frame(
    frame: PinFrame,
    candidate_origin: (f64, f64),
    output_size: (u32, u32),
) -> PinFrame {
    let (x, y) = clamp_origin(
        candidate_origin.0,
        candidate_origin.1,
        f64::from(frame.width),
        f64::from(frame.height),
        output_size,
    );
    PinFrame {
        x: floor_i32(x),
        y: floor_i32(y),
        ..frame
    }
}

pub(crate) fn resized_frame(
    frame: PinFrame,
    pointer_local: (f64, f64),
    tenth_octave_steps: f64,
    image_size: (u32, u32),
    output_size: (u32, u32),
    scale: u32,
) -> Result<PinFrame, PinRefusal> {
    if !pointer_local.0.is_finite()
        || !pointer_local.1.is_finite()
        || !tenth_octave_steps.is_finite()
        || image_size.0 == 0
        || image_size.1 == 0
    {
        return Err(PinRefusal::InvalidPlacement);
    }
    let old_width = f64::from(frame.width);
    let old_height = f64::from(frame.height);
    let u = pointer_local.0 / old_width;
    let v = pointer_local.1 / old_height;
    let global = (
        f64::from(frame.x) + pointer_local.0,
        f64::from(frame.y) + pointer_local.1,
    );
    let factor = 2_f64.powf(tenth_octave_steps / 10.0);
    let aspect = f64::from(image_size.0) / f64::from(image_size.1);
    let (width, height) = constrained_size(
        old_width * factor,
        old_height * factor,
        aspect,
        output_size,
        scale,
    )?;
    let (x, y) = clamp_origin(
        global.0 - u * width,
        global.1 - v * height,
        width,
        height,
        output_size,
    );
    frame_from_f64(x, y, width, height, aspect, scale)
}

/// Migrates a pin to a new output while preserving normalized centre when possible.
///
/// Unlike interactive resize, migration may shrink below the normal minimum so a
/// hotplug to a very small output remains representable instead of panicking.
pub(crate) fn migrated_frame(
    frame: PinFrame,
    image_size: (u32, u32),
    previous_output_size: (u32, u32),
    next_output_size: (u32, u32),
    next_scale: u32,
) -> Result<PinFrame, PinRefusal> {
    if image_size.0 == 0
        || image_size.1 == 0
        || previous_output_size.0 == 0
        || previous_output_size.1 == 0
        || next_output_size.0 == 0
        || next_output_size.1 == 0
        || next_scale == 0
    {
        return Err(PinRefusal::InvalidPlacement);
    }
    let aspect = f64::from(image_size.0) / f64::from(image_size.1);
    let center_u =
        (f64::from(frame.x) + f64::from(frame.width) / 2.0) / f64::from(previous_output_size.0);
    let center_v =
        (f64::from(frame.y) + f64::from(frame.height) / 2.0) / f64::from(previous_output_size.1);
    let max_width = f64::from(next_output_size.0) * OUTPUT_FRACTION;
    let max_height = f64::from(next_output_size.1) * OUTPUT_FRACTION;
    let width = f64::from(frame.width)
        .min(max_width)
        .min(max_height * aspect);
    let height = f64::from(frame.height)
        .min(max_height)
        .min(max_width / aspect);
    let (width, height) = quantized_size(width, height, aspect, next_scale)?;
    let candidate = (
        center_u * f64::from(next_output_size.0) - f64::from(width) / 2.0,
        center_v * f64::from(next_output_size.1) - f64::from(height) / 2.0,
    );
    let (x, y) = clamp_origin(
        candidate.0,
        candidate.1,
        f64::from(width),
        f64::from(height),
        next_output_size,
    );
    PinFrame::new(floor_i32(x), floor_i32(y), width, height).ok_or(PinRefusal::InvalidPlacement)
}

fn constrained_size(
    requested_width: f64,
    requested_height: f64,
    aspect: f64,
    output_size: (u32, u32),
    scale: u32,
) -> Result<(f64, f64), PinRefusal> {
    if !(requested_width.is_finite()
        && requested_height.is_finite()
        && aspect.is_finite()
        && aspect > 0.0)
        || output_size.0 == 0
        || output_size.1 == 0
        || scale == 0
    {
        return Err(PinRefusal::InvalidPlacement);
    }
    let source_long = aspect.max(1.0);
    let source_short = aspect.min(1.0);
    let min_height = (MIN_LONG_EDGE / source_long).max(MIN_SHORT_EDGE / source_short);
    let min_width = min_height * aspect;

    let mut max_width = f64::from(output_size.0) * OUTPUT_FRACTION;
    let mut max_height = f64::from(output_size.1) * OUTPUT_FRACTION;
    let physical_budget = MAX_PIN_SURFACE_PIXELS as f64 / f64::from(scale).powi(2);
    max_width = max_width.min((physical_budget * aspect).sqrt());
    max_height = max_height.min((physical_budget / aspect).sqrt());
    let fit_width = max_width.min(max_height * aspect);
    let fit_height = fit_width / aspect;
    if fit_width < min_width || fit_height < min_height {
        return Err(PinRefusal::LimitExceeded);
    }

    let requested_width = requested_width.min(requested_height * aspect);
    let width = requested_width.clamp(min_width, fit_width);
    Ok((width, width / aspect))
}

fn clamp_origin(x: f64, y: f64, width: f64, height: f64, output_size: (u32, u32)) -> (f64, f64) {
    let min_x = MIN_VISIBLE - width;
    let max_x = f64::from(output_size.0) - MIN_VISIBLE;
    let min_y = MIN_VISIBLE - height;
    let max_y = f64::from(output_size.1) - MIN_VISIBLE;
    let clamp_axis = |value: f64, min: f64, max: f64, output: u32, extent: f64| {
        if min <= max {
            value.clamp(min, max)
        } else if extent <= f64::from(output) {
            // The output is smaller than the normal two-sided visibility
            // allowance, but this surface still fits. Align it to the far
            // edge so the resize controls remain reachable.
            f64::from(output) - extent
        } else {
            // A sub-64px output cannot satisfy 32px visibility at both edges.
            // Centre the emergency-migrated surface to maximize visible area.
            (f64::from(output) - extent) / 2.0
        }
    };
    (
        clamp_axis(x, min_x, max_x, output_size.0, width),
        clamp_axis(y, min_y, max_y, output_size.1, height),
    )
}

fn frame_from_f64(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    aspect: f64,
    scale: u32,
) -> Result<PinFrame, PinRefusal> {
    let (width, height) = quantized_size(width, height, aspect, scale)?;
    PinFrame::new(floor_i32(x), floor_i32(y), width, height).ok_or(PinRefusal::InvalidPlacement)
}

fn quantized_size(
    width: f64,
    height: f64,
    aspect: f64,
    scale: u32,
) -> Result<(u32, u32), PinRefusal> {
    let mut long_axis = ceil_u32(if aspect >= 1.0 { width } else { height })?;
    loop {
        let (width, height) = if aspect >= 1.0 {
            (long_axis, ceil_u32(f64::from(long_axis) / aspect)?)
        } else {
            (ceil_u32(f64::from(long_axis) * aspect)?, long_axis)
        };
        let physical_width = width.checked_mul(scale).ok_or(PinRefusal::LimitExceeded)?;
        let physical_height = height.checked_mul(scale).ok_or(PinRefusal::LimitExceeded)?;
        let pixels = u64::from(physical_width)
            .checked_mul(u64::from(physical_height))
            .ok_or(PinRefusal::LimitExceeded)?;
        if pixels <= MAX_PIN_SURFACE_PIXELS {
            return Ok((width, height));
        }
        long_axis = long_axis.checked_sub(1).ok_or(PinRefusal::LimitExceeded)?;
    }
}

fn floor_i32(value: f64) -> i32 {
    value
        .floor()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn ceil_u32(value: f64) -> Result<u32, PinRefusal> {
    let value = value.ceil();
    if value < 1.0 || value > f64::from(u32::MAX) {
        return Err(PinRefusal::InvalidPlacement);
    }
    Ok(value as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pin::{PinOutputTransform, PinPlacementHint};

    fn output(scale: u32) -> PinOutputHint {
        PinOutputHint::new("DP-1".into(), 1920, 1080, scale, PinOutputTransform::Normal).unwrap()
    }

    #[test]
    fn initial_placement_centres_outward_quantized_rect_and_preserves_aspect() {
        let frame = initial_frame(
            (1600, 900),
            PinPlacementHint::new(100.0, 200.0, 803.0, 455.0).unwrap(),
            &output(1),
        )
        .unwrap();
        assert_eq!((frame.width, frame.height), (803, 452));
        assert_eq!((frame.x, frame.y), (100, 201));
    }

    #[test]
    fn reversed_review_corners_normalize_to_the_same_placement() {
        let forward = PinPlacementHint::from_corners(10.0, 20.0, 410.0, 245.0).unwrap();
        let reverse = PinPlacementHint::from_corners(410.0, 245.0, 10.0, 20.0).unwrap();
        assert_eq!(
            initial_frame((1600, 900), forward, &output(1)),
            initial_frame((1600, 900), reverse, &output(1))
        );
    }

    #[test]
    fn dragging_keeps_thirty_two_logical_pixels_visible_at_every_edge() {
        let frame = PinFrame::new(0, 0, 400, 200).unwrap();
        assert_eq!(
            dragged_frame(frame, (-10_000.0, -10_000.0), (1920, 1080)).x,
            -368
        );
        let bottom_right = dragged_frame(frame, (10_000.0, 10_000.0), (1920, 1080));
        assert_eq!((bottom_right.x, bottom_right.y), (1888, 1048));
    }

    #[test]
    fn wheel_resize_preserves_pointer_normalized_image_point() {
        let frame = PinFrame::new(100, 100, 400, 200).unwrap();
        let resized = resized_frame(frame, (100.0, 50.0), 10.0, (2, 1), (1920, 1080), 1).unwrap();
        assert_eq!((resized.width, resized.height), (800, 400));
        assert_eq!((resized.x, resized.y), (0, 50));
    }

    #[test]
    fn physical_pixel_budget_applies_at_scale_two() {
        let frame = PinFrame::new(0, 0, 1600, 900).unwrap();
        let resized =
            resized_frame(frame, (800.0, 450.0), 100.0, (16, 9), (8000, 8000), 2).unwrap();
        let physical_pixels = u64::from(resized.width) * u64::from(resized.height) * 4;
        assert!(physical_pixels <= MAX_PIN_SURFACE_PIXELS);
    }

    #[test]
    fn quantized_pixel_cap_is_exact_for_integer_scales_and_fractional_aspects() {
        for (scale, image) in [(1, (13, 7)), (2, (13, 7)), (2, (1919, 1079))] {
            let frame = PinFrame::new(0, 0, 1600, 900).unwrap();
            let resized =
                resized_frame(frame, (800.0, 450.0), 100.0, image, (20_000, 20_000), scale)
                    .unwrap();
            let pixels =
                u64::from(resized.width) * u64::from(resized.height) * u64::from(scale).pow(2);
            assert!(
                pixels <= MAX_PIN_SURFACE_PIXELS,
                "{scale}x {image:?}: {pixels}"
            );
        }
    }

    #[test]
    fn migration_can_emergency_shrink_below_interactive_minimum() {
        let migrated = migrated_frame(
            PinFrame::new(100, 100, 400, 200).unwrap(),
            (2, 1),
            (1920, 1080),
            (100, 60),
            2,
        )
        .unwrap();
        assert_eq!((migrated.width, migrated.height), (80, 40));
        assert!(migrated.right() >= 32 && migrated.bottom() >= 32);

        let portrait = migrated_frame(
            PinFrame::new(0, 0, 20, 400).unwrap(),
            (1, 20),
            (1920, 1080),
            (40, 30),
            1,
        )
        .unwrap();
        assert!(portrait.width <= 32 && portrait.height <= 24);
    }
}
