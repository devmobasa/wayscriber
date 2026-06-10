//! Drawing state machine and input state management.

pub const MIN_STROKE_THICKNESS: f64 = 1.0;
pub const MAX_STROKE_THICKNESS: f64 = 50.0;
pub const PRESET_FEEDBACK_DURATION_MS: u64 = 450;
pub const PRESET_TOAST_DURATION_MS: u64 = 1300;
pub const UI_TOAST_DURATION_MS: u64 = 5000;
pub const BOARD_DELETE_CONFIRM_MS: u64 = 7000;
pub const BLOCKED_ACTION_DURATION_MS: u64 = 200;
pub const BOARD_UNDO_EXPIRE_MS: u64 = 30_000;
pub const PAGE_DELETE_CONFIRM_MS: u64 = 5000;
pub const PAGE_UNDO_EXPIRE_MS: u64 = 30_000;
#[allow(dead_code)]
pub const STATUS_CHANGE_HIGHLIGHT_MS: u64 = 300;

use crate::capture::{ImageOperationKind, file::FileSaveConfig};
use crate::config::{Action, ToolPresetConfig};
use crate::draw::frame::ShapeSnapshot;
use crate::draw::{Color, Shape, ShapeId};
use crate::input::tool::Tool;
use crate::util::Rect;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// Current drawing mode state machine.
///
/// Tracks whether the user is idle, actively drawing a shape, or entering text.
/// State transitions occur based on mouse and keyboard events.
#[derive(Debug, Clone)]
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
        /// Accumulated thickness values for freehand drawing (pressure sensitivity)
        point_thicknesses: Vec<f32>,
    },
    /// Click-to-add freeform polygon construction.
    BuildingPolygon {
        /// Committed polygon vertices.
        points: Vec<(i32, i32)>,
        /// Current pointer location used for the preview edge.
        preview: Option<(i32, i32)>,
        /// Fill setting frozen at the first click.
        fill: bool,
        /// Color frozen at the first click.
        color: Color,
        /// Stroke thickness frozen at the first click.
        thick: f64,
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
    /// Resize selection by dragging a handle
    ResizingSelection {
        /// Which handle is being dragged
        handle: SelectionHandle,
        /// Original bounding box of selection
        original_bounds: crate::util::Rect,
        /// Starting mouse position
        start_x: i32,
        start_y: i32,
        /// Snapshots of shapes prior to resizing (for undo/cancel)
        snapshots: Arc<Vec<(ShapeId, ShapeSnapshot)>>,
    },
}

/// Which selection handle is being interacted with
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Top,
    Bottom,
    Left,
    Right,
}

