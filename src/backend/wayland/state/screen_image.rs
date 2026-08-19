//! The captured desktop image the canvas renderer actually displays.
//!
//! The screen eyedropper samples one pixel of it and OCR crops a rectangle out
//! of it. Both must read exactly what Wayscriber is showing rather than a fresh
//! screenshot, so source resolution, the logical→image mapping, and the checked
//! rectangle copy live here instead of inside either feature.

use crate::backend::wayland::frozen::{FrozenImage, FrozenState};
use crate::backend::wayland::zoom::ZoomState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScreenImageKind {
    Zoom,
    Frozen,
}

/// The captured image that the canvas renderer actually displays.
pub(super) struct DisplayedScreenImage<'a> {
    pub image: &'a FrozenImage,
    pub kind: ScreenImageKind,
    /// The renderer is painting this image through the zoom transform, so
    /// screen points must be un-zoomed before they are scaled into the image.
    pub zoom_transformed: bool,
}

/// A rectangle in source-image pixels, guaranteed non-empty and in bounds of
/// the image it was resolved against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ImageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CropError {
    /// The rectangle has no pixels, or the source image is empty.
    Empty,
    /// The rectangle leaves the source image, or its size does not fit memory.
    OutOfBounds,
}

/// What a modal that needs the displayed screen image should do when the user
/// asks for it. Shared by the eyedropper and OCR so the two cannot disagree
/// about when a temporary freeze is created or refused; each owns only the
/// wording it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScreenSourceEntry {
    Activate,
    WaitForZoom,
    AutoFreeze,
    RefuseWhileZoomedOnSolidBoard,
    RefuseSolidBoard,
    CaptureUnavailable,
    ZoomImageUnavailable,
}

pub(super) fn screen_source_entry(
    has_source: bool,
    board_is_transparent: bool,
    zoom_engaged: bool,
    zoom_active: bool,
    frozen_enabled: bool,
) -> ScreenSourceEntry {
    if has_source {
        ScreenSourceEntry::Activate
    } else if !board_is_transparent && zoom_engaged {
        ScreenSourceEntry::RefuseWhileZoomedOnSolidBoard
    } else if !board_is_transparent {
        ScreenSourceEntry::RefuseSolidBoard
    } else if zoom_engaged && !zoom_active {
        ScreenSourceEntry::WaitForZoom
    } else if !frozen_enabled {
        ScreenSourceEntry::CaptureUnavailable
    } else if zoom_active {
        ScreenSourceEntry::ZoomImageUnavailable
    } else {
        ScreenSourceEntry::AutoFreeze
    }
}

pub(super) fn displayed_screen_image<'a>(
    zoom: &'a ZoomState,
    frozen: &'a FrozenState,
    board_is_transparent: bool,
) -> Option<DisplayedScreenImage<'a>> {
    let allow_background_image = !zoom.is_engaged() || board_is_transparent;
    if !allow_background_image {
        return None;
    }

    if zoom.active {
        if let Some(image) = zoom.image() {
            return Some(DisplayedScreenImage {
                image,
                kind: ScreenImageKind::Zoom,
                zoom_transformed: true,
            });
        }
        if let Some(image) = frozen.image() {
            return Some(DisplayedScreenImage {
                image,
                kind: ScreenImageKind::Frozen,
                zoom_transformed: true,
            });
        }
    }

    frozen.image().map(|image| DisplayedScreenImage {
        image,
        kind: ScreenImageKind::Frozen,
        zoom_transformed: false,
    })
}

/// Map a logical screen point onto source-image pixel coordinates. The result
/// is unclamped on purpose: samplers clamp to the nearest pixel, while region
/// selection clips against the image bounds.
pub(super) fn image_point_for_screen_point(
    source: &DisplayedScreenImage<'_>,
    zoom: &ZoomState,
    surface: (u32, u32),
    point: (f64, f64),
) -> (f64, f64) {
    let (world_x, world_y) = if source.zoom_transformed {
        zoom.screen_to_world(point.0, point.1)
    } else {
        point
    };
    (
        world_x * f64::from(source.image.width) / f64::from(surface.0).max(1.0),
        world_y * f64::from(source.image.height) / f64::from(surface.1).max(1.0),
    )
}

/// Map a logical screen rectangle (given by any two opposite corners) onto a
/// non-empty in-bounds source-image rectangle, or `None` when the two corners
/// resolve to nothing the image can supply.
pub(super) fn image_rect_for_screen_rect(
    source: &DisplayedScreenImage<'_>,
    zoom: &ZoomState,
    surface: (u32, u32),
    start: (f64, f64),
    end: (f64, f64),
) -> Option<ImageRect> {
    let first = image_point_for_screen_point(source, zoom, surface, start);
    let second = image_point_for_screen_point(source, zoom, surface, end);
    image_rect_from_points(first, second, source.image.width, source.image.height)
}

