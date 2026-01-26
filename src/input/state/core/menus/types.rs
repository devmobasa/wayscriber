use crate::draw::ShapeId;

/// Distinguishes between canvas-level and shape-level context menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuKind {
    Shape,
    Canvas,
    Pages,
}

/// Tracks the context menu lifecycle.
#[derive(Debug, Clone)]
pub enum ContextMenuState {
    Hidden,
    Open {
        anchor: (i32, i32),
        shape_ids: Vec<ShapeId>,
        kind: ContextMenuKind,
        hover_index: Option<usize>,
        keyboard_focus: Option<usize>,
        hovered_shape_id: Option<ShapeId>,
    },
}

/// Commands triggered by context menu selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    Delete,
    Duplicate,
    SelectHoveredShape,
    MoveToFront,
    MoveToBack,
    Lock,
    Unlock,
    Properties,
    EditText,
    ClearAll,
    ToggleHighlightTool,
    OpenPagesMenu,
    PagePrev,
    PageNext,
    PageNew,
    PageDuplicate,
    PageDelete,
    SwitchToWhiteboard,
    SwitchToBlackboard,
    ReturnToTransparent,
    ToggleHelp,
    OpenConfigFile,
}

/// Lightweight descriptor for rendering context menu entries.
#[derive(Debug, Clone)]
pub struct ContextMenuEntry {
    pub label: String,
    pub shortcut: Option<String>,
    pub has_submenu: bool,
    pub disabled: bool,
    pub command: Option<MenuCommand>,
}

impl ContextMenuEntry {
    pub fn new(
        label: impl Into<String>,
        shortcut: Option<impl Into<String>>,
        has_submenu: bool,
        disabled: bool,
        command: Option<MenuCommand>,
    ) -> Self {
        Self {
            label: label.into(),
            shortcut: shortcut.map(|s| s.into()),
            has_submenu,
            disabled,
            command,
        }
    }
}

/// Layout metadata for rendering and hit-testing the context menu.
#[derive(Debug, Clone, Copy)]
pub struct ContextMenuLayout {
    pub origin_x: f64,
    pub origin_y: f64,
    pub width: f64,
    pub height: f64,
    pub row_height: f64,
    pub font_size: f64,
    pub padding_x: f64,
    pub padding_y: f64,
    pub shortcut_width: f64,
    pub arrow_width: f64,
}

/// Cursor hint for different regions of the context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuCursorHint {
    /// Default arrow cursor.
    Default,
    /// Pointer/hand cursor for clickable menu items.
    Pointer,
}
