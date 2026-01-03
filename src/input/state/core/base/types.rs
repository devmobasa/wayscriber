//! Drawing state machine and input state management.

pub const MIN_STROKE_THICKNESS: f64 = 1.0;
pub const MAX_STROKE_THICKNESS: f64 = 50.0;
pub const PRESET_FEEDBACK_DURATION_MS: u64 = 450;
pub const PRESET_TOAST_DURATION_MS: u64 = 1300;
pub const UI_TOAST_DURATION_MS: u64 = 5000;

use crate::config::ToolPresetConfig;
use crate::draw::ShapeId;
use crate::draw::frame::ShapeSnapshot;
use crate::input::tool::Tool;
use std::time::Instant;

/// Current drawing mode state machine.
///
/// Tracks whether the user is idle, actively drawing a shape, or entering text.
/// State transitions occur based on mouse and keyboard events.
#[derive(Debug)]
pub enum DrawingState {
    /// Not actively drawing - waiting for user input
    Idle,
    /// Actively drawing a shape (mouse button held down)
    Drawing {
        /// Which tool is being used for this shape
        tool: Tool,
        /// Starting X coordinate (where mouse was pressed)
        start_x: i32,
        /// Starting Y coordinate (where mouse was pressed)
        start_y: i32,
        /// Accumulated points for freehand drawing
        points: Vec<(i32, i32)>,
    },
    /// Text input mode - user is typing text to place on screen
    TextInput {
        /// X coordinate where text will be placed
        x: i32,
        /// Y coordinate where text will be placed
        y: i32,
        /// Accumulated text buffer
        buffer: String,
    },
    /// Pending click on text/note to detect double-click editing
    PendingTextClick {
        /// Starting X coordinate
        x: i32,
        /// Starting Y coordinate
        y: i32,
        /// Active tool when the click began
        tool: Tool,
        /// Shape id that was clicked
        shape_id: ShapeId,
    },
    /// Selection move mode - user is dragging selected shapes
    MovingSelection {
        /// Last pointer X coordinate applied
        last_x: i32,
        /// Last pointer Y coordinate applied
        last_y: i32,
        /// Snapshots of shapes prior to movement (for undo/cancel)
        snapshots: Vec<(ShapeId, ShapeSnapshot)>,
        /// Whether any translation has been applied
        moved: bool,
    },
    /// Selection box mode - user is dragging a rectangle to select shapes
    Selecting {
        /// Starting X coordinate
        start_x: i32,
        /// Starting Y coordinate
        start_y: i32,
        /// Whether the selection should be additive
        additive: bool,
    },
    /// Resize text/note wrap width by dragging a handle
    ResizingText {
        /// Shape id being resized
        shape_id: ShapeId,
        /// Snapshot of the shape prior to resizing (for undo/cancel)
        snapshot: ShapeSnapshot,
        /// Text baseline X coordinate (wrap width is measured from here)
        base_x: i32,
        /// Font size used to set minimum width
        size: f64,
    },
}

/// Describes which kind of text input is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputMode {
    Plain,
    StickyNote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomAction {
    In,
    Out,
    Reset,
    ToggleLock,
    RefreshCapture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarDrawerTab {
    View,
    App,
}

impl ToolbarDrawerTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::View => "View",
            Self::App => "Settings",
        }
    }
}

#[derive(Debug, Clone)]
pub enum PresetAction {
    Save {
        slot: usize,
        preset: ToolPresetConfig,
    },
    Clear {
        slot: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetFeedbackKind {
    Apply,
    Save,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiToastKind {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub(crate) struct PresetFeedbackState {
    pub kind: PresetFeedbackKind,
    pub started: Instant,
}

#[derive(Debug, Clone)]
pub(crate) struct UiToastState {
    pub kind: UiToastKind,
    pub message: String,
    pub started: Instant,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TextClickState {
    pub shape_id: ShapeId,
    pub x: i32,
    pub y: i32,
    pub at: Instant,
}

/// Tracks in-progress delayed undo/redo playback.
pub(crate) struct DelayedHistory {
    pub mode: HistoryMode,
    pub remaining: usize,
    pub delay_ms: u64,
    pub next_due: Instant,
}

#[derive(Clone, Copy)]
pub(crate) enum HistoryMode {
    Undo,
    Redo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpOverlayView {
    Quick,
    Full,
}

impl HelpOverlayView {
    pub fn toggle(self) -> Self {
        match self {
            HelpOverlayView::Quick => HelpOverlayView::Full,
            HelpOverlayView::Full => HelpOverlayView::Quick,
        }
    }

    pub fn page_count(self) -> usize {
        match self {
            HelpOverlayView::Quick => 1,
            HelpOverlayView::Full => 2,
        }
    }
}
