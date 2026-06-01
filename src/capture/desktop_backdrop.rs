use std::sync::Arc;

use tokio::task;

use crate::capture::{
    dependencies::CaptureDependencies,
    types::{
        CaptureError, CaptureType, DesktopBackdropCaptureRequest, DesktopBackdropCaptureResult,
    },
};
use crate::image_decode::{decode_rgba, format_from_mime_or_bytes};

pub(crate) async fn capture_desktop_backdrop(
    request: DesktopBackdropCaptureRequest,
    dependencies: Arc<CaptureDependencies>,
) -> Result<DesktopBackdropCaptureResult, CaptureError> {
    log::info!(
        "Starting desktop backdrop capture for {:?}",
        request.operation
    );
    let image_data = dependencies.source.capture(CaptureType::FullScreen).await?;
    log::info!(
        "Obtained desktop backdrop screenshot data ({} bytes)",
        image_data.len()
    );

    task::spawn_blocking(move || decode_desktop_backdrop(image_data, request))
        .await
        .map_err(|err| CaptureError::ImageError(format!("Backdrop decode task failed: {err}")))?
}

fn decode_desktop_backdrop(
    image_data: Vec<u8>,
    request: DesktopBackdropCaptureRequest,
) -> Result<DesktopBackdropCaptureResult, CaptureError> {
    let format = format_from_mime_or_bytes("", &image_data).ok_or_else(|| {
        CaptureError::ImageError("Desktop backdrop capture returned an unsupported image".into())
    })?;
    let decoded = decode_rgba(format, &image_data).map_err(|err| {
        CaptureError::ImageError(format!("Failed to decode desktop backdrop: {err}"))
    })?;
    let argb = rgba_to_cairo_argb(&decoded.rgba)?;
    desktop_backdrop_from_argb(argb, decoded.width, decoded.height, &request)
}

fn rgba_to_cairo_argb(rgba: &[u8]) -> Result<Vec<u8>, CaptureError> {
    if !rgba.len().is_multiple_of(4) {
        return Err(CaptureError::ImageError(
            "Decoded desktop backdrop RGBA data has an unexpected length".to_string(),
        ));
    }

    let mut argb = Vec::with_capacity(rgba.len());
    for pixel in rgba.chunks_exact(4) {
        let r = pixel[0];
        let g = pixel[1];
        let b = pixel[2];
        let a = pixel[3];
        let alpha = u16::from(a);
        let pr = ((u16::from(r) * alpha + 127) / 255) as u8;
        let pg = ((u16::from(g) * alpha + 127) / 255) as u8;
        let pb = ((u16::from(b) * alpha + 127) / 255) as u8;
        argb.extend_from_slice(&[pb, pg, pr, a]);
    }
    Ok(argb)
}

pub(crate) fn desktop_backdrop_from_argb(
    data: Vec<u8>,
    width: u32,
    height: u32,
    request: &DesktopBackdropCaptureRequest,
) -> Result<DesktopBackdropCaptureResult, CaptureError> {
    validate_argb_buffer(&data, width, height)?;
    let (logical_width, logical_height, expected_width, expected_height) =
        expected_backdrop_dimensions(request)?;

    if width == expected_width && height == expected_height {
        return desktop_backdrop_result(data, width, height, logical_width, logical_height);
    }

    let Some(geometry) = request.geometry else {
        return Err(CaptureError::ImageError(format!(
            "Desktop backdrop capture returned {width}x{height}, but active output is {expected_width}x{expected_height} and no output geometry is available"
        )));
    };
    let (origin_x, origin_y) = geometry.physical_origin().ok_or_else(|| {
        CaptureError::ImageError(
            "Active output crop origin is unavailable for desktop capture".to_string(),
        )
    })?;

    let cropped = crop_argb_strict(
        &data,
        width,
        height,
        origin_x,
        origin_y,
        expected_width,
        expected_height,
    )
    .ok_or_else(|| {
        CaptureError::ImageError(format!(
            "Desktop backdrop capture {width}x{height} does not contain active output crop {origin_x},{origin_y} {expected_width}x{expected_height}"
        ))
    })?;
    desktop_backdrop_result(
        cropped,
        expected_width,
        expected_height,
        logical_width,
        logical_height,
    )
}

