#[cfg(feature = "portal")]
use std::time::Duration;

#[cfg(feature = "portal")]
const PORTAL_STARTUP_PROBE_TIMEOUT: Duration = Duration::from_millis(500);

#[cfg(feature = "portal")]
pub(crate) fn screenshot_portal_available(runtime: &tokio::runtime::Runtime) -> bool {
    runtime.block_on(async {
        tokio::time::timeout(
            PORTAL_STARTUP_PROBE_TIMEOUT,
            crate::capture::portal::is_portal_available(),
        )
        .await
        .unwrap_or(false)
    })
}

#[cfg(not(feature = "portal"))]
pub(crate) fn screenshot_portal_available(_runtime: &tokio::runtime::Runtime) -> bool {
    false
}

#[cfg(feature = "portal")]
pub(crate) async fn capture_via_portal_fullscreen_bytes()
-> Result<Vec<u8>, crate::capture::types::CaptureError> {
    use crate::capture::sources::portal::capture_via_portal_bytes;
    use crate::capture::types::CaptureType;

    capture_via_portal_bytes(CaptureType::FullScreen).await
}

#[cfg(not(feature = "portal"))]
pub(crate) async fn capture_via_portal_fullscreen_bytes()
-> Result<Vec<u8>, crate::capture::types::CaptureError> {
    Err(crate::capture::types::CaptureError::PortalUnavailable)
}

pub(crate) const fn portal_output_matches(target: Option<u32>, current: Option<u32>) -> bool {
    match (target, current) {
        (Some(target_output), Some(current_output)) => target_output == current_output,
        (None, None) => true,
        (None, Some(_)) | (Some(_), None) => false,
    }
}

pub(crate) const fn layout_token_matches(
    captured_output: Option<u32>,
    captured_generation: u64,
    active_output: Option<u32>,
    active_generation: u64,
) -> bool {
    portal_output_matches(captured_output, active_output)
        && captured_generation == active_generation
}

pub(crate) fn crop_argb(
    data: &[u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    crop_w: u32,
    crop_h: u32,
) -> Option<(u32, u32, Vec<u8>)> {
    if x >= width || y >= height {
        return None;
    }
    let max_w = width.saturating_sub(x);
    let max_h = height.saturating_sub(y);
    let cw = crop_w.min(max_w);
    let ch = crop_h.min(max_h);
    if cw == 0 || ch == 0 {
        return None;
    }

    let pixel_count = cw.checked_mul(ch)?;
    let byte_count = pixel_count.checked_mul(4)?;
    let mut out = vec![0u8; usize::try_from(byte_count).ok()?];
    let src_stride = usize::try_from(width.checked_mul(4)?).ok()?;
    let dst_stride = usize::try_from(cw.checked_mul(4)?).ok()?;
    for row in 0..usize::try_from(ch).ok()? {
        let src_offset = (usize::try_from(y).ok()? + row)
            .checked_mul(src_stride)?
            .checked_add(usize::try_from(x).ok()?.checked_mul(4)?)?;
        let dst_offset = row.checked_mul(dst_stride)?;
        let end = src_offset.checked_add(dst_stride)?;
        if end > data.len() || dst_offset.checked_add(dst_stride)? > out.len() {
            return None;
        }
        out[dst_offset..dst_offset + dst_stride]
            .copy_from_slice(&data[src_offset..src_offset + dst_stride]);
    }
    Some((cw, ch, out))
}

#[cfg(test)]
mod tests {
    use super::{crop_argb, layout_token_matches, portal_output_matches};

    #[test]
    fn crop_argb_respects_bounds() {
        // 2x2 image with distinct pixels: row-major BGRA.
        let data = vec![
            1, 2, 3, 4, 5, 6, 7, 8, //
            9, 10, 11, 12, 13, 14, 15, 16,
        ];
        let (width, height, cropped) = crop_argb(&data, 2, 2, 1, 0, 1, 2).expect("crop");
        assert_eq!((width, height), (1, 2));
        assert_eq!(cropped, vec![5, 6, 7, 8, 13, 14, 15, 16]);
    }

    #[test]
    fn crop_argb_returns_none_when_out_of_bounds() {
        // x beyond width.
        assert!(crop_argb(&[0u8; 4], 1, 1, 2, 0, 1, 1).is_none());
        // y beyond height.
        assert!(crop_argb(&[0u8; 4], 1, 1, 0, 2, 1, 1).is_none());
    }

    #[test]
    fn crop_argb_rejects_dimensions_that_would_overflow() {
        assert!(crop_argb(&[0u8; 4], u32::MAX, 1, 0, 0, u32::MAX, 1).is_none());
    }

    #[test]
    fn portal_output_matches_requires_both_sides_or_neither() {
        assert!(portal_output_matches(Some(1), Some(1)));
        assert!(!portal_output_matches(Some(1), Some(2)));
        assert!(portal_output_matches(None, None));
        assert!(!portal_output_matches(None, Some(1)));
        assert!(!portal_output_matches(Some(1), None));
    }

    #[test]
    fn layout_token_matches_requires_output_and_generation() {
        assert!(layout_token_matches(Some(7), 3, Some(7), 3));
        assert!(!layout_token_matches(Some(7), 3, Some(8), 3));
        assert!(!layout_token_matches(Some(7), 3, Some(7), 4));
        assert!(!layout_token_matches(None, 3, Some(7), 3));
        assert!(layout_token_matches(None, 3, None, 3));
    }
}
