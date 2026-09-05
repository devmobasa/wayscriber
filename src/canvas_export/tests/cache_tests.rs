use std::sync::Arc;

use super::page::{
    CanvasExportBackdropSnapshot, CanvasPageExportSnapshot, draw_canvas_page_with_measurer,
};
use crate::draw::{BlurStyle, EmbeddedImage, Frame, RenderCaches, RenderCtx, Shape};

fn page(frame: Frame, backdrop: CanvasExportBackdropSnapshot) -> CanvasPageExportSnapshot {
    CanvasPageExportSnapshot {
        frame,
        backdrop,
        viewport_width: 20,
        viewport_height: 20,
        origin_x: 0,
        origin_y: 0,
        text_halo_enabled: true,
        spotlight: Default::default(),
    }
}

fn pixels(page: &CanvasPageExportSnapshot, caches: &mut RenderCaches) -> Vec<u8> {
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 20, 20).unwrap();
    {
        let cairo = cairo::Context::new(&surface).unwrap();
        draw_canvas_page_with_measurer(
            &crate::draw::TextMeasurer::default(),
            &mut RenderCtx::new(&cairo, caches),
            page,
            1.0,
        )
        .unwrap();
    }
    surface.flush();
    surface.data().unwrap().to_vec()
}

#[test]
fn page_owner_reuses_embedded_images_and_releases_them_after_the_job() {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 2, 2).unwrap();
    let cairo = cairo::Context::new(&surface).unwrap();
    cairo.set_source_rgb(1.0, 0.0, 0.0);
    cairo.paint().unwrap();
    let mut png = Vec::new();
    surface.write_to_png(&mut png).unwrap();
    let bytes: Arc<[u8]> = png.into();
    let mut frame = Frame::new();
    frame.add_shape(Shape::Image {
        x: 2,
        y: 2,
        w: 8,
        h: 8,
        data: EmbeddedImage {
            mime_type: "image/png".into(),
            width: 2,
            height: 2,
            bytes: Arc::clone(&bytes),
        },
    });
    let page = page(frame, CanvasExportBackdropSnapshot::Transparent);
    let baseline = Arc::strong_count(&bytes);
    let mut caches = RenderCaches::default();
    let first = pixels(&page, &mut caches);
    let retained = Arc::strong_count(&bytes);
    assert!(
        retained > baseline,
        "page rendering must retain decoded image resources in its owner"
    );
    assert!(
        first.iter().any(|value| *value != 0),
        "image must be painted"
    );
    assert_eq!(pixels(&page, &mut caches), first);
    assert_eq!(
        Arc::strong_count(&bytes),
        retained,
        "another page must reuse the same image entry"
    );
    drop(caches);
    assert_eq!(Arc::strong_count(&bytes), baseline);
}

#[test]
fn shared_export_owner_never_aliases_page_backdrops_with_identical_blur_geometry() {
    let mut frame = Frame::new();
    frame.add_shape(Shape::BlurRect {
        x: 2,
        y: 2,
        w: 16,
        h: 16,
        strength: 8.0,
        style: BlurStyle::Gaussian,
    });
    let make_page = |color: u32| {
        page(
            frame.clone_without_history(),
            CanvasExportBackdropSnapshot::PersistedImage {
                data: Arc::from(color.to_ne_bytes().repeat(20 * 20)),
                width: 20,
                height: 20,
                stride: 80,
                logical_to_image_scale_x: 1.0,
                logical_to_image_scale_y: 1.0,
            },
        )
    };
    let red = make_page(0xffff0000);
    let blue = make_page(0xff0000ff);
    let mut caches = RenderCaches::default();
    let red_pixels = pixels(&red, &mut caches);
    let blue_pixels = pixels(&blue, &mut caches);
    assert_ne!(red_pixels, blue_pixels);
    assert_eq!(blue_pixels, pixels(&blue, &mut RenderCaches::default()));
    assert_eq!(red_pixels, pixels(&red, &mut caches));
}

#[test]
fn export_snapshots_remain_sendable_to_workers() {
    fn assert_send<T: Send>() {}
    assert_send::<super::CanvasExportSnapshot>();
    assert_send::<super::BoardPdfExportSnapshot>();
    assert_send::<super::CanvasRegionExportSnapshot>();
}

