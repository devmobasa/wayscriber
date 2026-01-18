//! Drawing state machine and input state management.

pub const MIN_STROKE_THICKNESS: f64 = 1.0;
pub const MAX_STROKE_THICKNESS: f64 = 50.0;
pub const PRESET_FEEDBACK_DURATION_MS: u64 = 450;
pub const PRESET_TOAST_DURATION_MS: u64 = 1300;
pub const UI_TOAST_DURATION_MS: u64 = 5000;
pub const BLOCKED_ACTION_DURATION_MS: u64 = 200;
pub const BOARD_UNDO_EXPIRE_MS: u64 = 30_000;
#[allow(dead_code)]
pub const STATUS_CHANGE_HIGHLIGHT_MS: u64 = 300;

use crate::capture::file::FileSaveConfig;
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
            Self::View => "Canvas",
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

/// Action that can be triggered by clicking a toast.
#[derive(Debug, Clone)]
pub struct ToastAction {
    pub label: String,
    #[allow(dead_code)] // Used in check_toast_click via WaylandState
    pub action: crate::config::keybindings::Action,
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
    pub duration_ms: u64,
    /// Optional action that triggers when the toast is clicked.
    pub action: Option<ToastAction>,
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

/// Tracks which compositor features are available.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompositorCapabilities {
    pub layer_shell: bool,
    pub screencopy: bool,
    pub pointer_constraints: bool,
}

impl CompositorCapabilities {
    pub fn all_available(&self) -> bool {
        self.layer_shell && self.screencopy && self.pointer_constraints
    }

    pub fn limitations_summary(&self) -> Option<String> {
        let mut issues = Vec::new();
        if !self.layer_shell {
            issues.push("Toolbars limited");
        }
        if !self.screencopy {
            issues.push("Freeze unavailable");
        }
        if !self.pointer_constraints {
            issues.push("Pointer lock unavailable");
        }
        if issues.is_empty() {
            None
        } else {
            Some(issues.join(", "))
        }
    }
}

/// State for blocked action visual feedback (red flash).
#[derive(Debug, Clone)]
pub(crate) struct BlockedActionFeedback {
    pub started: Instant,
}

/// Pending clipboard fallback data for when clipboard copy fails.
#[derive(Debug, Clone)]
pub(crate) struct PendingClipboardFallback {
    pub image_data: Vec<u8>,
    pub save_config: FileSaveConfig,
    /// Whether to exit after successful fallback save (from exit-after-capture mode).
    pub exit_after_save: bool,
}

/// Pending board deletion confirmation state.
#[derive(Debug, Clone)]
pub(crate) struct PendingBoardDelete {
    pub board_id: String,
    pub expires_at: Instant,
}

/// State for status bar change highlight animation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct StatusChangeHighlight {
    pub started: Instant,
}
