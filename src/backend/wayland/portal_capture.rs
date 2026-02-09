#[cfg(feature = "portal")]
pub(crate) async fn capture_via_portal_fullscreen_bytes() -> Result<Vec<u8>, String> {
    use crate::capture::sources::portal::capture_via_portal_bytes;
    use crate::capture::types::CaptureType;

    capture_via_portal_bytes(CaptureType::FullScreen)
        .await
        .map_err(|error| format!("Portal capture failed: {error}"))
}

#[cfg(not(feature = "portal"))]
pub(crate) async fn capture_via_portal_fullscreen_bytes() -> Result<Vec<u8>, String> {
    Err("Portal capture is disabled (feature flag)".to_string())
}

pub(crate) const fn portal_output_matches(target: Option<u32>, current: Option<u32>) -> bool {
    match (target, current) {
        (Some(target_output), Some(current_output)) => target_output == current_output,
        (None, None) => true,
        (None, Some(_)) => true,
        (Some(_), None) => false,
    }
}

pub(crate) fn crop_argb(
    data: &[u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    crop_w: u32,
    crop_h: u32,
) -> Option<Vec<u8>> {
    if x >= width || y >= height {
        return None;
    }
    let max_w = width.saturating_sub(x);
    let max_h = height.saturating_sub(y);
    let cw = crop_w.min(max_w);
    let ch = crop_h.min(max_h);

    let mut out = vec![0u8; (cw * ch * 4) as usize];
    let src_stride = (width * 4) as usize;
    let dst_stride = (cw * 4) as usize;
    for row in 0..ch as usize {
        let src_offset = ((y as usize + row) * src_stride) + (x as usize * 4);
        let dst_offset = row * dst_stride;
        let end = src_offset + dst_stride;
        if end > data.len() || dst_offset + dst_stride > out.len() {
            return None;
        }
        out[dst_offset..dst_offset + dst_stride]
            .copy_from_slice(&data[src_offset..src_offset + dst_stride]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::crop_argb;

    #[test]
    fn crop_argb_respects_bounds() {
        // 2x2 image with distinct pixels: row-major BGRA.
        let data = vec![
            1, 2, 3, 4, 5, 6, 7, 8, //
            9, 10, 11, 12, 13, 14, 15, 16,
        ];
        let cropped = crop_argb(&data, 2, 2, 1, 0, 1, 2).expect("crop");
        assert_eq!(cropped, vec![5, 6, 7, 8, 13, 14, 15, 16]);
    }

    #[test]
    fn crop_argb_returns_none_when_out_of_bounds() {
        // x beyond width.
        assert!(crop_argb(&[0u8; 4], 1, 1, 2, 0, 1, 1).is_none());
        // y beyond height.
        assert!(crop_argb(&[0u8; 4], 1, 1, 0, 2, 1, 1).is_none());
    }
}