#[test]
fn pdf_pages_keep_distinct_blurred_backdrops_when_rasterized() {
    use super::{
        BoardPdfExportSnapshot, PdfPageExportSnapshot, PdfPageMetadata, render_board_pdf,
        resolve_pdf_page_layout,
    };
    use std::process::Command;
    if Command::new("pdftoppm").arg("-v").output().is_err() {
        return;
    }
    let mut frame = Frame::new();
    frame.add_shape(Shape::BlurRect {
        x: 2,
        y: 2,
        w: 16,
        h: 16,
        strength: 8.0,
        style: BlurStyle::Gaussian,
    });
    // Each page holds the same encoded allocation, exercising PDF image reuse.
    let image = cairo::ImageSurface::create(cairo::Format::ARgb32, 2, 2).unwrap();
    let image_ctx = cairo::Context::new(&image).unwrap();
    image_ctx.set_source_rgb(0.0, 1.0, 0.0);
    image_ctx.paint().unwrap();
    let mut encoded = Vec::new();
    image.write_to_png(&mut encoded).unwrap();
    frame.add_shape(Shape::Image {
        x: 0,
        y: 0,
        w: 2,
        h: 2,
        data: EmbeddedImage {
            mime_type: "image/png".into(),
            width: 2,
            height: 2,
            bytes: encoded.into(),
        },
    });
    let layout = resolve_pdf_page_layout(20, 20, 0, 0, None, &Default::default()).unwrap();
    let pages = [0xffff0000u32, 0xff0000ffu32]
        .into_iter()
        .enumerate()
        .map(|(index, color)| PdfPageExportSnapshot {
            page: page(
                frame.clone_without_history(),
                CanvasExportBackdropSnapshot::PersistedImage {
                    data: Arc::from(color.to_ne_bytes().repeat(20 * 20)),
                    width: 20,
                    height: 20,
                    stride: 80,
                    logical_to_image_scale_x: 1.0,
                    logical_to_image_scale_y: 1.0,
                },
            ),
            layout,
            metadata: PdfPageMetadata::new(
                0,
                1,
                index,
                2,
                index,
                2,
                index,
                2,
                "Board".into(),
                None,
            ),
        })
        .collect();
    let pdf = render_board_pdf(&BoardPdfExportSnapshot {
        pages,
        labels: Default::default(),
    })
    .unwrap();
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("pages.pdf");
    std::fs::write(&path, pdf).unwrap();
    let prefix = temp.path().join("page");
    let result = Command::new("pdftoppm")
        .args(["-png", "-r", "72"])
        .arg(&path)
        .arg(&prefix)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    for (index, expected) in [(1, 0xffff0000u32), (2, 0xff0000ffu32)] {
        let mut file = std::fs::File::open(temp.path().join(format!("page-{index}.png"))).unwrap();
        let mut image = cairo::ImageSurface::create_from_png(&mut file).unwrap();
        let offset = 10 * image.stride() as usize + 10 * 4;
        let data = image.data().unwrap();
        let actual = u32::from_ne_bytes(data[offset..offset + 4].try_into().unwrap());
        assert_eq!(
            u32::from_ne_bytes(data[0..4].try_into().unwrap()),
            0xff00ff00,
            "both pages must retain the repeated image"
        );
        assert_eq!(
            actual, expected,
            "page {index} blur must use its own backdrop"
        );
    }
}