/// Clip two unordered image-space points into an integer pixel rectangle.
/// Reversed drags, partly out-of-bounds drags, and non-finite coordinates all
/// resolve deterministically here rather than at the call sites.
fn image_rect_from_points(
    first: (f64, f64),
    second: (f64, f64),
    image_width: u32,
    image_height: u32,
) -> Option<ImageRect> {
    if image_width == 0 || image_height == 0 {
        return None;
    }
    if ![first.0, first.1, second.0, second.1]
        .iter()
        .all(|value| value.is_finite())
    {
        return None;
    }

    let width = f64::from(image_width);
    let height = f64::from(image_height);
    let left = first.0.min(second.0).floor().clamp(0.0, width);
    let top = first.1.min(second.1).floor().clamp(0.0, height);
    let right = first.0.max(second.0).ceil().clamp(0.0, width);
    let bottom = first.1.max(second.1).ceil().clamp(0.0, height);
    if right <= left || bottom <= top {
        return None;
    }

    Some(ImageRect {
        x: left as u32,
        y: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    })
}

/// Copy `rect` out of `source` into a tightly packed owned ARGB snapshot.
///
/// The source may carry row padding; the copy never does, so the worker can
/// hand it straight to Cairo without re-deriving a stride.
pub(super) fn copy_image_rect(
    source: &FrozenImage,
    rect: ImageRect,
) -> Result<crate::ocr::OcrPixels, CropError> {
    if rect.width == 0 || rect.height == 0 || source.width == 0 || source.height == 0 {
        return Err(CropError::Empty);
    }
    if rect
        .x
        .checked_add(rect.width)
        .ok_or(CropError::OutOfBounds)?
        > source.width
        || rect
            .y
            .checked_add(rect.height)
            .ok_or(CropError::OutOfBounds)?
            > source.height
    {
        return Err(CropError::OutOfBounds);
    }
    let source_stride = usize::try_from(source.stride).map_err(|_| CropError::OutOfBounds)?;
    let row_bytes = usize::try_from(rect.width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or(CropError::OutOfBounds)?;
    let total_bytes = usize::try_from(rect.height)
        .ok()
        .and_then(|height| height.checked_mul(row_bytes))
        .ok_or(CropError::OutOfBounds)?;
    let stride = i32::try_from(row_bytes).map_err(|_| CropError::OutOfBounds)?;

    let mut data = Vec::with_capacity(total_bytes);
    let start_column = usize::try_from(rect.x)
        .ok()
        .and_then(|x| x.checked_mul(4))
        .ok_or(CropError::OutOfBounds)?;
    for row in 0..rect.height {
        let source_row = usize::try_from(rect.y.saturating_add(row))
            .ok()
            .and_then(|row| row.checked_mul(source_stride))
            .ok_or(CropError::OutOfBounds)?;
        let start = source_row
            .checked_add(start_column)
            .ok_or(CropError::OutOfBounds)?;
        let end = start.checked_add(row_bytes).ok_or(CropError::OutOfBounds)?;
        data.extend_from_slice(source.data.get(start..end).ok_or(CropError::OutOfBounds)?);
    }

    Ok(crate::ocr::OcrPixels {
        width: rect.width,
        height: rect.height,
        stride,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32, stride: i32, data: Vec<u8>) -> FrozenImage {
        FrozenImage {
            width,
            height,
            stride,
            data,
        }
    }

    fn opaque(width: u32, height: u32) -> FrozenImage {
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                data.extend_from_slice(&[x as u8, y as u8, 0, 0xFF]);
            }
        }
        image(width, height, (width * 4) as i32, data)
    }

    fn zoom_state() -> ZoomState {
        ZoomState::new(None)
    }

    #[test]
    fn displayed_source_truth_table_matches_the_rendering_contract() {
        let mut zoom = zoom_state();
        let mut frozen = FrozenState::new(None);
        assert!(displayed_screen_image(&zoom, &frozen, true).is_none());

        frozen.set_image(opaque(2, 2));
        let source = displayed_screen_image(&zoom, &frozen, false).unwrap();
        assert_eq!(source.kind, ScreenImageKind::Frozen);
        assert!(!source.zoom_transformed);

        zoom.active = true;
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();
        assert_eq!(source.kind, ScreenImageKind::Frozen);
        assert!(source.zoom_transformed);
        assert!(displayed_screen_image(&zoom, &frozen, false).is_none());

        zoom.set_image(opaque(2, 2));
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();
        assert_eq!(source.kind, ScreenImageKind::Zoom);
        assert!(source.zoom_transformed);

        zoom.active = false;
        zoom.request_activation();
        assert!(displayed_screen_image(&zoom, &frozen, false).is_none());
    }

    #[test]
    fn unzoomed_points_scale_by_the_image_to_surface_ratio() {
        let mut frozen = FrozenState::new(None);
        frozen.set_image(opaque(200, 100));
        let zoom = zoom_state();
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();

        let point = image_point_for_screen_point(&source, &zoom, (100, 50), (50.0, 25.0));
        assert!((point.0 - 100.0).abs() < f64::EPSILON);
        assert!((point.1 - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zoomed_points_use_the_same_transform_the_renderer_applied() {
        let mut zoom = zoom_state();
        zoom.active = true;
        zoom.set_image(opaque(100, 100));
        zoom.scale = 2.0;
        zoom.view_offset = (10.0, 20.0);
        let frozen = FrozenState::new(None);
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();

        let point = image_point_for_screen_point(&source, &zoom, (100, 100), (40.0, 60.0));
        // screen_to_world(40, 60) = (10 + 20, 20 + 30) with a 1:1 image scale.
        assert!((point.0 - 30.0).abs() < f64::EPSILON);
        assert!((point.1 - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn reversed_drags_normalize_and_partly_offscreen_drags_clip() {
        let rect = image_rect_from_points((30.0, 40.0), (10.0, 20.0), 100, 100).unwrap();
        assert_eq!(
            rect,
            ImageRect {
                x: 10,
                y: 20,
                width: 20,
                height: 20,
            }
        );

        let clipped = image_rect_from_points((-50.0, -50.0), (10.5, 10.5), 100, 100).unwrap();
        assert_eq!(
            clipped,
            ImageRect {
                x: 0,
                y: 0,
                width: 11,
                height: 11,
            }
        );
    }

    #[test]
    fn degenerate_rectangles_are_rejected_instead_of_clamped_to_one_pixel() {
        assert!(image_rect_from_points((10.0, 10.0), (10.0, 10.0), 100, 100).is_none());
        assert!(image_rect_from_points((-20.0, -20.0), (-5.0, -5.0), 100, 100).is_none());
        assert!(image_rect_from_points((0.0, 0.0), (10.0, 10.0), 0, 10).is_none());
        assert!(image_rect_from_points((f64::NAN, 0.0), (10.0, 10.0), 100, 100).is_none());
        assert!(image_rect_from_points((0.0, 0.0), (f64::INFINITY, 10.0), 100, 100).is_none());
    }

    #[test]
    fn crop_repacks_a_padded_source_row_by_row() {
        let source = image(
            2,
            2,
            12,
            vec![
                1, 1, 1, 255, 2, 2, 2, 255, 9, 9, 9, 9, //
                3, 3, 3, 255, 4, 4, 4, 255, 8, 8, 8, 8,
            ],
        );

        let crop = copy_image_rect(
            &source,
            ImageRect {
                x: 1,
                y: 0,
                width: 1,
                height: 2,
            },
        )
        .unwrap();

        assert_eq!((crop.width, crop.height, crop.stride), (1, 2, 4));
        assert_eq!(crop.data, vec![2, 2, 2, 255, 4, 4, 4, 255]);
    }

    #[test]
    fn crop_preserves_premultiplied_and_transparent_pixels_verbatim() {
        let source = image(2, 1, 8, vec![25, 50, 100, 128, 0, 0, 0, 0]);

        let crop = copy_image_rect(
            &source,
            ImageRect {
                x: 0,
                y: 0,
                width: 2,
                height: 1,
            },
        )
        .unwrap();

        assert_eq!(crop.data, vec![25, 50, 100, 128, 0, 0, 0, 0]);
    }

    #[test]
    fn crop_rejects_empty_and_out_of_bounds_rectangles() {
        let source = opaque(4, 4);
        let error = |rect| copy_image_rect(&source, rect).err();

        assert_eq!(
            error(ImageRect {
                x: 0,
                y: 0,
                width: 0,
                height: 2,
            }),
            Some(CropError::Empty)
        );
        assert_eq!(
            error(ImageRect {
                x: 3,
                y: 0,
                width: 2,
                height: 1,
            }),
            Some(CropError::OutOfBounds)
        );
        assert_eq!(
            error(ImageRect {
                x: u32::MAX,
                y: 0,
                width: 2,
                height: 1,
            }),
            Some(CropError::OutOfBounds)
        );
    }

    #[test]
    fn crop_of_a_rotated_capture_keeps_the_transformed_orientation() {
        use wayland_client::protocol::wl_output;

        // Two rows of three distinct pixels; rotating to 270 makes the source
        // 2x3, and the crop must read that post-transform layout.
        let mut data = Vec::new();
        for value in [1u8, 2, 3, 4, 5, 6] {
            data.extend_from_slice(&[value, 0, 0, 0xFF]);
        }
        let rotated = image(3, 2, 12, data)
            .with_output_transform(wl_output::Transform::_270)
            .expect("valid transform");
        assert_eq!((rotated.width, rotated.height), (2, 3));

        let crop = copy_image_rect(
            &rotated,
            ImageRect {
                x: 0,
                y: 0,
                width: 2,
                height: 1,
            },
        )
        .unwrap();

        assert_eq!(
            crop.data.chunks_exact(4).map(|p| p[0]).collect::<Vec<_>>(),
            vec![4, 1]
        );
    }
}
