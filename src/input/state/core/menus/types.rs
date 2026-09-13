use crate::draw::ShapeId;

/// Distinguishes between canvas-level and shape-level context menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuKind {
    Shape,
    Canvas,
    Zoom,
    Pages,
    Boards,
    Page,
    PageMove,
    /// Actions for one board row in the board picker.
    Board,
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
        /// A submenu cascading from one of this menu's rows.
        submenu: Option<ContextSubmenu>,
    },
}

/// A submenu open beside the parent row that opened it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextSubmenu {
    pub(crate) kind: ContextMenuKind,
    /// The parent menu row this submenu belongs to.
    pub(crate) parent_index: usize,
    pub(crate) hover_index: Option<usize>,
    pub(crate) keyboard_focus: Option<usize>,
}

/// Which open menu an operation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextMenuLevel {
    Root,
    Submenu,
}

/// The side of the parent menu a submenu opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubmenuSide {
    #[default]
    Right,
    Left,
}

/// Commands triggered by context menu selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuCommand {
    Copy,
    Paste,
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
    ResetCanvasPosition,
    OpenZoomMenu,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    ToggleHighlightTool,
    OpenPagesMenu,
    OpenPageMoveMenu,
    PagePrev,
    PageNext,
    PageNew,
    PageDuplicate,
    PageDelete,
    PageRename,
    PageDuplicateFromContext,
    PageDeleteFromContext,
    PageMoveToBoard { id: String },
    SwitchToPage(usize),
    OpenBoardsMenu,
    OpenBoardPicker,
    BoardPrev,
    BoardNext,
    BoardNew,
    BoardDuplicate,
    BoardDelete,
    BoardEditPaper,
    BoardEditPaperFromContext,
    BoardRenameFromContext,
    BoardTogglePinFromContext,
    SwitchToBoard { id: String },
    SwitchToWhiteboard,
    SwitchToBlackboard,
    ReturnToTransparent,
    OpenRadialMenu,
    ToggleHelp,
    ShowToolbar,
    ShowStatusBar,
    OpenCommandPalette,
    OpenConfigFile,
}

/// Lightweight descriptor for rendering context menu entries.
#[derive(Debug, Clone)]
pub struct ContextMenuEntry {
    pub label: String,
    /// Text in the right-hand column: a shortcut, or a parent row's summary
    /// of its submenu's current state.
    pub shortcut: Option<String>,
    /// The menu this row opens beside itself instead of running a command.
    pub submenu: Option<ContextMenuKind>,
    pub disabled: bool,
    pub command: Option<MenuCommand>,
}

impl ContextMenuEntry {
    pub fn new(
        label: impl Into<String>,
        shortcut: Option<impl Into<String>>,
        disabled: bool,
        command: Option<MenuCommand>,
    ) -> Self {
        Self {
            label: label.into(),
            shortcut: shortcut.map(|s| s.into()),
            submenu: None,
            disabled,
            command,
        }
    }

    /// Makes this a parent row that opens `kind` as a submenu.
    pub fn with_submenu(mut self, kind: ContextMenuKind) -> Self {
        self.submenu = Some(kind);
        self
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
