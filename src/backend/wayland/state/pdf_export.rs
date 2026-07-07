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
        self.board_pdf_export_snapshot_inner(action, None)
    }

    pub(in crate::backend::wayland) fn board_pdf_export_snapshot_with_desktop_backdrop(
        &self,
        action: Action,
        desktop_backdrop: CanvasExportBackdropSnapshot,
    ) -> Result<BoardPdfExportSnapshot, crate::capture::CaptureError> {
        self.board_pdf_export_snapshot_inner(action, Some(desktop_backdrop))
    }

    fn board_pdf_export_snapshot_inner(
        &self,
        action: Action,
        desktop_backdrop: Option<CanvasExportBackdropSnapshot>,
    ) -> Result<BoardPdfExportSnapshot, crate::capture::CaptureError> {
        let scope = pdf_export_scope_for_action(action);
        build_board_pdf_export_snapshot(BoardPdfExportBuildContext {
            logical_width: self.surface.width(),
            logical_height: self.surface.height(),
            boards: self.input_state.boards.board_states(),
            active_board_index: self.input_state.boards.active_index(),
            pan_enabled: self.input_state.boards.pan_enabled(),
            scope,
            config: &self.config.export.pdf,
            desktop_backdrop,
        })
    }

    pub(in crate::backend::wayland) fn board_pdf_export_scope_has_transparent_pages(
        &self,
        action: Action,
    ) -> bool {
        pdf_export_scope_has_transparent_pages(
            self.input_state.boards.board_states(),
            self.input_state.boards.active_index(),
            pdf_export_scope_for_action(action),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PdfExportScope {
    ActiveBoard,
    AllBoards,
}

struct BoardPdfExportBuildContext<'a> {
    logical_width: u32,
    logical_height: u32,
    boards: &'a [BoardState],
    active_board_index: usize,
    pan_enabled: bool,
    scope: PdfExportScope,
    config: &'a crate::config::PdfExportConfig,
    desktop_backdrop: Option<CanvasExportBackdropSnapshot>,
}

fn build_board_pdf_export_snapshot(
    context: BoardPdfExportBuildContext<'_>,
) -> Result<BoardPdfExportSnapshot, crate::capture::CaptureError> {
    let BoardPdfExportBuildContext {
        logical_width,
        logical_height,
        boards,
        active_board_index,
        pan_enabled,
        scope,
        config,
        desktop_backdrop,
    } = context;

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
            let backdrop =
                backdrop_from_background(&board.spec.background, desktop_backdrop.as_ref());
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

fn pdf_export_scope_for_action(action: Action) -> PdfExportScope {
    match action {
        Action::ExportBoardPdfFile => PdfExportScope::ActiveBoard,
        Action::ExportAllBoardsPdfFile => PdfExportScope::AllBoards,
        _ => PdfExportScope::ActiveBoard,
    }
}

fn pdf_export_scope_has_transparent_pages(
    boards: &[BoardState],
    active_board_index: usize,
    scope: PdfExportScope,
) -> bool {
    export_board_indices(boards, active_board_index, scope)
        .into_iter()
        .filter_map(|index| boards.get(index))
        .any(|board| board.spec.background.is_transparent() && !board.pages.pages().is_empty())
}

fn backdrop_from_background(
    background: &BoardBackground,
    desktop_backdrop: Option<&CanvasExportBackdropSnapshot>,
) -> CanvasExportBackdropSnapshot {
    match background {
        BoardBackground::Transparent => desktop_backdrop
            .cloned()
            .unwrap_or(CanvasExportBackdropSnapshot::Transparent),
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
        let Some(bounds) = drawn.bounding_box() else {
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
mod tests;
