use super::*;
use crate::config::{PdfExportConfig, PdfFitMode, PdfOrientation, PdfPageSize};
use crate::draw::Frame;
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
fn rendered_pdf_reports_page_count_and_sizes_when_pdfinfo_is_available() {
    if Command::new("pdfinfo").arg("-v").output().is_err() {
        return;
    }
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
        .arg("-f")
        .arg("1")
        .arg("-l")
        .arg("2")
        .arg(&path)
        .output()
        .expect("pdfinfo");
    if !output.status.success() {
        return;
    }
    let text = String::from_utf8_lossy(&output.stdout);

    assert!(text.contains("Pages:"));
    assert!(text.contains('2'));
    assert!(text.contains("300 x 200") || text.contains("200 x 300"));
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
