use crate::capture::{
    DesktopBackdropCaptureRequest, DesktopBackdropGeometry, DesktopBackdropOutputGeometry,
    ImageOperationKind, desktop_backdrop::desktop_backdrop_from_argb,
};

fn request(
    logical_width: u32,
    logical_height: u32,
    scale: i32,
    geometry: Option<DesktopBackdropGeometry>,
) -> DesktopBackdropCaptureRequest {
    DesktopBackdropCaptureRequest {
        logical_width,
        logical_height,
        scale,
        geometry,
        operation: ImageOperationKind::BoardPdfExport,
    }
}

#[test]
fn desktop_backdrop_accepts_exact_active_output_size() {
    let data = vec![7u8; 4 * 2 * 4];
    let result = desktop_backdrop_from_argb(data.clone(), 4, 2, &request(2, 1, 2, None))
        .expect("exact backdrop");

    assert_eq!(result.width, 4);
    assert_eq!(result.height, 2);
    assert_eq!(result.stride, 16);
    assert_eq!(result.logical_to_image_scale_x, 2.0);
    assert_eq!(result.logical_to_image_scale_y, 2.0);
    assert_eq!(result.data.as_ref(), data.as_slice());
}

#[test]
fn desktop_backdrop_accepts_fractional_scale_output_mode_size() {
    let data = vec![7u8; 5 * 3 * 4];
    let geometry = DesktopBackdropGeometry {
        logical_x: 0,
        logical_y: 0,
        logical_width: 3,
        logical_height: 2,
        scale: 2,
        physical_width: Some(5),
        physical_height: Some(3),
        crop_x: Some(0),
        crop_y: Some(0),
    };

    let result =
        desktop_backdrop_from_argb(data, 5, 3, &request(3, 2, 2, Some(geometry))).expect("mode");

    assert_eq!(result.width, 5);
    assert_eq!(result.height, 3);
    assert_eq!(result.logical_to_image_scale_x, 5.0 / 3.0);
    assert_eq!(result.logical_to_image_scale_y, 1.5);
}

#[test]
fn desktop_backdrop_crops_multi_output_capture_to_active_output() {
    let mut data = Vec::new();
    for pixel in 0u8..6 {
        data.extend_from_slice(&[pixel, pixel, pixel, 255]);
    }
    let geometry = DesktopBackdropGeometry {
        logical_x: 2,
        logical_y: 0,
        logical_width: 2,
        logical_height: 1,
        scale: 1,
        physical_width: None,
        physical_height: None,
        crop_x: Some(2),
        crop_y: Some(0),
    };

    let result =
        desktop_backdrop_from_argb(data, 6, 1, &request(2, 1, 1, Some(geometry))).expect("crop");

    assert_eq!(result.width, 2);
    assert_eq!(result.height, 1);
    assert_eq!(result.data.as_ref(), &[2, 2, 2, 255, 3, 3, 3, 255]);
}

#[test]
fn desktop_backdrop_crop_failure_returns_error() {
    let data = vec![0u8; 3 * 2 * 4];
    let geometry = DesktopBackdropGeometry {
        logical_x: 2,
        logical_y: 0,
        logical_width: 2,
        logical_height: 2,
        scale: 1,
        physical_width: None,
        physical_height: None,
        crop_x: Some(2),
        crop_y: Some(0),
    };

    let err = desktop_backdrop_from_argb(data, 3, 2, &request(2, 2, 1, Some(geometry)))
        .expect_err("crop should fail");

    assert!(
        err.to_string().contains("does not contain active output"),
        "unexpected error: {err}"
    );
}

#[test]
fn desktop_backdrop_crops_mixed_scale_output_using_screenshot_origin() {
    let outputs = [output(-4, 0, 4, 1, 6, 1), output(0, 0, 4, 1, 4, 1)];
    let geometry =
        DesktopBackdropGeometry::from_outputs(outputs[1], &outputs, 1).expect("mixed scale origin");
    let mut data = Vec::new();
    for pixel in 0u8..10 {
        data.extend_from_slice(&[pixel, pixel, pixel, 255]);
    }

    let result =
        desktop_backdrop_from_argb(data, 10, 1, &request(4, 1, 1, Some(geometry))).expect("crop");

    assert_eq!(geometry.physical_origin(), Some((6, 0)));
    assert_eq!(
        result.data.as_ref(),
        &[6, 6, 6, 255, 7, 7, 7, 255, 8, 8, 8, 255, 9, 9, 9, 255]
    );
}

#[test]
fn desktop_backdrop_normalizes_negative_output_origins() {
    let outputs = [output(-2, 0, 2, 2, 2, 2), output(0, 0, 3, 2, 3, 2)];
    let geometry =
        DesktopBackdropGeometry::from_outputs(outputs[0], &outputs, 1).expect("negative origin");

    assert_eq!(geometry.physical_origin(), Some((0, 0)));
}

#[test]
fn desktop_backdrop_crops_rotated_output_using_transformed_size() {
    let outputs = [output(0, 0, 2, 4, 4, 2), output(2, 0, 2, 4, 2, 4)];
    let geometry =
        DesktopBackdropGeometry::from_outputs(outputs[0], &outputs, 1).expect("rotated output");
    let mut data = Vec::new();
    for pixel in 0u8..12 {
        data.extend_from_slice(&[pixel, pixel, pixel, 255]);
    }

    let result =
        desktop_backdrop_from_argb(data, 6, 2, &request(2, 4, 1, Some(geometry))).expect("crop");

    assert_eq!(geometry.physical_size(), Some((4, 2)));
    assert_eq!(geometry.physical_origin(), Some((0, 0)));
    assert_eq!(result.width, 4);
    assert_eq!(result.height, 2);
    assert_eq!(
        result.data.as_ref(),
        &[
            0, 0, 0, 255, 1, 1, 1, 255, 2, 2, 2, 255, 3, 3, 3, 255, 6, 6, 6, 255, 7, 7, 7, 255, 8,
            8, 8, 255, 9, 9, 9, 255,
        ]
    );
}

fn output(
    logical_x: i32,
    logical_y: i32,
    logical_width: u32,
    logical_height: u32,
    physical_width: u32,
    physical_height: u32,
) -> DesktopBackdropOutputGeometry {
    DesktopBackdropOutputGeometry {
        logical_x,
        logical_y,
        logical_width,
        logical_height,
        physical_width,
        physical_height,
    }
}
