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
use crate::config::ToolPresetConfig;
use crate::domain::{Action, OnboardingTip};
use crate::draw::frame::ShapeSnapshot;
use crate::draw::{Color, Shape, ShapeId};
use crate::input::tool::Tool;
use crate::util::Rect;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use std::{ops::Range, sync::Arc};

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
        /// Caret position as a byte offset into `buffer` (always on a UTF-8
        /// char boundary). Insertions, deletions, and navigation act here
        /// rather than only at the end.
        caret: usize,
        /// Selection anchor as a byte offset into `buffer`. The selected span
        /// is `min(anchor, caret)..max(anchor, caret)`; `None` means no
        /// selection (a plain caret).
        selection_anchor: Option<usize>,
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
    /// Drag the bend handle of a selected curved arrow.
    BendingArrow {
        /// Arrow whose arc is being reshaped.
        shape_id: ShapeId,
        /// Snapshot before the drag, for one undo entry and for Escape.
        snapshot: ShapeSnapshot,
    },
    /// Drag the on-canvas magnification knob of a selected Spotlight.
    AdjustingSpotlightMagnification {
        /// Spotlight whose factor is being dragged.
        shape_id: ShapeId,
        /// Snapshot before the drag, for one undo entry and for Escape.
        snapshot: ShapeSnapshot,
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

impl DrawingState {
    /// Build a [`DrawingState::TextInput`] with the caret at the end of
    /// `buffer` and no selection — the common entry point for both new text and
    /// resuming an edit of existing text.
    pub fn text_input(x: i32, y: i32, buffer: String) -> Self {
        let caret = buffer.len();
        DrawingState::TextInput {
            x,
            y,
            buffer,
            caret,
            selection_anchor: None,
        }
    }
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

/// Clipboard publication requested by the text editor. A cut carries the
/// exact selection that may be deleted after publication succeeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextClipboardRequest {
    pub(crate) text: String,
    pub(crate) cut: Option<TextCutTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextCutTarget {
    pub(crate) generation: u64,
    pub(crate) revision: u64,
    pub(crate) range: Range<usize>,
}

/// Exact editor location owned by an asynchronous Ctrl+V request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextPasteTarget {
    pub(crate) generation: u64,
    pub(crate) revision: u64,
    pub(crate) caret: usize,
    pub(crate) selection_anchor: Option<usize>,
}

/// Buffer edit produced when a clipboard completion is applied. Queued paste
/// targets use this to follow earlier requests without following unrelated
/// caret movement or buffer edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextPasteEdit {
    pub(crate) generation: u64,
    pub(crate) previous_revision: u64,
    pub(crate) revision: u64,
    pub(crate) replaced: Range<usize>,
    pub(crate) inserted_len: usize,
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

/// An accepted quick-color recolor awaiting the backend's config write. The
/// runtime palette is already updated; this carries what `config.toml` still
/// needs (`drawing.quick_colors[index].color`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuickColorEdit {
    pub index: usize,
    pub color: Color,
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

/// Command that can be triggered by an explicit toast action chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastCommand {
    Dispatch(Action),
    AcknowledgeTip {
        tip: OnboardingTip,
        then: Option<Action>,
    },
}

/// Labeled command rendered as a toast action chip.
#[derive(Debug, Clone)]
pub struct ToastAction {
    pub label: String,
    pub(crate) command: ToastCommand,
}

impl ToastAction {
    pub(crate) fn dispatch_action(&self) -> Option<Action> {
        match self.command {
            ToastCommand::Dispatch(action) => Some(action),
            ToastCommand::AcknowledgeTip { .. } => None,
        }
    }
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
    /// Optional second action. When present, only the individual action chips
    /// dispatch; clicking the rest of the toast dismisses it without acting.
    pub secondary_action: Option<ToastAction>,
    /// Queue priority this toast was pushed with (drives preemption).
    pub priority: super::toast_queue::ToastPriority,
    /// Dedup/rate-limit key this toast was pushed with.
    pub key: &'static str,
    /// Monotonic identity for this exact visible activation. Press/release
    /// handling uses it so a queue promotion or same-key update cannot retarget
    /// a pending click to different toast content.
    pub activation_id: u64,
}

/// Identity captured when an input press begins inside the visible toast.
///
/// The field stays opaque outside input state: callers can only return the
/// token on release, where it is matched against the still-active toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToastPress {
    activation_id: u64,
    target: ToastTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToastTarget {
    Body,
    Action(usize),
}

impl ToastPress {
    pub(crate) fn new(activation_id: u64, target: usize) -> Self {
        Self {
            activation_id,
            target: ToastTarget::Action(target),
        }
    }

    pub(crate) fn body(activation_id: u64) -> Self {
        Self {
            activation_id,
            target: ToastTarget::Body,
        }
    }

    pub(crate) fn matches(self, toast: &UiToastState) -> bool {
        self.activation_id == toast.activation_id
    }

    pub(crate) fn matches_target(self, target: Option<usize>) -> bool {
        self.target == target.map(ToastTarget::Action).unwrap_or(ToastTarget::Body)
    }

