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
    let geometry = DesktopBackdropGeometry {
        logical_x: 0,
        logical_y: 0,
        logical_width: 2,
        logical_height: 1,
        physical_width: Some(4),
        physical_height: Some(2),
        crop_x: Some(0),
        crop_y: Some(0),
        screenshot_width: Some(4),
        screenshot_height: Some(2),
    };
    let result = desktop_backdrop_from_argb(data.clone(), 4, 2, &request(2, 1, 2, Some(geometry)))
        .expect("exact backdrop");

    assert_eq!(result.width, 4);
    assert_eq!(result.height, 2);
    assert_eq!(result.stride, 16);
    assert_eq!(result.logical_to_image_scale_x, 2.0);
    assert_eq!(result.logical_to_image_scale_y, 2.0);
    assert_eq!(result.data.as_ref(), data.as_slice());
}

#[test]
fn desktop_backdrop_rejects_missing_geometry() {
    let error = desktop_backdrop_from_argb(vec![7u8; 4 * 2 * 4], 4, 2, &request(2, 1, 2, None))
        .expect_err("backdrop capture must not guess logical * scale");

    assert!(
        error
            .to_string()
            .contains("requires verified active output geometry"),
        "unexpected error: {error}"
    );
}

#[test]
fn desktop_backdrop_rejects_unverified_physical_size() {
    let geometry = DesktopBackdropGeometry {
        logical_x: 2,
        logical_y: 0,
        logical_width: 2,
        logical_height: 1,
        physical_width: None,
        physical_height: None,
        crop_x: Some(2),
        crop_y: Some(0),
        screenshot_width: None,
        screenshot_height: None,
    };

    let error = desktop_backdrop_from_argb(vec![0; 6 * 4], 6, 1, &request(2, 1, 1, Some(geometry)))
        .expect_err("missing mode pixels must fail closed");

    assert!(
        error.to_string().contains("pixel size is unavailable"),
        "unexpected error: {error}"
    );
}

#[test]
fn desktop_backdrop_accepts_fractional_scale_output_mode_size() {
    let data = vec![7u8; 5 * 3 * 4];
    let geometry = DesktopBackdropGeometry {
        logical_x: 0,
        logical_y: 0,
        logical_width: 3,
        logical_height: 2,
        physical_width: Some(5),
        physical_height: Some(3),
        crop_x: Some(0),
        crop_y: Some(0),
        screenshot_width: None,
        screenshot_height: None,
    };

    let result =
        desktop_backdrop_from_argb(data, 5, 3, &request(3, 2, 2, Some(geometry))).expect("mode");

    assert_eq!(result.width, 5);
    assert_eq!(result.height, 3);
    assert_eq!(result.logical_to_image_scale_x, 5.0 / 3.0);
    assert_eq!(result.logical_to_image_scale_y, 1.5);
}

#[test]
fn desktop_backdrop_rejects_a_surface_that_is_not_the_active_output() {
    let geometry = DesktopBackdropGeometry {
        logical_x: 0,
        logical_y: 0,
        logical_width: 4,
        logical_height: 2,
        physical_width: Some(4),
        physical_height: Some(2),
        crop_x: Some(0),
        crop_y: Some(0),
        screenshot_width: Some(4),
        screenshot_height: Some(2),
    };

    let error =
        desktop_backdrop_from_argb(vec![0; 4 * 2 * 4], 4, 2, &request(3, 2, 1, Some(geometry)))
            .expect_err("a partial-output surface cannot map the full output backdrop exactly");

    assert!(error.to_string().contains("does not match active output"));
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
        physical_width: Some(2),
        physical_height: Some(1),
        crop_x: Some(2),
        crop_y: Some(0),
        screenshot_width: None,
        screenshot_height: None,
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
        physical_width: Some(2),
        physical_height: Some(2),
        crop_x: Some(2),
        crop_y: Some(0),
        screenshot_width: None,
        screenshot_height: None,
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
        DesktopBackdropGeometry::from_outputs(outputs[1], &outputs).expect("mixed scale origin");
    let mut data = Vec::new();
    for pixel in 0u8..10 {
        data.extend_from_slice(&[pixel, pixel, pixel, 255]);
    }

    let result =
        desktop_backdrop_from_argb(data, 10, 1, &request(4, 1, 1, Some(geometry))).expect("crop");

    assert_eq!(geometry.physical_origin(), Some((6, 0)));
    assert_eq!(geometry.screenshot_size(), Some((10, 1)));
    assert_eq!(
        result.data.as_ref(),
        &[6, 6, 6, 255, 7, 7, 7, 255, 8, 8, 8, 255, 9, 9, 9, 255]
    );
}

#[test]
fn desktop_backdrop_rejects_layout_bounds_that_exceed_supported_coordinates() {
    let output = output(i32::MAX - 1, 0, 4, 1, 4, 1);

    assert!(DesktopBackdropGeometry::from_outputs(output, &[output]).is_none());
}

#[test]
fn desktop_backdrop_rejects_a_screenshot_whose_layout_size_does_not_match() {
    let outputs = [output(-4, 0, 4, 1, 6, 1), output(0, 0, 4, 1, 4, 1)];
    let geometry =
        DesktopBackdropGeometry::from_outputs(outputs[1], &outputs).expect("desktop geometry");
    let data = vec![0u8; 11 * 4];

    let err = desktop_backdrop_from_argb(data, 11, 1, &request(4, 1, 1, Some(geometry)))
        .expect_err("a wider image cannot use the crop origin from a different layout");

    assert!(
        err.to_string().contains("does not match output layout"),
        "unexpected error: {err}"
    );
}

#[test]
fn desktop_backdrop_single_output_at_nonzero_logical_origin_crops_at_zero() {
    let outputs = [output(10, 20, 4, 2, 8, 4)];
    let geometry =
        DesktopBackdropGeometry::from_outputs(outputs[0], &outputs).expect("single output");

    assert_eq!(geometry.physical_origin(), Some((0, 0)));
}

#[test]
fn desktop_backdrop_origin_shifts_when_a_left_output_is_removed() {
    let left = output(-4, 0, 4, 1, 6, 1);
    let right = output(0, 0, 4, 1, 4, 1);
    let with_left =
        DesktopBackdropGeometry::from_outputs(right, &[left, right]).expect("with left");
    let without_left = DesktopBackdropGeometry::from_outputs(right, &[right]).expect("removed");

    assert_eq!(with_left.physical_origin(), Some((6, 0)));
    assert_eq!(without_left.physical_origin(), Some((0, 0)));
}

#[test]
fn desktop_backdrop_normalizes_negative_output_origins() {
    let outputs = [output(-2, 0, 2, 2, 2, 2), output(0, 0, 3, 2, 3, 2)];
    let geometry =
        DesktopBackdropGeometry::from_outputs(outputs[0], &outputs).expect("negative origin");

    assert_eq!(geometry.physical_origin(), Some((0, 0)));
}

#[test]
fn desktop_backdrop_crops_rotated_output_using_transformed_size() {
    let outputs = [output(0, 0, 2, 4, 4, 2), output(2, 0, 2, 4, 2, 4)];
    let geometry =
        DesktopBackdropGeometry::from_outputs(outputs[0], &outputs).expect("rotated output");
    let mut data = Vec::new();
    for pixel in 0u8..24 {
        data.extend_from_slice(&[pixel, pixel, pixel, 255]);
    }

    let result =
        desktop_backdrop_from_argb(data, 6, 4, &request(2, 4, 1, Some(geometry))).expect("crop");

    assert_eq!(geometry.verified_physical_size(), Some((4, 2)));
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
