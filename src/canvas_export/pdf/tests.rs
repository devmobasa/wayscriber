use super::*;
use crate::config::{PdfExportConfig, PdfFitMode, PdfOrientation, PdfPageSize};
use crate::draw::{FontDescriptor, Frame, RED, Shape, WHITE};
use std::process::Command;

#[test]
fn viewport_fit_preserves_legacy_page_and_destination_size() {
    let config = PdfExportConfig::default();
    let layout = resolve_pdf_page_layout(640, 480, 100, -50, None, &config).expect("layout");

    assert_eq!(layout.page_width, 640.0);
    assert_eq!(layout.page_height, 480.0);
    assert_eq!(
        layout.source_rect,
        CanvasExportRect {
            x: 100.0,
            y: -50.0,
            width: 640.0,
            height: 480.0
        }
    );
    assert_eq!(
        layout.destination_rect,
        CanvasExportRect {
            x: 0.0,
            y: 0.0,
            width: 640.0,
            height: 480.0
        }
    );
}

#[test]
fn viewport_fit_uses_configured_page_size_without_scaling_content() {
    let config = PdfExportConfig {
        page_size: PdfPageSize::A4,
        orientation: PdfOrientation::Portrait,
        fit: PdfFitMode::Viewport,
        ..PdfExportConfig::default()
    };

    let layout = resolve_pdf_page_layout(640, 480, 0, 0, None, &config).expect("layout");

    assert_eq!(layout.page_width, A4_WIDTH);
    assert_eq!(layout.page_height, A4_HEIGHT);
    assert_eq!(layout.destination_rect.width, 640.0);
    assert_eq!(layout.destination_rect.height, 480.0);
}

#[test]
fn fit_viewport_to_page_centers_source_on_configured_page() {
    let config = PdfExportConfig {
        page_size: PdfPageSize::Letter,
        orientation: PdfOrientation::Portrait,
        fit: PdfFitMode::FitViewportToPage,
        ..PdfExportConfig::default()
    };

    let layout = resolve_pdf_page_layout(1200, 600, 0, 0, None, &config).expect("layout");

    assert_eq!(layout.page_width, LETTER_WIDTH);
    assert_eq!(layout.page_height, LETTER_HEIGHT);
    assert_eq!(layout.destination_rect.width, LETTER_WIDTH);
    assert!(layout.destination_rect.y > 0.0);
}

#[test]
fn fit_content_uses_shape_bounds_as_source() {
    let config = PdfExportConfig {
        fit: PdfFitMode::FitContentToPage,
        page_size: PdfPageSize::Custom,
        custom_width: 300.0,
        custom_height: 200.0,
        content_source_padding: 0.0,
        ..PdfExportConfig::default()
    };
    let content = CanvasExportRect::new(20.0, 30.0, 100.0, 50.0);

    let layout = resolve_pdf_page_layout(800, 600, 0, 0, content, &config).expect("layout");

    assert_eq!(layout.source_rect, content.expect("content"));
    assert_eq!(layout.destination_rect.width, 300.0);
    assert_eq!(layout.destination_rect.height, 150.0);
    assert_eq!(layout.destination_rect.y, 25.0);
}

#[test]
fn fit_content_expands_source_by_configured_padding() {
    let config = PdfExportConfig {
        fit: PdfFitMode::FitContentToPage,
        page_size: PdfPageSize::Custom,
        custom_width: 300.0,
        custom_height: 200.0,
        content_source_padding: 10.0,
        ..PdfExportConfig::default()
    };
    let content = CanvasExportRect::new(20.0, 30.0, 100.0, 50.0);

    let layout = resolve_pdf_page_layout(800, 600, 0, 0, content, &config).expect("layout");

    assert_eq!(
        layout.source_rect,
        CanvasExportRect {
            x: 10.0,
            y: 20.0,
            width: 120.0,
            height: 70.0
        }
    );
}

#[test]
fn auto_orientation_matches_source_for_standard_pages() {
    let config = PdfExportConfig {
        fit: PdfFitMode::FitViewportToPage,
        page_size: PdfPageSize::A4,
        orientation: PdfOrientation::Auto,
        ..PdfExportConfig::default()
    };

    let layout = resolve_pdf_page_layout(1200, 600, 0, 0, None, &config).expect("layout");

    assert!(layout.page_width > layout.page_height);
}

#[test]
fn rendered_pdf_reports_exact_page_count_and_ordered_sizes() {
    let source = CanvasExportRect::new(0.0, 0.0, 100.0, 100.0).expect("source");
    let pages = vec![
        pdf_page(300.0, 200.0, source, 0, 2),
        pdf_page(200.0, 300.0, source, 1, 2),
    ];
    let bytes = render_board_pdf(&BoardPdfExportSnapshot {
        pages,
        labels: Default::default(),
    })
    .expect("pdf");
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = temp.path().join("out.pdf");
    std::fs::write(&path, bytes).expect("write pdf");

    let output = Command::new("pdfinfo")
        .env("LC_ALL", "C")
        .arg("-f")
        .arg("1")
        .arg("-l")
        .arg("2")
        .arg(&path)
        .output()
        .expect("pdfinfo");
    let text = checked_pdf_output(output);
    assert_eq!(pdf_field(&text, "Pages"), "2");
    assert_eq!(pdf_field(&text, "Page    1 size"), "300 x 200 pts");
    assert_eq!(pdf_field(&text, "Page    2 size"), "200 x 300 pts");
}