#[test]
fn png_and_region_entries_preserve_embedded_image_and_text_pixels() {
    use super::{
        BoardExportSnapshot, CanvasExportRect, CanvasExportSnapshot, CanvasExportViewport,
        CanvasRegionExportSnapshot, CanvasRegionSource, render_canvas_png,
        render_canvas_region_pixels,
    };
    use crate::screen_pixels::{ImagePixelRect, ScreenImage};
    let image = cairo::ImageSurface::create(cairo::Format::ARgb32, 2, 2).unwrap();
    let ctx = cairo::Context::new(&image).unwrap();
    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.paint().unwrap();
    let mut encoded = Vec::new();
    image.write_to_png(&mut encoded).unwrap();
    let mut frame = Frame::new();
    frame.add_shape(Shape::Image {
        x: 14,
        y: 24,
        w: 20,
        h: 20,
        data: EmbeddedImage {
            mime_type: "image/png".into(),
            width: 2,
            height: 2,
            bytes: encoded.into(),
        },
    });
    frame.add_shape(Shape::Text {
        x: 42,
        y: 54,
        text: "Cache".into(),
        color: crate::draw::BLACK,
        size: 24.0,
        font_descriptor: Default::default(),
        background_enabled: false,
        wrap_width: None,
    });
    let export = CanvasExportSnapshot {
        viewport: CanvasExportViewport {
            logical_width: 160,
            logical_height: 64,
            scale: 1,
            origin_x: 10,
            origin_y: 20,
        },
        backdrop: CanvasExportBackdropSnapshot::Solid(crate::draw::WHITE),
        board: BoardExportSnapshot {
            frame: frame.clone_without_history(),
        },
        render_profile: None,
        text_halo_enabled: false,
        spotlight: Default::default(),
    };
    let png = render_canvas_png(&export).unwrap();
    assert_eq!((png.width, png.height), (160, 64));
    let mut decoded = cairo::ImageSurface::create_from_png(&mut png.bytes.as_slice()).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (160, 64));
    let stride = decoded.stride() as usize;
    let png_pixels = decoded.data().unwrap().to_vec();
    let pixel = |x: usize, y: usize| {
        u32::from_ne_bytes(
            png_pixels[y * stride + x * 4..y * stride + x * 4 + 4]
                .try_into()
                .unwrap(),
        )
    };
    assert_eq!(
        pixel(12, 12),
        0xffff0000,
        "PNG image location follows viewport origin"
    );
    assert_eq!(pixel(150, 50), 0xffffffff, "background remains white");
    let dark_text_pixels = (8..40)
        .flat_map(|y| (30..130).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(x, y) & 0x00ffffff < 0x00404040)
        .count();
    assert!(
        dark_text_pixels > 50,
        "PNG must contain visible text beside the image"
    );
    let region = render_canvas_region_pixels(CanvasRegionExportSnapshot {
        source: CanvasRegionSource {
            image: Arc::new(ScreenImage {
                data: 0xffffffffu32.to_ne_bytes().repeat(160 * 64),
                width: 160,
                height: 64,
                stride: 160 * 4,
            }),
            logical_bounds: CanvasExportRect::new(10.0, 20.0, 160.0, 64.0).unwrap(),
        },
        selection: ImagePixelRect::new(0, 0, 160, 64, (160, 64)).unwrap(),
        frame,
        text_halo_enabled: false,
        spotlight: Default::default(),
    })
    .unwrap();
    assert_eq!((region.width(), region.height()), (160, 64));
    for y in 0..64usize {
        assert_eq!(
            &png_pixels[y * stride..y * stride + 640],
            &region.data()[y * 640..(y + 1) * 640],
            "PNG and region row {y}"
        );
    }
}

#[test]
fn mixed_image_and_wrapped_text_pages_reuse_measurement_across_output_scales() {
    let image = cairo::ImageSurface::create(cairo::Format::ARgb32, 2, 2).unwrap();
    let ctx = cairo::Context::new(&image).unwrap();
    ctx.set_source_rgb(0.1, 0.4, 0.8);
    ctx.paint().unwrap();
    let mut png = Vec::new();
    image.write_to_png(&mut png).unwrap();
    let mut frame = Frame::new();
    frame.add_shape(Shape::Image {
        x: 5,
        y: 5,
        w: 190,
        h: 100,
        data: EmbeddedImage {
            mime_type: "image/png".into(),
            width: 2,
            height: 2,
            bytes: png.into(),
        },
    });
    frame.add_shape(Shape::Text {
        x: 20,
        y: 35,
        text: "Page 測試 العربية wrapped text".into(),
        color: crate::draw::WHITE,
        size: 16.0,
        font_descriptor: crate::draw::FontDescriptor::default(),
        background_enabled: false,
        wrap_width: Some(140),
    });
    let mut page = page(
        frame,
        CanvasExportBackdropSnapshot::Solid(crate::draw::WHITE),
    );
    page.viewport_width = 220;
    page.viewport_height = 130;
    let mut image_only = page.clone();
    image_only
        .frame
        .shapes
        .retain(|shape| matches!(shape.shape, Shape::Image { .. }));
    let paint = |page: &CanvasPageExportSnapshot,
                 measurer: &crate::draw::TextMeasurer,
                 caches: &mut RenderCaches,
                 scale: i32| {
        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, 220 * scale, 130 * scale).unwrap();
        {
            let ctx = cairo::Context::new(&surface).unwrap();
            draw_canvas_page_with_measurer(
                measurer,
                &mut RenderCtx::new(&ctx, caches),
                page,
                scale as f64,
            )
            .unwrap();
        }
        surface.data().unwrap().to_vec()
    };
    let measurer = crate::draw::TextMeasurer::default();
    let mut caches = RenderCaches::default();
    for scale in [1, 2, 1] {
        let actual = paint(&page, &measurer, &mut caches, scale);
        let fresh = paint(
            &page,
            &crate::draw::TextMeasurer::default(),
            &mut RenderCaches::default(),
            scale,
        );
        assert!(actual == fresh, "mixed export page at scale {scale}");
        let baseline = paint(&image_only, &measurer, &mut RenderCaches::default(), scale);
        assert!(
            actual != baseline,
            "text must paint over the image at scale {scale}"
        );
        assert!(
            actual
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[..3] != [255, 255, 255])
        );
    }
}
