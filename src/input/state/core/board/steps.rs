//! Step-capture session: an armed mode that turns captured screen frames
//! into numbered guide pages on a dedicated Steps board.
//!
//! Each step page carries a locked full-screen image shape (the captured
//! frame) beneath a numbered step marker. Both are ordinary shapes, so
//! rendering, session persistence, reordering, and export all apply
//! unchanged; the lock keeps the backdrop from being dragged while the
//! user annotates on top of it.

use super::super::base::InputState;
use super::super::utility::step_markers::default_step_marker_size;
use crate::draw::{EmbeddedImage, Frame, Shape, StepMarkerLabel};

/// Board that collects captured steps. Created on the first capture; the
/// id keeps re-arming sessions appending to the same guide.
pub const STEP_CAPTURE_BOARD_ID: &str = "steps";
const STEP_CAPTURE_BOARD_NAME: &str = "Steps";

/// One captured frame ready to become a step page.
#[derive(Debug, Clone)]
pub struct StepCaptureFrame {
    /// Encoded captured image (PNG) with its natural pixel dimensions.
    pub image: EmbeddedImage,
    /// Logical size of the captured output; the image shape spans it so
    /// annotations land in the same coordinate space the user saw.
    pub logical_width: i32,
    pub logical_height: i32,
    /// Marker position in surface-local logical coordinates. `None` places
    /// no marker (the frame was captured without a meaningful pointer).
    pub marker: Option<(i32, i32)>,
}

/// What a successful append produced, for user feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepPageReceipt {
    pub step: u32,
    pub page_count: usize,
}

/// Why an append did or did not land, so the caller can explain itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepPageOutcome {
    Appended(StepPageReceipt),
    /// The board limit prevents creating the Steps board.
    BoardLimit,
    /// The captured frame would push the session file past its save limits.
    SessionLimit,
    /// The configured per-frame shape limit cannot retain the backdrop and marker.
    ShapeLimit,
}

impl StepPageOutcome {
    pub fn receipt(self) -> Option<StepPageReceipt> {
        match self {
            Self::Appended(receipt) => Some(receipt),
            Self::BoardLimit | Self::SessionLimit | Self::ShapeLimit => None,
        }
    }
}

impl InputState {
    pub fn step_capture_armed(&self) -> bool {
        self.step_capture_armed
    }

    /// Arms or disarms the step-capture session. Disarming jumps to the
    /// Steps board's first page for review when any step was captured.
    /// Returns the new armed state.
    pub(crate) fn toggle_step_capture(&mut self) -> bool {
        self.step_capture_armed = !self.step_capture_armed;
        if !self.step_capture_armed && self.step_board_index().is_some() {
            self.switch_board_force(STEP_CAPTURE_BOARD_ID);
            self.switch_to_page(0);
        }
        self.needs_redraw = true;
        self.step_capture_armed
    }

    /// The number the next captured step will get: one past the Steps
    /// board's current page count (1 when the board does not exist yet).
    pub fn next_step_number(&self) -> u32 {
        let pages = self
            .step_board_index()
            .and_then(|index| self.boards.board_states().get(index))
            .map_or(0, |board| {
                if board_is_blank(board) {
                    0
                } else {
                    board.pages.page_count()
                }
            });
        pages.saturating_add(1) as u32
    }