    pub(crate) fn action_index(self) -> Option<usize> {
        match self.target {
            ToastTarget::Body => None,
            ToastTarget::Action(index) => Some(index),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TextClickState {
    pub shape_id: ShapeId,
    pub x: i32,
    pub y: i32,
    pub at: Instant,
}

/// Tracks an in-progress Alt+left-drag that repositions the active text block.
/// Stores the grab offset (pointer minus block origin) so the block follows the
/// cursor exactly under the grabbed point rather than snapping its origin there.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TextBlockDrag {
    pub grab_dx: i32,
    pub grab_dy: i32,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompositorCapabilities {
    pub layer_shell: bool,
    pub screencopy: bool,
    pub image_copy_capture: bool,
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
    pub fn direct_capture_available(&self) -> bool {
        self.screencopy || self.image_copy_capture
    }

    pub fn all_available(&self) -> bool {
        self.layer_shell && self.direct_capture_available() && self.pointer_constraints
    }

    pub fn limitations_summary(&self) -> Option<String> {
        let mut issues = Vec::new();
        if !self.layer_shell {
            issues.push("Toolbars limited, light passthrough unavailable");
        }
        if !self.freeze_capture {
            issues.push("Freeze unavailable");
        } else if !self.direct_capture_available() {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingBackendAction {
    Screenshot(Action),
    MeasureMode,
    CanvasExport(Action),
    BoardPdfExport(Action),
    DesktopOpen(crate::desktop_open::DesktopOpenRequest),
    ClearSavedToolState,
}

/// Durable toolbar chrome changes awaiting their runtime-ui.toml write.
///
/// Deliberately not part of [`PendingBackendAction`]: that slot has
/// last-action semantics, so sharing it would let a screenshot (or a second
/// toolbar change) silently cost an earlier change its persistence — or vice
/// versa. These are ordered, coalesced per kind, and drained once more at
/// teardown, so a change made in the same input batch as an exit request
/// still reaches the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingToolbarPersistence {
    /// The top-display preference changed by its keyboard cycle (F2). The
    /// cycle already applied, so the payload carries the pre-cycle mode for
    /// the runtime-UI preview's rollback; toolbar-event paths persist via
    /// their exact event-policy target instead.
    DisplayMode {
        previous: crate::config::TopDisplayMode,
    },
    /// The pin flag driven by the keyboard visibility toggle (F9). The
    /// toggle already applied, so the payload carries the pre-change pin for
    /// the runtime-UI preview's rollback; the pin button persists via its
    /// exact event-policy target instead.
    Visibility {
        previous_top_pinned: bool,
    },
    /// One chrome preference flipped by a direct action -- a keybinding, a
    /// command-palette entry, a menu command. Each kind is its own variant so
    /// the queue's per-discriminant coalescing keeps them apart; a shared
    /// variant would let the first toggle in a burst swallow the rest.
    ///
    /// The action applies before it queues, so the payload carries the
    /// pre-change value as the runtime-UI preview's rollback. Only the action
    /// handlers queue these: a mode transition that moves the same field --
    /// focus, light, or presenter mode taking chrome over and giving it back
    /// -- is not the user choosing, and never reaches here.
    StatusBar {
        previous: bool,
    },
    FloatingBadge {
        previous: bool,
    },
    ZoomChip {
        previous: bool,
    },
    InputHud {
        previous: bool,
    },
    /// The click highlight and its tool ring, which move together.
    ClickHighlight {
        previous_enabled: bool,
        previous_tool_ring: bool,
    },
}

/// What a shortcut edit should do to one action's binding list.
///
/// `Replace` carries the chord the capture modal read; `Reset` resolves against
/// the compiled defaults at apply time rather than storing them here, so the
/// request stays a description of intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeybindingEditOperation {
    Replace(Vec<String>),
    Delete,
    Reset,
}

/// One action's shortcut edit, for the running keymap and for `config.toml`.
///
/// The backend writes just this action's `[keybindings]` entry through the
/// narrow editor in `src/config/io.rs` before installing the new keymap, so a
/// chord the file has since given to another action is refused outright. Any
/// other save failure degrades to a this-run edit whose toast says the file did
/// not get it.
///
/// These queue rather than replace one another. Each is a separate write with
/// its own answer and its own toast, so a request the user made cannot be
/// dropped by the next one arriving before the backend drains — which is why
/// they ride `InputState::pending_keybinding_edits` and not the single-slot
/// [`PendingBackendAction`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingEditRequest {
    pub action: Action,
    pub operation: KeybindingEditOperation,
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
    pub used_board_picker: bool,
    /// A user-facing zoom control was activated.
    pub used_zoom_control: bool,
    pub used_canvas_popover: bool,
    /// A drawing color was applied (any path). Drives the colors/thickness
    /// first-run teaching step.
    pub used_color_change: bool,
    /// Stroke thickness / eraser size was adjusted (any path). Drives the
    /// colors/thickness first-run teaching step.
    pub used_thickness_change: bool,
    /// The last shortcut-bound action invoked via a "slow path" this tick —
    /// the command palette or the toolbar — that the shortcut coach nudges
    /// away from. Source-agnostic: the coach resolves and names this action's
    /// keyboard shortcut regardless of which surface triggered it.
    pub shortcut_slow_path_action: Option<Action>,
    /// Number of shortcut-bound slow-path invocations this tick (palette or
    /// toolbar). Folded into the coach's per-session slow-path streak.
    pub shortcut_slow_path_repeats: u32,
}

impl PendingOnboardingUsage {
    /// Record that a shortcut-bound action ran via a slow path (command palette
    /// or toolbar) when the user could have pressed the key. Called only when
    /// the action has a resolvable shortcut so the coach can always name it.
    pub(crate) fn note_shortcut_slow_path(&mut self, action: Action) {
        self.shortcut_slow_path_action = Some(action);
        self.shortcut_slow_path_repeats = self.shortcut_slow_path_repeats.saturating_add(1);
    }
}

#[cfg(test)]
mod tests;
