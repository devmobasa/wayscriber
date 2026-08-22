//! The captured desktop image the canvas renderer actually displays.
//!
//! The screen eyedropper samples one pixel of it and OCR crops a rectangle out
//! of it. Both must read exactly what Wayscriber is showing rather than a fresh
//! screenshot, so source resolution, the logical→image mapping, and the checked
//! rectangle copy live here instead of inside either feature.

use crate::backend::wayland::frozen::{FrozenImage, FrozenState, ScreenImageProvenance};
use crate::backend::wayland::zoom::ZoomState;
use crate::screen_pixels::{ImagePixelRect, ImagePoint, PixelSpan};
use crate::util::Rect;
use wayland_client::protocol::wl_output;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScreenImageKind {
    Zoom,
    Frozen,
}

/// The captured image that the canvas renderer actually displays.
pub(super) struct DisplayedScreenImage<'a> {
    pub image: &'a FrozenImage,
    pub kind: ScreenImageKind,
    pub provenance: ScreenImageProvenance,
    /// The renderer is painting this image through the zoom transform, so
    /// screen points must be un-zoomed before they are scaled into the image.
    pub zoom_transformed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ScreenSourceToken {
    pub output_id: u32,
    pub output_layout_generation: u64,
    pub kind: ScreenImageKind,
    pub image_generation: u64,
    pub image_size: (u32, u32),
    pub stride: i32,
    pub surface: (u32, u32),
    pub output_scale: i32,
    pub output_transform: wl_output::Transform,
    pub zoom_transformed: bool,
    pub zoom_scale: f64,
    pub zoom_view_offset: (f64, f64),
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
            return zoom
                .image_provenance()
                .map(|provenance| DisplayedScreenImage {
                    image,
                    kind: ScreenImageKind::Zoom,
                    provenance,
                    zoom_transformed: true,
                });
        }
        if let Some(image) = frozen.image() {
            return frozen
                .image_provenance()
                .map(|provenance| DisplayedScreenImage {
                    image,
                    kind: ScreenImageKind::Frozen,
                    provenance,
                    zoom_transformed: true,
                });
        }
    }

    let image = frozen.image()?;
    frozen
        .image_provenance()
        .map(|provenance| DisplayedScreenImage {
            image,
            kind: ScreenImageKind::Frozen,
            provenance,
            zoom_transformed: false,
        })
}

/// Map a logical screen point onto source-image pixel coordinates. The result
/// is unclamped on purpose: samplers clamp to the nearest pixel, while region
/// selection clips against the image bounds.
pub(super) fn screen_source_token(
    source: &DisplayedScreenImage<'_>,
    zoom: &ZoomState,
    frozen: &FrozenState,
    surface: (u32, u32),
) -> ScreenSourceToken {
    let provenance = source.provenance;
    let (zoom_scale, zoom_view_offset) = if source.zoom_transformed {
        (zoom.scale, zoom.view_offset)
    } else {
        (1.0, (0.0, 0.0))
    };
    ScreenSourceToken {
        output_id: provenance.output_id,
        output_layout_generation: provenance.output_layout_generation,
        kind: source.kind,
        image_generation: match source.kind {
            ScreenImageKind::Zoom => zoom.image_generation(),
            ScreenImageKind::Frozen => frozen.image_generation(),
        },
        image_size: (source.image.width, source.image.height),
        stride: source.image.stride,
        surface,
        output_scale: provenance.output_scale,
        output_transform: provenance.output_transform,
        zoom_transformed: source.zoom_transformed,
        zoom_scale,
        zoom_view_offset,
    }
}

pub(super) fn screen_source_is(
    expected: &ScreenSourceToken,
    source: &DisplayedScreenImage<'_>,
    zoom: &ZoomState,
    frozen: &FrozenState,
    surface: (u32, u32),
) -> bool {
    current_screen_source_token(source, zoom, frozen, surface) == Some(*expected)
}

pub(super) fn current_screen_source_token(
    source: &DisplayedScreenImage<'_>,
    zoom: &ZoomState,
    frozen: &FrozenState,
    surface: (u32, u32),
) -> Option<ScreenSourceToken> {
    let context_matches = match source.kind {
        ScreenImageKind::Zoom => zoom.source_context_matches(source.provenance),
        ScreenImageKind::Frozen => frozen.source_context_matches(source.provenance),
    };
    context_matches.then(|| screen_source_token(source, zoom, frozen, surface))
}