    /// Appends one captured frame as a new step page: locked full-screen
    /// image shape plus a numbered marker.
    ///
    /// The page is built before it is inserted so the session preflight can
    /// weigh the captured PNG against the save limits: a guide long enough to
    /// blow past them would otherwise make every autosave and the final save
    /// fail, losing the whole session rather than one step.
    pub(crate) fn append_step_page(&mut self, frame: StepCaptureFrame) -> StepPageOutcome {
        let required_shapes = 1 + usize::from(frame.marker.is_some());
        if required_shapes > self.max_shapes_per_frame {
            self.push_toast(
                crate::input::state::ToastPriority::Critical,
                "steps",
                crate::input::state::Toast::error(format!(
                    "Step capture needs {required_shapes} shapes per page, but session.max_shapes_per_frame is {}.",
                    self.max_shapes_per_frame
                )),
            );
            return StepPageOutcome::ShapeLimit;
        }
        let Some(board_index) = self.ensure_step_board() else {
            return StepPageOutcome::BoardLimit;
        };
        let Some(board) = self.boards.board_states().get(board_index) else {
            return StepPageOutcome::BoardLimit;
        };
        // A freshly created board starts with one empty page; the first
        // step claims it instead of leaving a blank leading page.
        let claims_blank_page = board_is_blank(board);
        let step = if claims_blank_page {
            1
        } else {
            board.pages.page_count().saturating_add(1) as u32
        };
        let page = self.build_step_page(&frame, step);
        if !self.session_allows_step_page(board_index, &page) {
            return StepPageOutcome::SessionLimit;
        }

        let is_active_board = self.boards.active_index() == board_index;
        if is_active_board {
            self.prepare_active_page_content_change();
        }
        let Some(board) = self.boards.board_state_mut(board_index) else {
            return StepPageOutcome::BoardLimit;
        };
        if claims_blank_page {
            board.pages.switch_to_page(0);
        } else {
            board.pages.new_page();
        }
        *board.pages.active_frame_mut() = page;
        let step = (board.pages.active_index() + 1) as u32;
        let page_count = board.pages.page_count();

        self.renumber_step_pages();
        self.finish_board_page_content_change(board_index);
        StepPageOutcome::Appended(StepPageReceipt { step, page_count })
    }

    /// Builds the page a captured frame becomes, without touching the board.
    fn build_step_page(&self, frame: &StepCaptureFrame, step: u32) -> Frame {
        let mut page = Frame::new();
        let image_id = page.add_shape(Shape::Image {
            x: 0,
            y: 0,
            w: frame.logical_width.max(1),
            h: frame.logical_height.max(1),
            data: frame.image.clone(),
        });
        if let Some(backdrop) = page.shape_mut(image_id) {
            backdrop.locked = true;
        }
        if let Some((x, y)) = frame.marker {
            page.add_shape(Shape::StepMarker {
                x,
                y,
                color: self.current_color,
                label: StepMarkerLabel {
                    value: step,
                    size: default_step_marker_size(self.current_font_size),
                    font_descriptor: self.font_descriptor.clone(),
                    auto_numbered: true,
                },
            });
        }
        page
    }

    /// Rewrites every capture-placed marker on the Steps board so its number
    /// matches its page's position. Steps pages stay reorderable, deletable,
    /// duplicable, and restorable, and the Markdown export numbers headings
    /// positionally — without this the visible markers drift out of step with
    /// the exported guide after any such edit. Hand-placed markers (the
    /// step-marker tool) are left alone.
    pub(crate) fn renumber_step_pages(&mut self) {
        let Some(board_index) = self.step_board_index() else {
            return;
        };
        let Some(board) = self.boards.board_state_mut(board_index) else {
            return;
        };
        let mut changed = false;
        for (index, page) in board.pages.pages_mut().iter_mut().enumerate() {
            let number = index.saturating_add(1) as u32;
            for drawn in &mut page.shapes {
                if let Shape::StepMarker { label, .. } = &mut drawn.shape
                    && label.auto_numbered
                    && label.value != number
                {
                    label.value = number;
                    changed = true;
                }
            }
        }
        if changed {
            self.mark_board_surface_changed();
        }
    }

    fn step_board_index(&self) -> Option<usize> {
        self.boards
            .board_states()
            .iter()
            .position(|board| board.spec.id == STEP_CAPTURE_BOARD_ID)
    }

    fn ensure_step_board(&mut self) -> Option<usize> {
        if let Some(index) = self.step_board_index() {
            return Some(index);
        }
        let board = self.boards.ensure_board(STEP_CAPTURE_BOARD_ID)?;
        board.spec.name = STEP_CAPTURE_BOARD_NAME.to_string();
        self.step_board_index()
    }
}

fn board_is_blank(board: &crate::input::boards::BoardState) -> bool {
    board.pages.page_count() == 1
        && board
            .pages
            .pages()
            .first()
            .is_some_and(|page| page.shapes.is_empty())
}
