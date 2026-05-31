use crate::canvas_export::{
    BoardPdfExportSnapshot, CanvasExportBackdropSnapshot, CanvasExportRect,
    CanvasPageExportSnapshot, PdfPageExportSnapshot, PdfPageMetadata, resolve_pdf_page_layout,
};
use crate::config::{Action, PdfFitMode};
use crate::draw::Frame;
use crate::input::BoardBackground;
use crate::input::boards::BoardState;

use super::WaylandState;

impl WaylandState {
    pub(in crate::backend::wayland) fn board_pdf_export_snapshot(
        &self,
        action: Action,
    ) -> Result<BoardPdfExportSnapshot, crate::capture::CaptureError> {
        let scope = match action {
            Action::ExportBoardPdfFile => PdfExportScope::ActiveBoard,
            Action::ExportAllBoardsPdfFile => PdfExportScope::AllBoards,
            _ => PdfExportScope::ActiveBoard,
        };
        build_board_pdf_export_snapshot(
            self.surface.width(),
            self.surface.height(),
            self.input_state.boards.board_states(),
            self.input_state.boards.active_index(),
            self.input_state.boards.pan_enabled(),
            scope,
            &self.config.export.pdf,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PdfExportScope {
    ActiveBoard,
    AllBoards,
}

fn build_board_pdf_export_snapshot(
    logical_width: u32,
    logical_height: u32,
    boards: &[BoardState],
    active_board_index: usize,
    pan_enabled: bool,
    scope: PdfExportScope,
    config: &crate::config::PdfExportConfig,
) -> Result<BoardPdfExportSnapshot, crate::capture::CaptureError> {
    let app_board_count = boards.len();
    let export_boards = export_board_indices(boards, active_board_index, scope);
    let export_board_count = export_boards.len();
    let document_page_count = export_boards
        .iter()
        .map(|index| boards[*index].pages.pages().len())
        .sum::<usize>()
        .max(1);

    let mut pages = Vec::with_capacity(document_page_count);
    let mut document_page_index = 0usize;
    for (export_board_index, app_board_index) in export_boards.iter().copied().enumerate() {
        let board = &boards[app_board_index];
        let board_page_count = board.pages.pages().len().max(1);
        for (board_page_index, frame) in board.pages.pages().iter().enumerate() {
            let backdrop = backdrop_from_background(&board.spec.background);
            let use_page_offsets = pan_enabled && !board.spec.background.is_transparent();
            let (origin_x, origin_y) = if use_page_offsets {
                frame.view_offset()
            } else {
                (0, 0)
            };
            let content_bounds = if config.fit == PdfFitMode::FitContentToPage {
                frame_content_bounds(frame)
            } else {
                None
            };
            let layout = resolve_pdf_page_layout(
                logical_width,
                logical_height,
                origin_x,
                origin_y,
                content_bounds,
                config,
            )?;
            let page_name = frame.page_name().map(ToString::to_string);
            pages.push(PdfPageExportSnapshot {
                page: CanvasPageExportSnapshot {
                    frame: frame.clone_without_history(),
                    backdrop,
                    viewport_width: logical_width,
                    viewport_height: logical_height,
                    origin_x,
                    origin_y,
                },
                metadata: PdfPageMetadata::new(
                    app_board_index,
                    app_board_count,
                    export_board_index,
                    export_board_count,
                    board_page_index,
                    board_page_count,
                    document_page_index,
                    document_page_count,
                    board.spec.name.clone(),
                    page_name,
                ),
                layout,
            });
            document_page_index += 1;
        }
    }

    if pages.is_empty() {
        let layout = resolve_pdf_page_layout(logical_width, logical_height, 0, 0, None, config)?;
        pages.push(PdfPageExportSnapshot {
            page: CanvasPageExportSnapshot {
                frame: Frame::new(),
                backdrop: CanvasExportBackdropSnapshot::Transparent,
                viewport_width: logical_width,
                viewport_height: logical_height,
                origin_x: 0,
                origin_y: 0,
            },
            metadata: PdfPageMetadata::new(
                0,
                app_board_count,
                0,
                export_board_count,
                0,
                1,
                0,
                1,
                "Board".to_string(),
                None,
            ),
            layout,
        });
    }

    Ok(BoardPdfExportSnapshot {
        pages,
        labels: config.labels.clone(),
    })
}

fn export_board_indices(
    boards: &[BoardState],
    active_board_index: usize,
    scope: PdfExportScope,
) -> Vec<usize> {
    match scope {
        PdfExportScope::ActiveBoard => {
            if boards.get(active_board_index).is_some() {
                vec![active_board_index]
            } else if boards.is_empty() {
                Vec::new()
            } else {
                vec![0]
            }
        }
        PdfExportScope::AllBoards => (0..boards.len()).collect(),
    }
}

fn backdrop_from_background(background: &BoardBackground) -> CanvasExportBackdropSnapshot {
    match background {
        BoardBackground::Transparent => CanvasExportBackdropSnapshot::Transparent,
        BoardBackground::Solid(color) => CanvasExportBackdropSnapshot::Solid(*color),
    }
}

fn frame_content_bounds(frame: &Frame) -> Option<CanvasExportRect> {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    let mut found = false;

    for drawn in &frame.shapes {
        let Some(bounds) = drawn.shape.bounding_box() else {
            continue;
        };
        min_x = min_x.min(bounds.x);
        min_y = min_y.min(bounds.y);
        max_x = max_x.max(bounds.x.saturating_add(bounds.width));
        max_y = max_y.max(bounds.y.saturating_add(bounds.height));
        found = true;
    }

    found.then(|| {
        CanvasExportRect::new(
            min_x as f64,
            min_y as f64,
            (max_x - min_x).max(1) as f64,
            (max_y - min_y).max(1) as f64,
        )
        .expect("positive bounds")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PdfExportConfig, PdfFitMode};
    use crate::draw::{RED, Shape, WHITE};
    use crate::input::BoardSpec;

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

        let snapshot = build_board_pdf_export_snapshot(
            800,
            600,
            &boards,
            0,
            true,
            PdfExportScope::ActiveBoard,
            &PdfExportConfig::default(),
        )
        .expect("snapshot");

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

        let snapshot = build_board_pdf_export_snapshot(
            800,
            600,
            &boards,
            1,
            true,
            PdfExportScope::AllBoards,
            &PdfExportConfig::default(),
        )
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

        let snapshot = build_board_pdf_export_snapshot(
            800,
            600,
            &boards,
            0,
            true,
            PdfExportScope::ActiveBoard,
            &PdfExportConfig::default(),
        )
        .expect("snapshot");

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

        let snapshot = build_board_pdf_export_snapshot(
            800,
            600,
            &boards,
            0,
            true,
            PdfExportScope::ActiveBoard,
            &PdfExportConfig::default(),
        )
        .expect("snapshot");

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

        let snapshot = build_board_pdf_export_snapshot(
            800,
            600,
            &boards,
            0,
            true,
            PdfExportScope::ActiveBoard,
            &config,
        )
        .expect("snapshot");

        assert!(snapshot.pages[0].layout.source_rect.x <= 20.0);
        assert!(snapshot.pages[0].layout.source_rect.y <= 30.0);
        assert!(snapshot.pages[0].layout.source_rect.width >= 100.0);
    }
}
