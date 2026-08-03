use anyhow::{Context, Result};
use wayland_client::protocol::{wl_output, wl_shm};

/// CPU-side frozen image ready for Cairo rendering.
pub struct FrozenImage {
    pub width: u32,
    pub height: u32,
    pub stride: i32,
    pub data: Vec<u8>,
}

impl FrozenImage {
    /// Apply the wl_output transform advertised for the captured output.
    ///
    /// WLR screencopy buffers are returned in output framebuffer coordinates.
    /// Frozen/zoom rendering paints them onto a logical surface, so rotated or
    /// flipped outputs need the same transform applied to the captured pixels.
    pub fn with_output_transform(mut self, transform: wl_output::Transform) -> Self {
        if transform == wl_output::Transform::Normal {
            return self;
        }

        if let Some((width, height, data)) = transform_argb(
            self.width as usize,
            self.height as usize,
            &self.data,
            transform,
        ) {
            self.width = width as u32;
            self.height = height as u32;
            self.stride = (width * 4) as i32;
            self.data = data;
        }

        self
    }
}

/// Copy a compositor-provided SHM buffer into tightly packed Cairo-compatible
/// ARGB data while validating every advertised dimension and row boundary.
pub(super) fn copy_shm_argb(
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
        for pixel in data.chunks_exact_mut(4) {
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

fn transformed_coords(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    transform: wl_output::Transform,
) -> (usize, usize) {
    match transform {
        wl_output::Transform::Normal => (x, y),
        wl_output::Transform::_90 => (y, width - 1 - x),
        wl_output::Transform::_180 => (width - 1 - x, height - 1 - y),
        wl_output::Transform::_270 => (height - 1 - y, x),
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
            data.extend_from_slice(&[*value, 0, 0, 0xFF]);
        }

        FrozenImage {
            width,
            height,
            stride: (width * 4) as i32,
            data,
        }
    }

    fn values(image: &FrozenImage) -> Vec<u8> {
        image.data.chunks_exact(4).map(|chunk| chunk[0]).collect()
    }

    #[test]
    fn output_transform_270_rotates_into_logical_orientation() {
        let transformed =
            image(3, 2, &[1, 2, 3, 4, 5, 6]).with_output_transform(wl_output::Transform::_270);

        assert_eq!((transformed.width, transformed.height), (2, 3));
        assert_eq!(values(&transformed), vec![4, 1, 5, 2, 6, 3]);
        assert_eq!(transformed.stride, 8);
    }

    #[test]
    fn output_transform_90_rotates_into_logical_orientation() {
        let transformed =
            image(3, 2, &[1, 2, 3, 4, 5, 6]).with_output_transform(wl_output::Transform::_90);

        assert_eq!((transformed.width, transformed.height), (2, 3));
        assert_eq!(values(&transformed), vec![3, 6, 2, 5, 1, 4]);
    }

    #[test]
    fn flipped_transform_mirrors_pixels() {
        let transformed =
            image(3, 2, &[1, 2, 3, 4, 5, 6]).with_output_transform(wl_output::Transform::Flipped);

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
}