/// Describes which kind of text input is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputMode {
    Plain,
    StickyNote,
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PressureThicknessEditMode {
    #[default]
    Disabled,
    Add,
    Scale,
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PressureThicknessEntryMode {
    Never,
    #[default]
    PressureOnly,
    AnyPressure,
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
pub enum OutputFocusAction {
    Next,
    Prev,
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
        preset: Box<ToolPresetConfig>,
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct BoardPickerClickState {
    pub row: usize,
    pub x: i32,
    pub y: i32,
    pub at: Instant,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PolygonClickState {
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
    pub freeze_capture: bool,
    pub pointer_constraints: bool,
    pub desktop_environment: DesktopEnvironment,
    pub shell_mode: ShellMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DesktopEnvironment {
    Gnome,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShellMode {
    LayerShell,
    XdgFallback,
    #[default]
    Unknown,
}

impl CompositorCapabilities {
    pub fn all_available(&self) -> bool {
        self.layer_shell && self.screencopy && self.pointer_constraints
    }

    pub fn limitations_summary(&self) -> Option<String> {
        let mut issues = Vec::new();
        if !self.layer_shell {
            issues.push("Toolbars limited, light passthrough unavailable");
        }
        if !self.freeze_capture {
            issues.push("Freeze unavailable");
        } else if !self.screencopy {
            issues.push("Freeze uses portal capture");
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
    pub operation: ImageOperationKind,
    /// Whether to exit after successful fallback save (from exit-after-capture mode).
    pub exit_after_save: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingBackendAction {
    Screenshot(Action),
    CanvasExport(Action),
    BoardPdfExport(Action),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WayscriberClipboardSelection {
    pub schema_version: u32,
    pub app_version: String,
    pub app_instance_id: String,
    pub copy_generation: u64,
    pub shapes: Vec<Shape>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClipboardFingerprint {
    pub offered_mime_types: Vec<String>,
    pub selected_mime_type: Option<String>,
    pub bounded_content_hash: Option<u64>,
    pub bounded_content_len: Option<usize>,
    pub bounded_content_truncated: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum SelectionPublishState {
    #[default]
    NotAttempted,
    Published {
        generation: u64,
    },
    Failed {
        generation: u64,
        clipboard_fingerprint_at_failure: Option<ClipboardFingerprint>,
    },
    Superseded {
        generation: u64,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct PendingSelectionClipboardPublish {
    pub generation: u64,
    pub payload_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PasteAnchor {
    Pointer { x: i32, y: i32 },
    VisibleCenter { x: i32, y: i32 },
}

impl PasteAnchor {
    #[allow(dead_code)]
    pub(crate) fn point(self) -> (i32, i32) {
        match self {
            PasteAnchor::Pointer { x, y } | PasteAnchor::VisibleCenter { x, y } => (x, y),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClipboardPasteRequest {
    pub id: u64,
    pub target_board_id: String,
    pub target_page_index: usize,
    pub target_page_generation: u64,
    pub anchor: PasteAnchor,
    pub visible_canvas_rect: Rect,
    pub screen_size: (u32, u32),
    pub selection_clipboard_generation_at_request: u64,
    pub local_selection_fallback_generation: Option<u64>,
}

/// Pending board deletion confirmation state.
#[derive(Debug, Clone)]
pub(crate) struct PendingBoardDelete {
    pub confirmation: crate::input::boards::BoardDeleteConfirmation,
    pub expires_at: Instant,
}

/// Pending page deletion confirmation state.
#[derive(Debug, Clone)]
pub(crate) struct PendingPageDelete {
    pub confirmation: crate::input::boards::PageDeleteConfirmation,
    pub expires_at: Instant,
}

/// State for status bar change highlight animation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct StatusChangeHighlight {
    pub started: Instant,
}

/// Duration for text edit entry animation in milliseconds.
pub const TEXT_EDIT_ENTRY_DURATION_MS: u64 = 200;

/// State for text edit entry animation (teal glow pulse).
#[derive(Debug, Clone)]
pub(crate) struct TextEditEntryFeedback {
    pub started: Instant,
}

/// Pending first-run onboarding usage signals emitted by input handlers.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PendingOnboardingUsage {
    pub first_stroke_done: bool,
    pub first_undo_done: bool,
    pub used_toolbar_toggle: bool,
    pub used_radial_menu: bool,
    pub used_context_menu_right_click: bool,
    pub used_context_menu_keyboard: bool,
    pub used_help_overlay: bool,
    pub used_command_palette: bool,
}

#[cfg(test)]
mod tests {
    use super::CompositorCapabilities;

    #[test]
    fn compositor_capabilities_limitations_summary_returns_none_when_fully_available() {
        assert_eq!(
            CompositorCapabilities {
                layer_shell: true,
                screencopy: true,
                freeze_capture: true,
                pointer_constraints: true,
                desktop_environment: Default::default(),
                shell_mode: Default::default(),
            }
            .limitations_summary(),
            None
        );
    }

    #[test]
    fn compositor_capabilities_limitations_summary_lists_missing_features_in_order() {
        assert_eq!(
            CompositorCapabilities {
                layer_shell: false,
                screencopy: true,
                freeze_capture: true,
                pointer_constraints: false,
                desktop_environment: Default::default(),
                shell_mode: Default::default(),
            }
            .limitations_summary(),
            Some(
                "Toolbars limited, light passthrough unavailable, Pointer lock unavailable"
                    .to_string()
            )
        );
    }

    #[test]
    fn compositor_capabilities_reports_portal_freeze_without_hiding_limitations() {
        let caps = CompositorCapabilities {
            layer_shell: true,
            screencopy: false,
            freeze_capture: true,
            pointer_constraints: true,
            desktop_environment: Default::default(),
            shell_mode: Default::default(),
        };

        assert!(!caps.all_available());
        assert_eq!(
            caps.limitations_summary(),
            Some("Freeze uses portal capture".to_string())
        );
    }
}
