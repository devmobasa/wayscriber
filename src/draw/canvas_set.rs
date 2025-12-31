//! Multi-frame canvas management for board modes.

use super::Frame;
use crate::input::BoardMode;
use std::sync::LazyLock;

/// Collection of pages for a single board mode.
#[derive(Debug, Clone)]
pub struct BoardPages {
    pages: Vec<Frame>,
    active: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageDeleteOutcome {
    Removed,
    Cleared,
}

impl Default for BoardPages {
    fn default() -> Self {
        Self::new()
    }
}

impl BoardPages {
    pub fn new() -> Self {
        Self {
            pages: vec![Frame::new()],
            active: 0,
        }
    }

    pub fn from_pages(mut pages: Vec<Frame>, active: usize) -> Self {
        if pages.is_empty() {
            pages.push(Frame::new());
        }
        let active = active.min(pages.len() - 1);
        Self { pages, active }
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active_frame(&self) -> &Frame {
        &self.pages[self.active]
    }

    pub fn active_frame_mut(&mut self) -> &mut Frame {
        &mut self.pages[self.active]
    }

    pub fn next_page(&mut self) -> bool {
        if self.active + 1 < self.pages.len() {
            self.active += 1;
            true
        } else {
            false
        }
    }

    pub fn prev_page(&mut self) -> bool {
        if self.active > 0 {
            self.active -= 1;
            true
        } else {
            false
        }
    }

    pub fn new_page(&mut self) {
        self.pages.push(Frame::new());
        self.active = self.pages.len() - 1;
    }

    pub fn duplicate_page(&mut self) {
        let cloned = self.active_frame().clone_without_history();
        self.pages.push(cloned);
        self.active = self.pages.len() - 1;
    }

    pub fn delete_page(&mut self) -> PageDeleteOutcome {
        if self.pages.len() == 1 {
            self.pages[0].clear();
            PageDeleteOutcome::Cleared
        } else {
            self.pages.remove(self.active);
            if self.active >= self.pages.len() {
                self.active = self.pages.len() - 1;
            }
            PageDeleteOutcome::Removed
        }
    }

    #[allow(dead_code)]
    pub fn trim_trailing_empty_pages(&mut self) {
        while self.pages.len() > 1
            && self
                .pages
                .last()
                .is_some_and(|frame| !frame.has_persistable_data())
        {
            self.pages.pop();
            if self.active >= self.pages.len() {
                self.active = self.pages.len() - 1;
            }
        }
    }

    pub fn pages(&self) -> &[Frame] {
        &self.pages
    }

    pub fn pages_mut(&mut self) -> &mut Vec<Frame> {
        &mut self.pages
    }
}

/// Manages multiple frames, one per board mode (with lazy initialization).
///
/// This structure maintains separate drawing frames for each board mode:
/// - Transparent mode always has pages (used for screen annotation)
/// - Whiteboard and Blackboard pages are lazily created on first use
///
/// This design allows seamless mode switching while preserving work,
/// and saves memory when board modes are never activated.
pub struct CanvasSet {
    /// Pages for transparent overlay mode (always exists)
    transparent: BoardPages,
    /// Pages for whiteboard mode (lazy: created on first use)
    whiteboard: Option<BoardPages>,
    /// Pages for blackboard mode (lazy: created on first use)
    blackboard: Option<BoardPages>,
    /// Currently active mode
    active_mode: BoardMode,
}

impl CanvasSet {
    /// Creates a new canvas set with only the transparent pages initialized.
    pub fn new() -> Self {
        Self {
            transparent: BoardPages::new(),
            whiteboard: None,
            blackboard: None,
            active_mode: BoardMode::Transparent,
        }
    }

    fn ensure_pages_mut(&mut self, mode: BoardMode) -> &mut BoardPages {
        match mode {
            BoardMode::Transparent => &mut self.transparent,
            BoardMode::Whiteboard => self.whiteboard.get_or_insert_with(BoardPages::new),
            BoardMode::Blackboard => self.blackboard.get_or_insert_with(BoardPages::new),
        }
    }

    fn pages_or_empty(&self, mode: BoardMode) -> &BoardPages {
        static EMPTY_PAGES: LazyLock<BoardPages> = LazyLock::new(BoardPages::new);
        match mode {
            BoardMode::Transparent => &self.transparent,
            BoardMode::Whiteboard => self.whiteboard.as_ref().unwrap_or(&EMPTY_PAGES),
            BoardMode::Blackboard => self.blackboard.as_ref().unwrap_or(&EMPTY_PAGES),
        }
    }

    /// Gets the currently active frame (mutable).
    ///
    /// Lazily creates whiteboard/blackboard pages if they don't exist yet.
    pub fn active_frame_mut(&mut self) -> &mut Frame {
        self.ensure_pages_mut(self.active_mode).active_frame_mut()
    }

