use super::*;
use crate::config::{PdfExportConfig, PdfFitMode};
use crate::draw::{RED, Shape, WHITE};
use crate::input::BoardSpec;
use std::sync::Arc;

fn board(id: &str, name: &str, background: BoardBackground, pages: Vec<Frame>) -> BoardState {
    BoardState {
        spec: BoardSpec {
            id: id.to_string(),
            name: name.to_string(),
            background,
            default_pen_color: None,
            auto_adjust_pen: false,
            persist: true,
            pinned: false,
        },
        pages: crate::draw::BoardPages::from_pages(pages, 0),
    }
}

fn snapshot_context<'a>(
    boards: &'a [BoardState],
    config: &'a PdfExportConfig,
) -> BoardPdfExportBuildContext<'a> {
    BoardPdfExportBuildContext {
        logical_width: 800,
        logical_height: 600,
        boards,
        active_board_index: 0,
        pan_enabled: true,
        scope: PdfExportScope::ActiveBoard,
        config,
        desktop_backdrop: None,
    }
}

#[test]
fn active_board_pdf_snapshot_preserves_page_order_and_metadata() {
    let mut first = Frame::new();
    first.set_page_name(Some("first".to_string()));
    let mut second = Frame::new();
    second.set_page_name(Some("second".to_string()));
    let boards = vec![board(
        "white",
        "Whiteboard",
        BoardBackground::Solid(WHITE),
        vec![first, second],
    )];

    let config = PdfExportConfig::default();
    let snapshot =
        build_board_pdf_export_snapshot(snapshot_context(&boards, &config)).expect("snapshot");

    assert_eq!(snapshot.pages.len(), 2);
    assert_eq!(snapshot.pages[0].page.frame.page_name(), Some("first"));
    assert_eq!(snapshot.pages[1].page.frame.page_name(), Some("second"));
    assert_eq!(snapshot.pages[0].metadata.document_page_index, 0);
    assert_eq!(snapshot.pages[1].metadata.document_page_label, "2");
    assert_eq!(snapshot.pages[1].metadata.document_page_count_label, "2");
}

#[test]
fn all_boards_pdf_snapshot_uses_app_board_order() {
    let mut first = Frame::new();
    first.set_page_name(Some("one".to_string()));
    let mut second = Frame::new();
    second.set_page_name(Some("two".to_string()));
    let boards = vec![
        board("a", "A", BoardBackground::Transparent, vec![first]),
        board("b", "B", BoardBackground::Solid(WHITE), vec![second]),
    ];

    let config = PdfExportConfig::default();
    let snapshot = build_board_pdf_export_snapshot(BoardPdfExportBuildContext {
        active_board_index: 1,
        scope: PdfExportScope::AllBoards,
        ..snapshot_context(&boards, &config)
    })
    .expect("snapshot");

    let names: Vec<_> = snapshot
        .pages
        .iter()
        .map(|page| page.metadata.board_name.as_str())
        .collect();
    assert_eq!(names, vec!["A", "B"]);
    assert_eq!(snapshot.pages[0].metadata.app_board_index, 0);
    assert_eq!(snapshot.pages[1].metadata.app_board_index, 1);
}

#[test]
fn pdf_snapshot_uses_per_page_view_offset_for_solid_pannable_boards() {
    let mut first = Frame::new();
    assert!(first.set_view_offset(100, -50));
    let mut second = Frame::new();
    assert!(second.set_view_offset(-25, 40));
    let boards = vec![board(
        "white",
        "Whiteboard",
        BoardBackground::Solid(WHITE),
        vec![first, second],
    )];

    let config = PdfExportConfig::default();
    let snapshot =
        build_board_pdf_export_snapshot(snapshot_context(&boards, &config)).expect("snapshot");

    assert_eq!(snapshot.pages[0].page.origin_x, 100);
    assert_eq!(snapshot.pages[0].page.origin_y, -50);
    assert_eq!(snapshot.pages[1].page.origin_x, -25);
    assert_eq!(snapshot.pages[1].page.origin_y, 40);
}

