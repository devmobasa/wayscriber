use anyhow::{Context, Result};
use wayland_client::protocol::{wl_output, wl_shm};

pub use crate::screen_pixels::ScreenImage as FrozenImage;

pub(in crate::backend::wayland) struct ShmBufferLayout {
    pub(in crate::backend::wayland) width: i32,
    pub(in crate::backend::wayland) height: i32,
    pub(in crate::backend::wayland) stride: i32,
    pub(in crate::backend::wayland) total_size: usize,
}

/// Validate compositor-owned dimensions before allocating or creating a
/// `wl_buffer` from them.
pub(in crate::backend::wayland) fn validate_shm_buffer_layout(
    width: u32,
    height: u32,
    stride: u32,
) -> Result<ShmBufferLayout> {
    if width == 0 || height == 0 {
        anyhow::bail!("Frozen capture advertised an empty SHM buffer");
    }

    let buffer_width =
        i32::try_from(width).context("Frozen capture width exceeds the Wayland limit")?;
    let buffer_height =
        i32::try_from(height).context("Frozen capture height exceeds the Wayland limit")?;
    let buffer_stride =
        i32::try_from(stride).context("Frozen capture stride exceeds the Wayland limit")?;
    let row_bytes = width
        .checked_mul(4)
        .context("Frozen capture row size overflow")?;
    if stride < row_bytes {
        anyhow::bail!("Frozen capture stride is smaller than its pixel row");
    }
    let total_size = usize::try_from(stride)
        .ok()
        .and_then(|stride| {
            usize::try_from(height)
                .ok()
                .and_then(|height| stride.checked_mul(height))
        })
        .context("Frozen capture buffer size overflow")?;

    Ok(ShmBufferLayout {
        width: buffer_width,
        height: buffer_height,
        stride: buffer_stride,
        total_size,
    })
}

impl FrozenImage {
    /// Undo the `wl_output` transform the compositor applied to the captured
    /// output's pixels.
    ///
    /// `ext-image-copy-capture` reports the transform the compositor applied
    /// to each frame. WLR screencopy frames carry no transform of their own, so
    /// callers pass the advertised `wl_output` transform on the assumption
    /// that the frame holds output framebuffer pixels, as wlroots and niri
    /// return them. Frozen/zoom rendering paints the pixels onto a
    /// logical surface, so rotated or flipped outputs need the inverse of that
    /// transform. A 90 degree output, for example, holds its content rotated 90
    /// degrees counter-clockwise, so the buffer is rotated back clockwise.
    pub fn with_output_transform(mut self, transform: wl_output::Transform) -> Result<Self> {
        let width =
            usize::try_from(self.width).context("Frozen image width does not fit memory")?;
        let height =
            usize::try_from(self.height).context("Frozen image height does not fit memory")?;
        let row_bytes = width
            .checked_mul(4)
            .context("Frozen image row size overflow")?;
        let expected_len = row_bytes
            .checked_mul(height)
            .context("Frozen image size overflow")?;
        let expected_stride =
            i32::try_from(row_bytes).context("Frozen image stride exceeds i32")?;
        if self.stride != expected_stride || self.data.len() != expected_len {
            anyhow::bail!("Frozen image buffer does not match its dimensions");
        }
        if transform == wl_output::Transform::Normal {
            return Ok(self);
        }

        let (width, height, data) = transform_argb(width, height, &self.data, transform)
            .context("Frozen image transform rejected its pixel buffer")?;
        self.width = u32::try_from(width).context("Transformed frozen width exceeds u32")?;
        self.height = u32::try_from(height).context("Transformed frozen height exceeds u32")?;
        self.stride = i32::try_from(
            width
                .checked_mul(4)
                .context("Transformed frozen stride overflow")?,
        )
        .context("Transformed frozen stride exceeds i32")?;
        self.data = data;

        Ok(self)
    }
}