pub(super) fn image_point_for_screen_point(
    token: &ScreenSourceToken,
    point: (f64, f64),
) -> ImagePoint {
    let (world_x, world_y) = if token.zoom_transformed {
        (
            token.zoom_view_offset.0 + point.0 / token.zoom_scale,
            token.zoom_view_offset.1 + point.1 / token.zoom_scale,
        )
    } else {
        point
    };
    ImagePoint::new(
        world_x * f64::from(token.image_size.0) / f64::from(token.surface.0).max(1.0),
        world_y * f64::from(token.image_size.1) / f64::from(token.surface.1).max(1.0),
    )
}

pub(super) fn screen_point_for_image_point(
    token: &ScreenSourceToken,
    point: ImagePoint,
) -> (f64, f64) {
    let world = (
        point.x * f64::from(token.surface.0) / f64::from(token.image_size.0).max(1.0),
        point.y * f64::from(token.surface.1) / f64::from(token.image_size.1).max(1.0),
    );
    if token.zoom_transformed {
        (
            (world.0 - token.zoom_view_offset.0) * token.zoom_scale,
            (world.1 - token.zoom_view_offset.1) * token.zoom_scale,
        )
    } else {
        world
    }
}

pub(super) fn screen_rect_for_image_rect(token: &ScreenSourceToken, rect: ImagePixelRect) -> Rect {
    let first = screen_point_for_image_point(
        token,
        ImagePoint::new(f64::from(rect.x()), f64::from(rect.y())),
    );
    let second = screen_point_for_image_point(
        token,
        ImagePoint::new(
            f64::from(rect.x() + rect.width()),
            f64::from(rect.y() + rect.height()),
        ),
    );
    let left = first.0.min(second.0).floor() as i32;
    let top = first.1.min(second.1).floor() as i32;
    let right = first.0.max(second.0).ceil() as i32;
    let bottom = first.1.max(second.1).ceil() as i32;
    Rect::from_min_max(left, top, right, bottom)
        .expect("a non-empty image rectangle must map to a non-empty screen rectangle")
}

#[allow(dead_code)] // The empty-span contract is exercised by mapping tests.
pub(super) fn screen_rect_for_pixel_span(
    token: &ScreenSourceToken,
    span: PixelSpan,
) -> Option<Rect> {
    Some(screen_rect_for_image_rect(token, span.try_into().ok()?))
}

/// Clip two unordered image-space points into an integer pixel rectangle.
/// Reversed drags, partly out-of-bounds drags, and non-finite coordinates all
/// resolve deterministically here rather than at the call sites.
#[cfg(test)]
fn image_rect_from_points(
    first: (f64, f64),
    second: (f64, f64),
    image_width: u32,
    image_height: u32,
) -> Option<ImagePixelRect> {
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

    ImagePixelRect::new(
        left as u32,
        top as u32,
        (right - left) as u32,
        (bottom - top) as u32,
        (image_width, image_height),
    )
}