#[test]
fn pdf_snapshot_forces_origin_for_transparent_boards() {
    let mut frame = Frame::new();
    assert!(frame.set_view_offset(100, -50));
    let boards = vec![board(
        "overlay",
        "Overlay",
        BoardBackground::Transparent,
        vec![frame],
    )];

    let config = PdfExportConfig::default();
    let snapshot =
        build_board_pdf_export_snapshot(snapshot_context(&boards, &config)).expect("snapshot");

    assert_eq!(snapshot.pages[0].page.origin_x, 0);
    assert_eq!(snapshot.pages[0].page.origin_y, 0);
}

#[test]
fn fit_content_snapshot_uses_content_bounds() {
    let mut frame = Frame::new();
    frame.add_shape(Shape::Rect {
        x: 20,
        y: 30,
        w: 100,
        h: 50,
        fill: true,
        color: RED,
        thick: 1.0,
    });
    let boards = vec![board(
        "white",
        "Whiteboard",
        BoardBackground::Solid(WHITE),
        vec![frame],
    )];
    let config = PdfExportConfig {
        fit: PdfFitMode::FitContentToPage,
        ..PdfExportConfig::default()
    };

    let snapshot =
        build_board_pdf_export_snapshot(snapshot_context(&boards, &config)).expect("snapshot");

    assert!(snapshot.pages[0].layout.source_rect.x <= 20.0);
    assert!(snapshot.pages[0].layout.source_rect.y <= 30.0);
    assert!(snapshot.pages[0].layout.source_rect.width >= 100.0);
}

#[test]
fn transparent_pdf_pages_use_desktop_backdrop_when_supplied() {
    let boards = vec![board(
        "overlay",
        "Overlay",
        BoardBackground::Transparent,
        vec![Frame::new()],
    )];
    let backdrop = CanvasExportBackdropSnapshot::PersistedImage {
        data: Arc::from(vec![0u8; 800 * 600 * 4]),
        width: 800,
        height: 600,
        stride: 800 * 4,
        logical_to_image_scale_x: 1.0,
        logical_to_image_scale_y: 1.0,
    };

    let config = PdfExportConfig::default();
    let snapshot = build_board_pdf_export_snapshot(BoardPdfExportBuildContext {
        desktop_backdrop: Some(backdrop),
        ..snapshot_context(&boards, &config)
    })
    .expect("snapshot");

    assert!(matches!(
        snapshot.pages[0].page.backdrop,
        CanvasExportBackdropSnapshot::PersistedImage { .. }
    ));
}

#[test]
fn solid_pdf_pages_keep_solid_backdrop_when_desktop_backdrop_supplied() {
    let boards = vec![board(
        "white",
        "Whiteboard",
        BoardBackground::Solid(WHITE),
        vec![Frame::new()],
    )];
    let backdrop = CanvasExportBackdropSnapshot::PersistedImage {
        data: Arc::from(vec![0u8; 800 * 600 * 4]),
        width: 800,
        height: 600,
        stride: 800 * 4,
        logical_to_image_scale_x: 1.0,
        logical_to_image_scale_y: 1.0,
    };

    let config = PdfExportConfig::default();
    let snapshot = build_board_pdf_export_snapshot(BoardPdfExportBuildContext {
        desktop_backdrop: Some(backdrop),
        ..snapshot_context(&boards, &config)
    })
    .expect("snapshot");

    assert!(matches!(
        snapshot.pages[0].page.backdrop,
        CanvasExportBackdropSnapshot::Solid(_)
    ));
}

#[test]
fn pdf_export_scope_detects_transparent_boards() {
    let boards = vec![
        board(
            "overlay",
            "Overlay",
            BoardBackground::Transparent,
            vec![Frame::new()],
        ),
        board(
            "white",
            "Whiteboard",
            BoardBackground::Solid(WHITE),
            vec![Frame::new()],
        ),
    ];

    assert!(pdf_export_scope_has_transparent_pages(
        &boards,
        0,
        PdfExportScope::ActiveBoard
    ));
    assert!(!pdf_export_scope_has_transparent_pages(
        &boards,
        1,
        PdfExportScope::ActiveBoard
    ));
    assert!(pdf_export_scope_has_transparent_pages(
        &boards,
        1,
        PdfExportScope::AllBoards
    ));
}