/// Copy a compositor-provided SHM buffer into tightly packed Cairo-compatible
/// ARGB data while validating every advertised dimension and row boundary.
pub(in crate::backend::wayland) fn copy_shm_argb(
    canvas: &[u8],
    width: u32,
    height: u32,
    stride: i32,
    format: wl_shm::Format,
    y_invert: bool,
) -> Result<FrozenImage> {
    if !matches!(format, wl_shm::Format::Argb8888 | wl_shm::Format::Xrgb8888) {
        anyhow::bail!("Unsupported frozen capture SHM format: {format:?}");
    }

    let width = usize::try_from(width).context("Frozen capture width does not fit in memory")?;
    let height = usize::try_from(height).context("Frozen capture height does not fit in memory")?;
    let row_bytes = width
        .checked_mul(4)
        .context("Frozen capture row size overflow")?;
    let stride = usize::try_from(stride).context("Frozen capture stride is negative")?;
    if stride < row_bytes {
        anyhow::bail!("Frozen capture stride is smaller than its pixel row");
    }

    let image_size = row_bytes
        .checked_mul(height)
        .context("Frozen capture image size overflow")?;
    let mut data = vec![0; image_size];
    for source_row in 0..height {
        let source_start = source_row
            .checked_mul(stride)
            .context("Frozen capture source row offset overflow")?;
        let source_end = source_start
            .checked_add(row_bytes)
            .context("Frozen capture source row end overflow")?;
        let target_row = if y_invert {
            height - 1 - source_row
        } else {
            source_row
        };
        let target_start = target_row
            .checked_mul(row_bytes)
            .context("Frozen capture target row offset overflow")?;
        let target_end = target_start
            .checked_add(row_bytes)
            .context("Frozen capture target row end overflow")?;
        let source = canvas
            .get(source_start..source_end)
            .context("Frozen capture buffer is shorter than advertised")?;
        let target = data
            .get_mut(target_start..target_end)
            .context("Frozen capture image allocation is shorter than expected")?;
        target.copy_from_slice(source);
    }

    if format == wl_shm::Format::Xrgb8888 {
        for pixel in data.as_chunks_mut::<4>().0 {
            pixel[3] = 0xff;
        }
    }

    let width = u32::try_from(width).context("Frozen capture width exceeds u32")?;
    let height = u32::try_from(height).context("Frozen capture height exceeds u32")?;
    let stride = i32::try_from(row_bytes).context("Frozen capture packed stride exceeds i32")?;
    Ok(FrozenImage {
        width,
        height,
        stride,
        data,
    })
}

fn transform_argb(
    width: usize,
    height: usize,
    data: &[u8],
    transform: wl_output::Transform,
) -> Option<(usize, usize, Vec<u8>)> {
    if !matches!(
        transform,
        wl_output::Transform::Normal
            | wl_output::Transform::_90
            | wl_output::Transform::_180
            | wl_output::Transform::_270
            | wl_output::Transform::Flipped
            | wl_output::Transform::Flipped90
            | wl_output::Transform::Flipped180
            | wl_output::Transform::Flipped270
    ) {
        return None;
    }
    if data.len() != width.checked_mul(height)?.checked_mul(4)? {
        return None;
    }

    let swaps_axes = matches!(
        transform,
        wl_output::Transform::_90
            | wl_output::Transform::_270
            | wl_output::Transform::Flipped90
            | wl_output::Transform::Flipped270
    );
    let (dest_width, dest_height) = if swaps_axes {
        (height, width)
    } else {
        (width, height)
    };
    let mut transformed = vec![0u8; data.len()];

    for y in 0..height {
        for x in 0..width {
            let (dest_x, dest_y) = transformed_coords(x, y, width, height, transform);
            let src = (y * width + x) * 4;
            let dest = (dest_y * dest_width + dest_x) * 4;
            transformed[dest..dest + 4].copy_from_slice(&data[src..src + 4]);
        }
    }

    Some((dest_width, dest_height, transformed))
}

