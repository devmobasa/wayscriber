use super::pages::{BoardPages, PageDeleteOutcome};
use crate::draw::Frame;
use crate::input::BoardMode;
use std::sync::LazyLock;

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