fn text_pdf(text_halo_enabled: bool) -> Vec<u8> {
    let source = CanvasExportRect::new(0.0, 0.0, 400.0, 120.0).expect("source");
    let mut page = pdf_page(400.0, 120.0, source, 0, 1);
    page.page.backdrop = CanvasExportBackdropSnapshot::Solid(WHITE);
    page.page.text_halo_enabled = text_halo_enabled;
    page.page.frame.add_shape(Shape::Text {
        x: 20,
        y: 80,
        text: "Read me".to_string(),
        color: RED,
        size: 36.0,
        font_descriptor: FontDescriptor::default(),
        background_enabled: false,
        wrap_width: None,
    });
    render_board_pdf(&BoardPdfExportSnapshot {
        pages: vec![page],
        labels: Default::default(),
    })
    .expect("text PDF renders")
}

#[test]
fn pdf_export_honours_the_text_halo_setting() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    for enabled in [false, true] {
        let path = temp.path().join(format!("halo-{enabled}.pdf"));
        let prefix = temp.path().join(format!("halo-{enabled}"));
        std::fs::write(&path, text_pdf(enabled)).expect("write text PDF");
        checked_pdf_output(
            Command::new("pdftoppm")
                .env("LC_ALL", "C")
                .args(["-png", "-singlefile", "-r", "72"])
                .arg(&path)
                .arg(&prefix)
                .output()
                .expect("pdftoppm is required for PDF artifact tests (install poppler)"),
        );
        let bytes = std::fs::read(prefix.with_extension("png")).expect("rasterized PDF");
        let image = crate::image_decode::decode_rgba(
            crate::image_decode::EncodedImageFormat::Png,
            &bytes,
            crate::screen_pixels::EmbeddedImageLimits::default().into(),
        )
        .expect("decode rasterized PDF");
        assert_eq!((image.width, image.height), (400, 120));
        let red = image
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 180 && p[1] < 80 && p[2] < 80)
            .count();
        let dark = image
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] < 80 && p[1] < 80 && p[2] < 80)
            .count();
        assert!(red > 100, "red glyphs must remain visible: {red}");
        if enabled {
            assert!(
                dark > 100,
                "enabled halo must outline the red glyphs: {dark}"
            );
        } else {
            assert_eq!(dark, 0, "disabled halo must not add a dark outline");
        }
    }
}

fn pdf_page(
    width: f64,
    height: f64,
    source: CanvasExportRect,
    document_page_index: usize,
    document_page_count: usize,
) -> PdfPageExportSnapshot {
    PdfPageExportSnapshot {
        page: CanvasPageExportSnapshot {
            frame: Frame::new(),
            backdrop: CanvasExportBackdropSnapshot::Transparent,
            viewport_width: 100,
            viewport_height: 100,
            origin_x: 0,
            origin_y: 0,
            text_halo_enabled: true,
            spotlight: Default::default(),
        },
        metadata: PdfPageMetadata::new(
            0,
            1,
            0,
            1,
            document_page_index,
            document_page_count,
            document_page_index,
            document_page_count,
            "Board".to_string(),
            None,
        ),
        layout: PdfPageLayout {
            page_width: width,
            page_height: height,
            source_rect: source,
            destination_rect: CanvasExportRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
        },
    }
}

#[test]
fn worker_exports_three_page_pdf_from_unicode_metadata() {
    let source = CanvasExportRect::new(0.0, 0.0, 100.0, 100.0).unwrap();
    let pages = (0..3)
        .map(|index| {
            let mut page = pdf_page(300.0, 200.0, source, index, 3);
            page.metadata.board_name = "Board 測試 العربية".into();
            page
        })
        .collect();
    let snapshot = BoardPdfExportSnapshot {
        pages,
        labels: crate::config::PdfLabelConfig {
            enabled: true,
            content: crate::config::PdfLabelContentMode::BoardName,
            ..Default::default()
        },
    };
    // Only value snapshots cross the worker boundary; text resources are
    // constructed by the export root on the worker thread.
    let bytes = std::thread::spawn(move || render_board_pdf(&snapshot))
        .join()
        .unwrap()
        .unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    // Cairo can compress page dictionaries into PDF object streams. Ask the
    // same PDF reader used by the existing page-layout test to inspect them.
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let path = temp.path().join("worker-pages.pdf");
    std::fs::write(&path, bytes).expect("write worker PDF");
    let info = checked_pdf_output(
        Command::new("pdfinfo")
            .env("LC_ALL", "C")
            .arg(&path)
            .output()
            .expect("pdfinfo is required for PDF artifact tests (install poppler)"),
    );
    assert_eq!(
        pdf_field(&info, "Pages"),
        "3",
        "worker PDF must contain all three pages"
    );
}

fn checked_pdf_output(output: std::process::Output) -> String {
    assert!(
        output.status.success(),
        "PDF reader failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("PDF reader output is UTF-8")
}

fn pdf_field<'a>(info: &'a str, name: &str) -> &'a str {
    info.lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key == name).then(|| value.trim())
        })
        .unwrap_or_else(|| panic!("missing PDF field {name:?}: {info}"))
}