/// Copy `rect` out of `source` into a tightly packed owned ARGB snapshot.
///
/// The source may carry row padding; the copy never does, so the worker can
/// hand it straight to Cairo without re-deriving a stride.
pub(super) fn copy_image_rect(
    source: &FrozenImage,
    rect: ImagePixelRect,
) -> Result<crate::screen_pixels::PackedArgb32, CropError> {
    if source.width == 0 || source.height == 0 {
        return Err(CropError::Empty);
    }
    if rect
        .x()
        .checked_add(rect.width())
        .ok_or(CropError::OutOfBounds)?
        > source.width
        || rect
            .y()
            .checked_add(rect.height())
            .ok_or(CropError::OutOfBounds)?
            > source.height
    {
        return Err(CropError::OutOfBounds);
    }
    let source_stride = usize::try_from(source.stride).map_err(|_| CropError::OutOfBounds)?;
    let row_bytes = usize::try_from(rect.width())
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or(CropError::OutOfBounds)?;
    let total_bytes = usize::try_from(rect.height())
        .ok()
        .and_then(|height| height.checked_mul(row_bytes))
        .ok_or(CropError::OutOfBounds)?;
    let stride = i32::try_from(row_bytes).map_err(|_| CropError::OutOfBounds)?;

    let mut data = Vec::with_capacity(total_bytes);
    let start_column = usize::try_from(rect.x())
        .ok()
        .and_then(|x| x.checked_mul(4))
        .ok_or(CropError::OutOfBounds)?;
    for row in 0..rect.height() {
        let source_row = usize::try_from(rect.y().saturating_add(row))
            .ok()
            .and_then(|row| row.checked_mul(source_stride))
            .ok_or(CropError::OutOfBounds)?;
        let start = source_row
            .checked_add(start_column)
            .ok_or(CropError::OutOfBounds)?;
        let end = start.checked_add(row_bytes).ok_or(CropError::OutOfBounds)?;
        data.extend_from_slice(source.data.get(start..end).ok_or(CropError::OutOfBounds)?);
    }

    crate::screen_pixels::PackedArgb32::new(rect.width(), rect.height(), stride, data)
        .ok_or(CropError::OutOfBounds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::frozen::ScreenImageProvenance;

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

    fn provenance(
        output_id: u32,
        output_layout_generation: u64,
        output_scale: i32,
        output_transform: wl_output::Transform,
    ) -> ScreenImageProvenance {
        ScreenImageProvenance::new(
            output_id,
            output_layout_generation,
            output_scale,
            output_transform,
        )
        .unwrap()
    }

    fn install_frozen(frozen: &mut FrozenState, image: FrozenImage) {
        frozen.set_image_with_provenance_for_test(
            image,
            provenance(1, 1, 1, wl_output::Transform::Normal),
        );
    }

    fn install_zoom(zoom: &mut ZoomState, image: FrozenImage) {
        zoom.set_image_with_provenance_for_test(
            image,
            provenance(1, 1, 1, wl_output::Transform::Normal),
        );
    }

    #[test]
    fn displayed_source_truth_table_matches_the_rendering_contract() {
        let mut zoom = zoom_state();
        let mut frozen = FrozenState::new(None);
        assert!(displayed_screen_image(&zoom, &frozen, true).is_none());

        install_frozen(&mut frozen, opaque(2, 2));
        let source = displayed_screen_image(&zoom, &frozen, false).unwrap();
        assert_eq!(source.kind, ScreenImageKind::Frozen);
        assert!(!source.zoom_transformed);

        zoom.active = true;
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();
        assert_eq!(source.kind, ScreenImageKind::Frozen);
        assert!(source.zoom_transformed);
        assert!(displayed_screen_image(&zoom, &frozen, false).is_none());

        install_zoom(&mut zoom, opaque(2, 2));
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
        install_frozen(&mut frozen, opaque(200, 100));
        let zoom = zoom_state();
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();

        let token = screen_source_token(&source, &zoom, &frozen, (100, 50));
        let point = image_point_for_screen_point(&token, (50.0, 25.0));
        assert!((point.x - 100.0).abs() < f64::EPSILON);
        assert!((point.y - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zoomed_points_use_the_same_transform_the_renderer_applied() {
        let mut zoom = zoom_state();
        zoom.active = true;
        install_zoom(&mut zoom, opaque(100, 100));
        zoom.scale = 2.0;
        zoom.view_offset = (10.0, 20.0);
        let frozen = FrozenState::new(None);
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();

        let token = screen_source_token(&source, &zoom, &frozen, (100, 100));
        let point = image_point_for_screen_point(&token, (40.0, 60.0));
        // screen_to_world(40, 60) = (10 + 20, 20 + 30) with a 1:1 image scale.
        assert!((point.x - 30.0).abs() < f64::EPSILON);
        assert!((point.y - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn image_and_screen_points_round_trip_at_native_integer_and_fractional_scale() {
        for image_size in [(100, 50), (200, 100), (150, 75)] {
            let mut frozen = FrozenState::new(None);
            install_frozen(&mut frozen, opaque(image_size.0, image_size.1));
            let zoom = zoom_state();
            let source = displayed_screen_image(&zoom, &frozen, true).unwrap();
            let token = screen_source_token(&source, &zoom, &frozen, (100, 50));

            let image_point = ImagePoint::new(23.5, 17.25);
            let screen_point = screen_point_for_image_point(&token, image_point);
            let round_trip = image_point_for_screen_point(&token, screen_point);

            assert!((round_trip.x - image_point.x).abs() < 1e-12);
            assert!((round_trip.y - image_point.y).abs() < 1e-12);
        }
    }

    #[test]
    fn token_records_the_frozen_outputs_complete_identity() {
        use crate::backend::wayland::frozen_geometry::OutputGeometry;
        use wayland_client::protocol::wl_output;

        let mut frozen = FrozenState::new(None);
        frozen.set_active_output(None, Some(42));
        frozen.set_active_geometry(OutputGeometry::update_from(
            Some((0, 0)),
            Some((100, 50)),
            (100, 50),
            2,
            wl_output::Transform::_90,
            Some((100, 200)),
        ));
        frozen.set_image_with_provenance_for_test(
            opaque(100, 200),
            provenance(42, 1, 2, wl_output::Transform::_90),
        );
        let zoom = zoom_state();
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();

        let token = screen_source_token(&source, &zoom, &frozen, (100, 50));

        assert_eq!(token.output_id, 42);
        assert_eq!(token.output_layout_generation, 1);
        assert_eq!(token.output_scale, 2);
        assert_eq!(token.output_transform, wl_output::Transform::_90);
    }

    #[test]
    fn fractional_transformed_zoom_mapping_and_pixel_span_inverse_are_stable() {
        use crate::backend::wayland::frozen_geometry::OutputGeometry;
        use crate::screen_pixels::pixel_span;
        use crate::util::Rect;
        use wayland_client::protocol::wl_output;

        let frozen = FrozenState::new(None);
        let mut zoom = zoom_state();
        zoom.set_active_output(None, Some(7));
        zoom.set_active_geometry(OutputGeometry::update_from(
            Some((0, 0)),
            Some((200, 100)),
            (200, 100),
            2,
            wl_output::Transform::_90,
            Some((300, 150)),
        ));
        zoom.active = true;
        zoom.set_image_with_provenance_for_test(
            opaque(300, 150),
            provenance(7, 1, 2, wl_output::Transform::_90),
        );
        zoom.scale = 2.0;
        zoom.view_offset = (100.0, 50.0);
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();
        let token = screen_source_token(&source, &zoom, &frozen, (200, 100));

        assert_eq!(token.output_id, 7);
        assert_eq!(token.output_layout_generation, 1);
        assert_eq!(token.output_scale, 2);
        assert_eq!(token.output_transform, wl_output::Transform::_90);
        let screen_point = screen_point_for_image_point(&token, ImagePoint::new(180.0, 90.0));
        assert_eq!(screen_point, (40.0, 20.0));
        assert_eq!(
            screen_rect_for_image_rect(
                &token,
                ImagePixelRect::new(150, 75, 30, 15, (300, 150)).unwrap(),
            ),
            Rect::new(0, 0, 40, 20).unwrap()
        );

        let span = pixel_span(
            ImagePoint::new(150.0, 75.0),
            ImagePoint::new(180.0, 90.0),
            token.image_size,
        )
        .unwrap();
        assert_eq!(
            screen_rect_for_pixel_span(&token, span),
            Rect::new(0, 0, 40, 20)
        );
        let empty = pixel_span(
            ImagePoint::new(150.0, 75.0),
            ImagePoint::new(150.0, 75.0),
            token.image_size,
        )
        .unwrap();
        assert_eq!(screen_rect_for_pixel_span(&token, empty), None);
    }

    #[test]
    fn source_identity_rejects_live_output_layout_view_and_image_replacement() {
        use crate::backend::wayland::frozen_geometry::OutputGeometry;
        use wayland_client::protocol::wl_output;

        let mut frozen = FrozenState::new(None);
        frozen.set_active_output(None, Some(11));
        frozen.set_active_geometry(OutputGeometry::update_from(
            Some((0, 0)),
            Some((100, 50)),
            (100, 50),
            1,
            wl_output::Transform::Normal,
            Some((100, 50)),
        ));
        frozen.set_image_with_provenance_for_test(
            opaque(100, 50),
            provenance(11, 1, 1, wl_output::Transform::Normal),
        );
        let mut zoom = zoom_state();
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();
        let token = screen_source_token(&source, &zoom, &frozen, (100, 50));
        assert!(screen_source_is(&token, &source, &zoom, &frozen, (100, 50)));
        assert!(!screen_source_is(
            &token,
            &source,
            &zoom,
            &frozen,
            (101, 50)
        ));

        frozen.set_active_output(None, Some(12));
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();
        assert!(!screen_source_is(
            &token,
            &source,
            &zoom,
            &frozen,
            (100, 50)
        ));
        frozen.set_active_output(None, Some(11));

        frozen.set_active_geometry(OutputGeometry::update_from(
            Some((0, 0)),
            Some((100, 50)),
            (100, 50),
            2,
            wl_output::Transform::Flipped,
            Some((100, 50)),
        ));
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();
        assert!(!screen_source_is(
            &token,
            &source,
            &zoom,
            &frozen,
            (100, 50)
        ));

        zoom.active = true;
        zoom.set_active_output(None, Some(12));
        zoom.set_active_geometry(OutputGeometry::update_from(
            Some((0, 0)),
            Some((100, 50)),
            (100, 50),
            1,
            wl_output::Transform::Normal,
            Some((100, 50)),
        ));
        zoom.set_image_with_provenance_for_test(
            opaque(100, 50),
            provenance(12, 1, 1, wl_output::Transform::Normal),
        );
        let zoom_token = {
            let source = displayed_screen_image(&zoom, &frozen, true).unwrap();
            screen_source_token(&source, &zoom, &frozen, (100, 50))
        };
        zoom.set_active_output(None, Some(99));
        zoom.set_active_geometry(OutputGeometry::update_from(
            Some((0, 0)),
            Some((100, 50)),
            (100, 50),
            2,
            wl_output::Transform::Flipped,
            Some((100, 50)),
        ));
        {
            let source = displayed_screen_image(&zoom, &frozen, true).unwrap();
            assert!(!screen_source_is(
                &zoom_token,
                &source,
                &zoom,
                &frozen,
                (100, 50)
            ));
        }
        zoom.view_offset = (1.0, 0.0);
        {
            let source = displayed_screen_image(&zoom, &frozen, true).unwrap();
            assert!(!screen_source_is(
                &zoom_token,
                &source,
                &zoom,
                &frozen,
                (100, 50)
            ));
        }

        zoom.view_offset = (0.0, 0.0);
        zoom.set_image_with_provenance_for_test(
            opaque(100, 50),
            provenance(12, 1, 1, wl_output::Transform::Normal),
        );
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();
        assert!(!screen_source_is(
            &zoom_token,
            &source,
            &zoom,
            &frozen,
            (100, 50)
        ));
    }

    #[test]
    fn displayed_source_refuses_images_without_complete_capture_identity() {
        let zoom = zoom_state();
        let mut frozen = FrozenState::new(None);
        frozen.set_image(opaque(2, 2));

        assert!(displayed_screen_image(&zoom, &frozen, true).is_none());
    }

    #[test]
    fn unzoomed_tokens_canonicalize_dormant_zoom_state() {
        let mut frozen = FrozenState::new(None);
        install_frozen(&mut frozen, opaque(100, 50));
        let mut zoom = zoom_state();
        zoom.scale = 7.0;
        zoom.view_offset = (123.0, 456.0);
        let source = displayed_screen_image(&zoom, &frozen, true).unwrap();

        let token = screen_source_token(&source, &zoom, &frozen, (100, 50));

        assert!(!token.zoom_transformed);
        assert_eq!(token.zoom_scale, 1.0);
        assert_eq!(token.zoom_view_offset, (0.0, 0.0));
    }

    #[test]
    fn reversed_drags_normalize_and_partly_offscreen_drags_clip() {
        let rect = image_rect_from_points((30.0, 40.0), (10.0, 20.0), 100, 100).unwrap();
        assert_eq!((rect.x(), rect.y(), rect.size()), (10, 20, (20, 20)));

        let clipped = image_rect_from_points((-50.0, -50.0), (10.5, 10.5), 100, 100).unwrap();
        assert_eq!((clipped.x(), clipped.y(), clipped.size()), (0, 0, (11, 11)));
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

        let crop =
            copy_image_rect(&source, ImagePixelRect::new(1, 0, 1, 2, (2, 2)).unwrap()).unwrap();

        assert_eq!((crop.width, crop.height, crop.stride), (1, 2, 4));
        assert_eq!(crop.data, vec![2, 2, 2, 255, 4, 4, 4, 255]);
    }

    #[test]
    fn crop_preserves_premultiplied_and_transparent_pixels_verbatim() {
        let source = image(2, 1, 8, vec![25, 50, 100, 128, 0, 0, 0, 0]);

        let crop =
            copy_image_rect(&source, ImagePixelRect::new(0, 0, 2, 1, (2, 1)).unwrap()).unwrap();

        assert_eq!(crop.data, vec![25, 50, 100, 128, 0, 0, 0, 0]);
    }

    #[test]
    fn crop_rejects_rectangles_that_do_not_fit_the_actual_source() {
        let source = opaque(4, 4);
        let error = |rect| copy_image_rect(&source, rect).err();

        assert_eq!(
            error(ImagePixelRect::new(3, 0, 2, 1, (5, 4)).unwrap()),
            Some(CropError::OutOfBounds)
        );
        assert_eq!(
            copy_image_rect(
                &image(0, 0, 0, Vec::new()),
                ImagePixelRect::new(0, 0, 1, 1, (1, 1)).unwrap(),
            ),
            Err(CropError::Empty)
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

        let crop =
            copy_image_rect(&rotated, ImagePixelRect::new(0, 0, 2, 1, (2, 3)).unwrap()).unwrap();

        assert_eq!(
            crop.data.chunks_exact(4).map(|p| p[0]).collect::<Vec<_>>(),
            vec![4, 1]
        );
    }
}