fn expected_backdrop_dimensions(
    request: &DesktopBackdropCaptureRequest,
) -> Result<(u32, u32, u32, u32), CaptureError> {
    let scale = u32::try_from(request.scale).map_err(|_| {
        CaptureError::ImageError(format!("Invalid desktop backdrop scale: {}", request.scale))
    })?;
    if scale == 0 || request.logical_width == 0 || request.logical_height == 0 {
        return Err(CaptureError::ImageError(
            "Desktop backdrop capture requires a configured non-empty active output".to_string(),
        ));
    }

    if let Some(geometry) = request.geometry {
        let (width, height) = geometry.physical_size().ok_or_else(|| {
            CaptureError::ImageError("Active output dimensions are too large".to_string())
        })?;
        if geometry.logical_width == 0 || geometry.logical_height == 0 || width == 0 || height == 0
        {
            return Err(CaptureError::ImageError(
                "Desktop backdrop capture requires a non-empty active output geometry".to_string(),
            ));
        }
        Ok((
            geometry.logical_width,
            geometry.logical_height,
            width,
            height,
        ))
    } else {
        let width = request.logical_width.checked_mul(scale).ok_or_else(|| {
            CaptureError::ImageError("Active output width is too large".to_string())
        })?;
        let height = request.logical_height.checked_mul(scale).ok_or_else(|| {
            CaptureError::ImageError("Active output height is too large".to_string())
        })?;
        Ok((request.logical_width, request.logical_height, width, height))
    }
}

fn validate_argb_buffer(data: &[u8], width: u32, height: u32) -> Result<(), CaptureError> {
    let required_len = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| CaptureError::ImageError("Desktop backdrop image is too large".into()))?;
    if data.len() != required_len {
        return Err(CaptureError::ImageError(format!(
            "Desktop backdrop buffer has {} bytes, expected {required_len}",
            data.len()
        )));
    }
    Ok(())
}

fn crop_argb_strict(
    data: &[u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    crop_width: u32,
    crop_height: u32,
) -> Option<Vec<u8>> {
    let crop_right = x.checked_add(crop_width)?;
    let crop_bottom = y.checked_add(crop_height)?;
    if x >= width || y >= height || crop_right > width || crop_bottom > height {
        return None;
    }

    let src_stride = usize::try_from(width.checked_mul(4)?).ok()?;
    let dst_stride = usize::try_from(crop_width.checked_mul(4)?).ok()?;
    let output_len = dst_stride.checked_mul(usize::try_from(crop_height).ok()?)?;
    let mut output = vec![0u8; output_len];

    for row in 0..usize::try_from(crop_height).ok()? {
        let src_offset = (usize::try_from(y).ok()? + row)
            .checked_mul(src_stride)?
            .checked_add(usize::try_from(x).ok()?.checked_mul(4)?)?;
        let dst_offset = row.checked_mul(dst_stride)?;
        let src_end = src_offset.checked_add(dst_stride)?;
        let dst_end = dst_offset.checked_add(dst_stride)?;
        if src_end > data.len() || dst_end > output.len() {
            return None;
        }
        output[dst_offset..dst_end].copy_from_slice(&data[src_offset..src_end]);
    }
    Some(output)
}

fn desktop_backdrop_result(
    data: Vec<u8>,
    width: u32,
    height: u32,
    logical_width: u32,
    logical_height: u32,
) -> Result<DesktopBackdropCaptureResult, CaptureError> {
    let width_i32 = i32::try_from(width)
        .map_err(|_| CaptureError::ImageError("Desktop backdrop width is too large".into()))?;
    let height_i32 = i32::try_from(height)
        .map_err(|_| CaptureError::ImageError("Desktop backdrop height is too large".into()))?;
    let stride = width_i32
        .checked_mul(4)
        .ok_or_else(|| CaptureError::ImageError("Desktop backdrop stride overflow".into()))?;

    Ok(DesktopBackdropCaptureResult {
        data: Arc::from(data),
        width: width_i32,
        height: height_i32,
        stride,
        logical_to_image_scale_x: width as f64 / logical_width as f64,
        logical_to_image_scale_y: height as f64 / logical_height as f64,
    })
}