    /// Gets the currently active frame (immutable).
    ///
    /// For board modes that don't exist yet, returns a reference to a static empty frame
    /// instead of creating one (since we can't mutate in an immutable method).
    pub fn active_frame(&self) -> &Frame {
        self.pages_or_empty(self.active_mode).active_frame()
    }

    /// Returns the current active board mode.
    pub fn active_mode(&self) -> BoardMode {
        self.active_mode
    }

    /// Switches to a different board mode.
    ///
    /// This does not create pages lazily - they are created when first accessed
    /// via `active_frame_mut()`.
    pub fn switch_mode(&mut self, new_mode: BoardMode) {
        self.active_mode = new_mode;
    }

    /// Clears only the active frame.
    #[allow(dead_code)]
    pub fn clear_active(&mut self) {
        self.active_frame_mut().clear();
    }

    /// Returns an immutable reference to the pages for the requested mode, if it exists.
    pub fn pages(&self, mode: BoardMode) -> Option<&BoardPages> {
        match mode {
            BoardMode::Transparent => Some(&self.transparent),
            BoardMode::Whiteboard => self.whiteboard.as_ref(),
            BoardMode::Blackboard => self.blackboard.as_ref(),
        }
    }

    /// Returns a mutable reference to the pages for the requested mode, if it exists.
    pub fn pages_mut(&mut self, mode: BoardMode) -> Option<&mut BoardPages> {
        match mode {
            BoardMode::Transparent => Some(&mut self.transparent),
            BoardMode::Whiteboard => self.whiteboard.as_mut(),
            BoardMode::Blackboard => self.blackboard.as_mut(),
        }
    }

    /// Returns the active frame for the requested mode, if it exists.
    #[allow(dead_code)]
    pub fn frame(&self, mode: BoardMode) -> Option<&Frame> {
        self.pages(mode).map(|pages| pages.active_frame())
    }

    /// Returns the active frame for the requested mode, if it exists.
    #[allow(dead_code)]
    pub fn frame_mut(&mut self, mode: BoardMode) -> Option<&mut Frame> {
        self.pages_mut(mode).map(|pages| pages.active_frame_mut())
    }

    /// Replaces the pages for the requested mode with the provided data.
    pub fn set_pages(&mut self, mode: BoardMode, pages: Option<BoardPages>) {
        match mode {
            BoardMode::Transparent => {
                self.transparent = pages.unwrap_or_default();
            }
            BoardMode::Whiteboard => {
                self.whiteboard = pages;
            }
            BoardMode::Blackboard => {
                self.blackboard = pages;
            }
        }
    }

    pub fn page_count(&self, mode: BoardMode) -> usize {
        self.pages(mode)
            .map(|pages| pages.page_count())
            .unwrap_or(1)
    }

    pub fn active_page_index(&self, mode: BoardMode) -> usize {
        self.pages(mode)
            .map(|pages| pages.active_index())
            .unwrap_or(0)
    }

    pub fn next_page(&mut self, mode: BoardMode) -> bool {
        self.ensure_pages_mut(mode).next_page()
    }

    pub fn prev_page(&mut self, mode: BoardMode) -> bool {
        self.ensure_pages_mut(mode).prev_page()
    }

    pub fn new_page(&mut self, mode: BoardMode) {
        self.ensure_pages_mut(mode).new_page();
    }

    pub fn duplicate_page(&mut self, mode: BoardMode) {
        self.ensure_pages_mut(mode).duplicate_page();
    }

    pub fn delete_page(&mut self, mode: BoardMode) -> PageDeleteOutcome {
        self.ensure_pages_mut(mode).delete_page()
    }
}

impl Default for CanvasSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{BLACK, RED, Shape, frame::UndoAction};

    #[test]
    fn test_initial_mode_is_transparent() {
        let canvas_set = CanvasSet::new();
        assert_eq!(canvas_set.active_mode(), BoardMode::Transparent);
    }

    #[test]
    fn test_frame_created_on_first_mutable_access() {
        let mut canvas_set = CanvasSet::new();

        // Switch to whiteboard
        canvas_set.switch_mode(BoardMode::Whiteboard);

        // Access the frame (this should create it via lazy initialization)
        let frame = canvas_set.active_frame_mut();

        // Frame should be empty initially
        assert_eq!(frame.shapes.len(), 0);
    }

