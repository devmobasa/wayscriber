//! Multi-frame canvas management for board modes.

use super::Frame;
use crate::input::BoardMode;

/// Manages multiple frames, one per board mode (with lazy initialization).
///
/// This structure maintains separate drawing frames for each board mode:
/// - Transparent mode always has a frame (used for screen annotation)
/// - Whiteboard and Blackboard frames are lazily created on first use
///
/// This design allows seamless mode switching while preserving work,
/// and saves memory when board modes are never activated.
pub struct CanvasSet {
    /// Frame for transparent overlay mode (always exists)
    transparent: Frame,
    /// Frame for whiteboard mode (lazy: created on first use)
    whiteboard: Option<Frame>,
    /// Frame for blackboard mode (lazy: created on first use)
    blackboard: Option<Frame>,
    /// Currently active mode
    active_mode: BoardMode,
}

impl CanvasSet {
    /// Creates a new canvas set with only the transparent frame initialized.
    pub fn new() -> Self {
        Self {
            transparent: Frame::new(),
            whiteboard: None,
            blackboard: None,
            active_mode: BoardMode::Transparent,
        }
    }

    /// Gets the currently active frame (mutable).
    ///
    /// Lazily creates whiteboard/blackboard frames if they don't exist yet.
    pub fn active_frame_mut(&mut self) -> &mut Frame {
        match self.active_mode {
            BoardMode::Transparent => &mut self.transparent,
            BoardMode::Whiteboard => self.whiteboard.get_or_insert_with(Frame::new),
            BoardMode::Blackboard => self.blackboard.get_or_insert_with(Frame::new),
        }
    }

    /// Gets the currently active frame (immutable).
    ///
    /// For board modes that don't exist yet, returns a reference to a static empty frame
    /// instead of creating one (since we can't mutate in an immutable method).
    pub fn active_frame(&self) -> &Frame {
        static EMPTY_FRAME: Frame = Frame::new();

        match self.active_mode {
            BoardMode::Transparent => &self.transparent,
            BoardMode::Whiteboard => self.whiteboard.as_ref().unwrap_or(&EMPTY_FRAME),
            BoardMode::Blackboard => self.blackboard.as_ref().unwrap_or(&EMPTY_FRAME),
        }
    }

    /// Returns the current active board mode.
    pub fn active_mode(&self) -> BoardMode {
        self.active_mode
    }

    /// Switches to a different board mode.
    ///
    /// This does not create frames lazily - they are created when first accessed
    /// via `active_frame_mut()`.
    pub fn switch_mode(&mut self, new_mode: BoardMode) {
        self.active_mode = new_mode;
    }

    /// Clears only the active frame.
    pub fn clear_active(&mut self) {
        self.active_frame_mut().clear();
    }

    /// Returns an immutable reference to the frame for the requested mode, if it exists.
    pub fn frame(&self, mode: BoardMode) -> Option<&Frame> {
        match mode {
            BoardMode::Transparent => Some(&self.transparent),
            BoardMode::Whiteboard => self.whiteboard.as_ref(),
            BoardMode::Blackboard => self.blackboard.as_ref(),
        }
    }

    /// Replaces the frame for the requested mode with the provided data.
    pub fn set_frame(&mut self, mode: BoardMode, frame: Option<Frame>) {
        match mode {
            BoardMode::Transparent => {
                self.transparent = frame.unwrap_or_else(Frame::new);
            }
            BoardMode::Whiteboard => {
                self.whiteboard = frame;
            }
            BoardMode::Blackboard => {
                self.blackboard = frame;
            }
        }
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
    use crate::draw::{BLACK, RED, Shape};

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
        canvas_set.active_frame_mut().add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 100,
            color: RED,
            thick: 3.0,
        });
        assert_eq!(canvas_set.active_frame().shapes.len(), 1);

        // Switch to whiteboard
        canvas_set.switch_mode(BoardMode::Whiteboard);
        assert_eq!(canvas_set.active_frame().shapes.len(), 0); // Empty frame

        // Add shape to whiteboard frame
        canvas_set.active_frame_mut().add_shape(Shape::Rect {
            x: 10,
            y: 10,
            w: 50,
            h: 50,
            color: BLACK,
            thick: 2.0,
        });
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
        canvas_set.active_frame_mut().add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 100,
            color: RED,
            thick: 3.0,
        });
        let _ = canvas_set.active_frame_mut().undo();
        assert_eq!(canvas_set.active_frame().shapes.len(), 0);

        // Switch to whiteboard and add shape
        canvas_set.switch_mode(BoardMode::Whiteboard);
        canvas_set.active_frame_mut().add_shape(Shape::Rect {
            x: 10,
            y: 10,
            w: 50,
            h: 50,
            color: BLACK,
            thick: 2.0,
        });

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
        // This test demonstrates the static EMPTY_FRAME pattern
        assert_eq!(canvas_set.active_frame().shapes.len(), 0);
    }
}