/// Logical position of buffer pixel `(x, y)` once the output transform is
/// undone. Each flipped transform and 180 degrees is its own inverse, while 90
/// and 270 degrees invert each other.
fn transformed_coords(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    transform: wl_output::Transform,
) -> (usize, usize) {
    match transform {
        wl_output::Transform::Normal => (x, y),
        wl_output::Transform::_90 => (height - 1 - y, x),
        wl_output::Transform::_180 => (width - 1 - x, height - 1 - y),
        wl_output::Transform::_270 => (y, width - 1 - x),
        wl_output::Transform::Flipped => (width - 1 - x, y),
        wl_output::Transform::Flipped90 => (y, x),
        wl_output::Transform::Flipped180 => (x, height - 1 - y),
        wl_output::Transform::Flipped270 => (height - 1 - y, width - 1 - x),
        _ => (x, y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32, values: &[u8]) -> FrozenImage {
        let mut data = Vec::with_capacity(values.len() * 4);
        for value in values {
            data.extend_from_slice(&[*value, 0, 0, 0xff]);
        }

        FrozenImage {
            width,
            height,
            stride: (width * 4) as i32,
            data,
        }
    }

    fn values(image: &FrozenImage) -> Vec<u8> {
        image
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| chunk[0])
            .collect()
    }

    const ALL_TRANSFORMS: [wl_output::Transform; 8] = [
        wl_output::Transform::Normal,
        wl_output::Transform::_90,
        wl_output::Transform::_180,
        wl_output::Transform::_270,
        wl_output::Transform::Flipped,
        wl_output::Transform::Flipped90,
        wl_output::Transform::Flipped180,
        wl_output::Transform::Flipped270,
    ];

    /// grim's `get_output_rotation` in quarter turns and its
    /// `get_output_flipped` x mirror.
    fn grim_rotation(transform: wl_output::Transform) -> (usize, i64) {
        match transform {
            wl_output::Transform::Normal => (0, 1),
            wl_output::Transform::_90 => (1, 1),
            wl_output::Transform::_180 => (2, 1),
            wl_output::Transform::_270 => (3, 1),
            wl_output::Transform::Flipped => (0, -1),
            wl_output::Transform::Flipped90 => (1, -1),
            wl_output::Transform::Flipped180 => (2, -1),
            wl_output::Transform::Flipped270 => (3, -1),
            _ => panic!("grim only defines the eight wl_output transforms"),
        }
    }

    /// grim's `apply_output_transform`: quarter turns swap the buffer axes.
    fn grim_logical_size(
        (width, height): (u32, u32),
        transform: wl_output::Transform,
    ) -> (u32, u32) {
        let (quarter_turns, _) = grim_rotation(transform);

        if quarter_turns % 2 == 1 {
            (height, width)
        } else {
            (width, height)
        }
    }

    /// Logical pixel that grim's `out2com` matrix sends a captured buffer
    /// pixel to.
    ///
    /// grim, the reference wlroots screenshot client, composites screencopy
    /// and `ext-image-copy-capture` buffers by moving the buffer centre to the
    /// origin, mirroring y for `y_invert` frames, rotating by
    /// `get_output_rotation(transform)` through pixman's
    /// `[[cos, -sin], [sin, cos]]` matrix, mirroring x for flipped transforms,
    /// and moving the result to the logical centre. Coordinates are doubled so
    /// pixel centres stay integral.
    fn grim_logical_pixel(
        (x, y): (u32, u32),
        (width, height): (u32, u32),
        transform: wl_output::Transform,
        y_invert: bool,
    ) -> (u32, u32) {
        let (quarter_turns, mirror_x) = grim_rotation(transform);
        let (cos, sin) = [(1, 0), (0, 1), (-1, 0), (0, -1)][quarter_turns];
        let mirror_y = if y_invert { -1 } else { 1 };
        let (logical_width, logical_height) = grim_logical_size((width, height), transform);

        let centred_x = 2 * i64::from(x) + 1 - i64::from(width);
        let centred_y = mirror_y * (2 * i64::from(y) + 1 - i64::from(height));
        let rotated_x = cos * centred_x - sin * centred_y;
        let rotated_y = sin * centred_x + cos * centred_y;
        let logical_x = (mirror_x * rotated_x + i64::from(logical_width) - 1) / 2;
        let logical_y = (rotated_y + i64::from(logical_height) - 1) / 2;

        (
            u32::try_from(logical_x).expect("grim keeps every buffer pixel inside the output"),
            u32::try_from(logical_y).expect("grim keeps every buffer pixel inside the output"),
        )
    }

    #[test]
    fn shm_captures_orient_like_grim_for_every_transform_and_row_order() {
        // Two rows of three distinct pixels, each followed by four padding
        // bytes that the copy must drop.
        let (width, height, stride) = (3, 2, 16);
        let mut canvas = Vec::new();
        for row in [[1, 2, 3], [4, 5, 6]] {
            for value in row {
                canvas.extend_from_slice(&[value, 0, 0, 0xff]);
            }
            canvas.extend_from_slice(&[0xee; 4]);
        }

        for y_invert in [false, true] {
            for transform in ALL_TRANSFORMS {
                let oriented = copy_shm_argb(
                    &canvas,
                    width,
                    height,
                    stride,
                    wl_shm::Format::Argb8888,
                    y_invert,
                )
                .and_then(|image| image.with_output_transform(transform))
                .expect("the padded capture is valid");
                let (logical_width, logical_height) = grim_logical_size((width, height), transform);

                let mut expected = vec![0; 6];
                for (index, value) in (0..).zip(1..=6) {
                    let (x, y) = grim_logical_pixel(
                        (index % width, index / width),
                        (width, height),
                        transform,
                        y_invert,
                    );
                    let logical_index = usize::try_from(y * logical_width + x)
                        .expect("the test image index fits in memory");
                    expected[logical_index] = value;
                }

                assert_eq!(
                    (oriented.width, oriented.height),
                    (logical_width, logical_height),
                    "{transform:?} must swap the axes only for quarter turns"
                );
                assert_eq!(
                    oriented.stride,
                    i32::try_from(logical_width * 4).expect("the test stride fits in i32"),
                    "{transform:?} must leave tightly packed rows"
                );
                assert_eq!(
                    values(&oriented),
                    expected,
                    "{transform:?} with y_invert={y_invert} must place pixels where grim does"
                );
            }
        }
    }

    #[test]
    fn output_transform_rejects_an_invalid_pixel_buffer() {
        let malformed = FrozenImage {
            width: 2,
            height: 2,
            stride: 8,
            data: vec![0; 12],
        };

        assert!(
            malformed
                .with_output_transform(wl_output::Transform::_90)
                .is_err()
        );
    }

    #[test]
    fn output_transform_270_rotates_buffer_counter_clockwise_into_logical_orientation() {
        let transformed = image(3, 2, &[1, 2, 3, 4, 5, 6])
            .with_output_transform(wl_output::Transform::_270)
            .expect("valid transform");

        assert_eq!(
            (transformed.width, transformed.height),
            (2, 3),
            "a quarter turn swaps the buffer axes"
        );
        assert_eq!(
            values(&transformed),
            vec![3, 6, 2, 5, 1, 4],
            "the last buffer column, read top to bottom, is the logical top row"
        );
        assert_eq!(transformed.stride, 8, "rotated rows stay tightly packed");
    }

    #[test]
    fn output_transform_90_rotates_buffer_clockwise_into_logical_orientation() {
        // A 90 degree output's framebuffer holds the logical image rotated 90
        // degrees counter-clockwise: its first buffer column, read bottom to
        // top, is the logical top row. Rotating it the same way again leaves
        // the frozen image upside down (#446).
        let transformed = image(3, 2, &[1, 2, 3, 4, 5, 6])
            .with_output_transform(wl_output::Transform::_90)
            .expect("valid transform");

        assert_eq!(
            (transformed.width, transformed.height),
            (2, 3),
            "a quarter turn swaps the buffer axes"
        );
        assert_eq!(
            values(&transformed),
            vec![4, 1, 5, 2, 6, 3],
            "the first buffer column, read bottom to top, is the logical top row"
        );
    }

    #[test]
    fn flipped_transform_mirrors_pixels() {
        let transformed = image(3, 2, &[1, 2, 3, 4, 5, 6])
            .with_output_transform(wl_output::Transform::Flipped)
            .expect("valid transform");

        assert_eq!((transformed.width, transformed.height), (3, 2));
        assert_eq!(values(&transformed), vec![3, 2, 1, 6, 5, 4]);
    }

    #[test]
    fn shm_copy_removes_padding_and_applies_y_invert() {
        let image = copy_shm_argb(
            &[
                1, 0, 0, 255, 2, 0, 0, 255, 9, 9, 9, 9, //
                3, 0, 0, 255, 4, 0, 0, 255, 8, 8, 8, 8,
            ],
            2,
            2,
            12,
            wl_shm::Format::Argb8888,
            true,
        )
        .expect("valid padded SHM buffer");

        assert_eq!(values(&image), vec![3, 4, 1, 2]);
        assert_eq!(image.stride, 8);
    }

    #[test]
    fn shm_copy_makes_xrgb_pixels_opaque_and_rejects_short_buffers() {
        let image = copy_shm_argb(&[1, 2, 3, 0], 1, 1, 4, wl_shm::Format::Xrgb8888, false)
            .expect("valid XRGB buffer");
        assert_eq!(image.data, vec![1, 2, 3, 255]);

        assert!(copy_shm_argb(&[1, 2, 3], 1, 1, 4, wl_shm::Format::Argb8888, false,).is_err());
    }

    #[test]
    fn shm_layout_rejects_invalid_compositor_dimensions_before_allocation() {
        assert!(validate_shm_buffer_layout(0, 1, 4).is_err());
        assert!(validate_shm_buffer_layout(1, 0, 4).is_err());
        assert!(validate_shm_buffer_layout(2, 1, 4).is_err());
        assert!(validate_shm_buffer_layout(u32::MAX, 1, u32::MAX).is_err());
        assert!(validate_shm_buffer_layout(1, 1, u32::MAX).is_err());
    }

    #[test]
    fn shm_layout_accepts_padding_and_returns_checked_wayland_values() {
        let layout = validate_shm_buffer_layout(2, 3, 12)
            .expect("the test establishes positive in-range dimensions and sufficient stride");

        assert_eq!(layout.width, 2);
        assert_eq!(layout.height, 3);
        assert_eq!(layout.stride, 12);
        assert_eq!(layout.total_size, 36);
    }
}