    #[test]
    fn test_frame_isolation() {
        let mut canvas_set = CanvasSet::new();

        // Add shape to transparent frame
        let frame = canvas_set.active_frame_mut();
        let id = frame.add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 100,
            color: RED,
            thick: 3.0,
        });
        let index = frame.find_index(id).unwrap();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(index, frame.shape(id).unwrap().clone())],
            },
            10,
        );
        assert_eq!(canvas_set.active_frame().shapes.len(), 1);

        // Switch to whiteboard
        canvas_set.switch_mode(BoardMode::Whiteboard);
        assert_eq!(canvas_set.active_frame().shapes.len(), 0); // Empty frame

        // Add shape to whiteboard frame
        let frame = canvas_set.active_frame_mut();
        let id = frame.add_shape(Shape::Rect {
            x: 10,
            y: 10,
            w: 50,
            h: 50,
            fill: false,
            color: BLACK,
            thick: 2.0,
        });
        let index = frame.find_index(id).unwrap();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(index, frame.shape(id).unwrap().clone())],
            },
            10,
        );
        assert_eq!(canvas_set.active_frame().shapes.len(), 1);

        // Switch back to transparent
        canvas_set.switch_mode(BoardMode::Transparent);
        assert_eq!(canvas_set.active_frame().shapes.len(), 1); // Original shape still there

        // Verify whiteboard still has its shape
        canvas_set.switch_mode(BoardMode::Whiteboard);
        assert_eq!(canvas_set.active_frame().shapes.len(), 1);
    }

    #[test]
    fn test_undo_isolation() {
        let mut canvas_set = CanvasSet::new();

        // Add and undo in transparent mode
        let frame = canvas_set.active_frame_mut();
        let id = frame.add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 100,
            color: RED,
            thick: 3.0,
        });
        let index = frame.find_index(id).unwrap();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(index, frame.shape(id).unwrap().clone())],
            },
            10,
        );
        let _ = canvas_set.active_frame_mut().undo_last();
        assert_eq!(canvas_set.active_frame().shapes.len(), 0);

        // Switch to whiteboard and add shape
        canvas_set.switch_mode(BoardMode::Whiteboard);
        let frame = canvas_set.active_frame_mut();
        let id = frame.add_shape(Shape::Rect {
            x: 10,
            y: 10,
            w: 50,
            h: 50,
            fill: false,
            color: BLACK,
            thick: 2.0,
        });
        let index = frame.find_index(id).unwrap();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(index, frame.shape(id).unwrap().clone())],
            },
            10,
        );

        // Undo should only affect whiteboard frame
        let _ = canvas_set.active_frame_mut().undo();
        assert_eq!(canvas_set.active_frame().shapes.len(), 0);

        // Transparent frame should still be empty (undo happened there earlier)
        canvas_set.switch_mode(BoardMode::Transparent);
        assert_eq!(canvas_set.active_frame().shapes.len(), 0);
    }

    #[test]
    fn test_clear_active() {
        let mut canvas_set = CanvasSet::new();

        // Add shapes to transparent
        canvas_set.active_frame_mut().add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 100,
            color: RED,
            thick: 3.0,
        });

        // Add shapes to whiteboard
        canvas_set.switch_mode(BoardMode::Whiteboard);
        canvas_set.active_frame_mut().add_shape(Shape::Rect {
            x: 10,
            y: 10,
            w: 50,
            h: 50,
            fill: false,
            color: BLACK,
            thick: 2.0,
        });

        // Clear whiteboard only
        canvas_set.clear_active();
        assert_eq!(canvas_set.active_frame().shapes.len(), 0);

        // Transparent should still have its shape
        canvas_set.switch_mode(BoardMode::Transparent);
        assert_eq!(canvas_set.active_frame().shapes.len(), 1);
    }

    #[test]
    fn test_immutable_access_to_nonexistent_frame() {
        let canvas_set = CanvasSet::new();

        // Accessing a non-existent board frame immutably should work
        // (returns empty frame reference, doesn't create it)
        // This test demonstrates the static EMPTY_PAGES pattern
        assert_eq!(canvas_set.active_frame().shapes.len(), 0);
    }

    #[test]
    fn test_page_navigation_and_delete() {
        let mut canvas_set = CanvasSet::new();
        assert_eq!(canvas_set.page_count(BoardMode::Transparent), 1);
        assert_eq!(canvas_set.active_page_index(BoardMode::Transparent), 0);
        assert!(!canvas_set.next_page(BoardMode::Transparent));

        canvas_set.new_page(BoardMode::Transparent);
        assert_eq!(canvas_set.page_count(BoardMode::Transparent), 2);
        assert_eq!(canvas_set.active_page_index(BoardMode::Transparent), 1);
        assert!(canvas_set.prev_page(BoardMode::Transparent));
        assert_eq!(canvas_set.active_page_index(BoardMode::Transparent), 0);

        canvas_set.duplicate_page(BoardMode::Transparent);
        assert_eq!(canvas_set.page_count(BoardMode::Transparent), 3);
        assert_eq!(canvas_set.active_page_index(BoardMode::Transparent), 2);

        let outcome = canvas_set.delete_page(BoardMode::Transparent);
        assert_eq!(outcome, PageDeleteOutcome::Removed);
        assert_eq!(canvas_set.page_count(BoardMode::Transparent), 2);
    }

    #[test]
    fn test_delete_last_page_clears() {
        let mut canvas_set = CanvasSet::new();
        canvas_set.active_frame_mut().add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            color: RED,
            thick: 2.0,
        });

        let outcome = canvas_set.delete_page(BoardMode::Transparent);
        assert_eq!(outcome, PageDeleteOutcome::Cleared);
        assert_eq!(canvas_set.active_frame().shapes.len(), 0);
    }
}
