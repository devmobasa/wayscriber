//! Drawing state machine and input state management.

use super::highlight::{ClickHighlightSettings, ClickHighlightState};
use crate::config::{Action, BoardConfig, KeyBinding};
use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::draw::{
    CanvasSet, Color, DirtyTracker, FontDescriptor, Frame, Shape, ShapeId,
    shape::{
        bounding_box_for_arrow, bounding_box_for_ellipse, bounding_box_for_line,
        bounding_box_for_points, bounding_box_for_rect, bounding_box_for_text,
    },
};
use crate::input::{board_mode::BoardMode, hit_test, modifiers::Modifiers, tool::Tool};
use crate::legacy;
use crate::util::{self, Rect};
use cairo::Context as CairoContext;
use chrono::{Local, Utc};
use std::collections::{HashMap, HashSet};
use std::process::{Command, Stdio};
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
}

/// Tracks current selection state.
#[derive(Debug, Clone)]
pub enum SelectionState {
    None,
    Active { shape_ids: Vec<ShapeId> },
}

/// Distinguishes between canvas-level and shape-level context menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuKind {
    Shape,
    Canvas,
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
    },
}

/// Commands triggered by context menu selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    Delete,
    Duplicate,
    MoveToFront,
    MoveToBack,
    Lock,
    Unlock,
    Properties,
    EditText,
    ClearAll,
    ToggleHighlightTool,
    ToggleClickHighlight,
    SwitchToWhiteboard,
    SwitchToBlackboard,
    ReturnToTransparent,
    ToggleHelp,
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

const SPATIAL_GRID_CELL_SIZE: i32 = 64;

#[derive(Debug, Clone)]
struct SpatialGrid {
    cell_size: i32,
    cells: HashMap<(i32, i32), Vec<usize>>,
    shape_count: usize,
}

/// Summarizes shape metadata for the on-screen properties panel.
#[derive(Debug, Clone)]
pub struct ShapePropertiesPanel {
    pub title: String,
    pub anchor: (f64, f64),
    pub lines: Vec<String>,
    pub multiple_selection: bool,
}

/// Main input state containing all drawing session state.
///
/// This struct holds the current frame (all drawn shapes), drawing parameters,
/// modifier keys, drawing mode, and UI flags. It processes all keyboard and
/// mouse events to update the drawing state and determine when redraws are needed.
pub struct InputState {
    /// Multi-frame canvas management (transparent, whiteboard, blackboard)
    pub canvas_set: CanvasSet,
    /// Current drawing color (changed with color keys: R, G, B, etc.)
    pub current_color: Color,
    /// Current pen/line thickness in pixels (changed with +/- keys)
    pub current_thickness: f64,
    /// Current font size for text mode (from config)
    pub current_font_size: f64,
    /// Font descriptor for text rendering (family, weight, style)
    pub font_descriptor: FontDescriptor,
    /// Whether to draw background behind text
    pub text_background_enabled: bool,
    /// Arrowhead length in pixels (from config)
    pub arrow_length: f64,
    /// Arrowhead angle in degrees (from config)
    pub arrow_angle: f64,
    /// Current modifier key state
    pub modifiers: Modifiers,
    /// Current drawing mode state machine
    pub state: DrawingState,
    /// Whether user requested to exit the overlay
    pub should_exit: bool,
    /// Whether the display needs to be redrawn
    pub needs_redraw: bool,
    /// Whether the help overlay is currently visible (toggled with F10)
    pub show_help: bool,
    /// Whether the status bar is currently visible (toggled via keybinding)
    pub show_status_bar: bool,
    /// Screen width in pixels (set by backend after configuration)
    pub screen_width: u32,
    /// Screen height in pixels (set by backend after configuration)
    pub screen_height: u32,
    /// Previous color before entering board mode (for restoration)
    pub board_previous_color: Option<Color>,
    /// Board mode configuration
    pub board_config: BoardConfig,
    /// Tracks dirty regions between renders
    pub(crate) dirty_tracker: DirtyTracker,
    /// Cached bounds for the current provisional shape (if any)
    pub(crate) last_provisional_bounds: Option<Rect>,
    /// Cached bounds for live text preview/caret (if any)
    pub(crate) last_text_preview_bounds: Option<Rect>,
    /// Keybinding action map for efficient lookup
    action_map: HashMap<KeyBinding, Action>,
    /// Pending capture action (to be handled by WaylandState)
    pending_capture_action: Option<Action>,
    /// Maximum number of shapes allowed per frame (0 = unlimited)
    pub max_shapes_per_frame: usize,
    /// Click highlight animation state
    pub(crate) click_highlight: ClickHighlightState,
    /// Optional tool override independent of modifier keys
    tool_override: Option<Tool>,
    /// Current selection information
    pub selection_state: SelectionState,
    /// Current context menu state
    pub context_menu_state: ContextMenuState,
    /// Whether context menu interactions are enabled
    context_menu_enabled: bool,
    /// Cached hit-test bounds per shape id
    hit_test_cache: HashMap<ShapeId, Rect>,
    /// Hit test tolerance in pixels
    pub hit_test_tolerance: f64,
    /// Threshold before enabling spatial indexing
    pub max_linear_hit_test: usize,
    /// Maximum number of undo actions retained in history
    pub undo_stack_limit: usize,
    /// Cached layout details for the currently open context menu
    pub context_menu_layout: Option<ContextMenuLayout>,
    /// Optional spatial index for accelerating hit-testing when many shapes are present
    spatial_index: Option<SpatialGrid>,
    /// Last known pointer position (for keyboard anchors and hover refresh)
    last_pointer_position: (i32, i32),
    /// Recompute hover next time layout is available
    pending_menu_hover_recalc: bool,
    /// Optional properties panel describing the current selection
    shape_properties_panel: Option<ShapePropertiesPanel>,
}

impl InputState {
    /// Creates a new InputState with specified defaults.
    ///
    /// Screen dimensions default to 0 and should be updated by the backend
    /// after surface configuration (see `update_screen_dimensions`).
    ///
    /// # Arguments
    /// * `color` - Initial drawing color
    /// * `thickness` - Initial pen thickness in pixels
    /// * `font_size` - Font size for text mode in points
    /// * `font_descriptor` - Font configuration for text rendering
    /// * `text_background_enabled` - Whether to draw background behind text
    /// * `arrow_length` - Arrowhead length in pixels
    /// * `arrow_angle` - Arrowhead angle in degrees
    /// * `show_status_bar` - Whether the status bar starts visible
    /// * `board_config` - Board mode configuration
    /// * `action_map` - Keybinding action map
    #[allow(clippy::too_many_arguments)]
    pub fn with_defaults(
        color: Color,
        thickness: f64,
        font_size: f64,
        font_descriptor: FontDescriptor,
        text_background_enabled: bool,
        arrow_length: f64,
        arrow_angle: f64,
        show_status_bar: bool,
        board_config: BoardConfig,
        action_map: HashMap<KeyBinding, Action>,
        max_shapes_per_frame: usize,
        click_highlight_settings: ClickHighlightSettings,
    ) -> Self {
        let mut state = Self {
            canvas_set: CanvasSet::new(),
            current_color: color,
            current_thickness: thickness,
            current_font_size: font_size,
            font_descriptor,
            text_background_enabled,
            arrow_length,
            arrow_angle,
            modifiers: Modifiers::new(),
            state: DrawingState::Idle,
            should_exit: false,
            needs_redraw: true,
            show_help: false,
            show_status_bar,
            screen_width: 0,
            screen_height: 0,
            board_previous_color: None,
            board_config,
            dirty_tracker: DirtyTracker::new(),
            last_provisional_bounds: None,
            last_text_preview_bounds: None,
            action_map,
            pending_capture_action: None,
            max_shapes_per_frame,
            click_highlight: ClickHighlightState::new(click_highlight_settings),
            tool_override: None,
            selection_state: SelectionState::None,
            context_menu_state: ContextMenuState::Hidden,
            context_menu_enabled: true,
            hit_test_cache: HashMap::new(),
            hit_test_tolerance: 6.0,
            max_linear_hit_test: 400,
            undo_stack_limit: 100,
            context_menu_layout: None,
            spatial_index: None,
            last_pointer_position: (0, 0),
            pending_menu_hover_recalc: false,
            shape_properties_panel: None,
        };

        if state.click_highlight.uses_pen_color() {
            state.sync_highlight_color();
        }

        state
    }

    /// Returns ids of the currently selected shapes.
    pub fn selected_shape_ids(&self) -> &[ShapeId] {
        match &self.selection_state {
            SelectionState::Active { shape_ids } => shape_ids,
            _ => &[],
        }
    }

    /// Updates the cached pointer location.
    pub fn update_pointer_position(&mut self, x: i32, y: i32) {
        self.last_pointer_position = (x, y);
    }

    /// Returns true if any shapes are selected.
    pub fn has_selection(&self) -> bool {
        matches!(self.selection_state, SelectionState::Active { .. })
    }

    /// Returns an active properties panel description, if any.
    pub fn properties_panel(&self) -> Option<&ShapePropertiesPanel> {
        self.shape_properties_panel.as_ref()
    }

    /// Hides the properties panel if visible.
    pub fn close_properties_panel(&mut self) {
        if self.shape_properties_panel.take().is_some() {
            self.needs_redraw = true;
        }
    }

    fn set_properties_panel(&mut self, panel: ShapePropertiesPanel) {
        self.shape_properties_panel = Some(panel);
        self.needs_redraw = true;
    }

    /// Enables or disables context menu interactions.
    pub fn set_context_menu_enabled(&mut self, enabled: bool) {
        self.context_menu_enabled = enabled;
        if !enabled && self.is_context_menu_open() {
            self.close_context_menu();
        }
    }

    /// Returns true if context menus are currently permitted.
    pub fn context_menu_enabled(&self) -> bool {
        self.context_menu_enabled
    }

    /// Clears any active selection state.
    pub fn clear_selection(&mut self) {
        self.selection_state = SelectionState::None;
        self.close_properties_panel();
    }

    /// Replaces the selection with the provided ids.
    pub fn set_selection(&mut self, ids: Vec<ShapeId>) {
        if ids.is_empty() {
            self.selection_state = SelectionState::None;
            self.close_properties_panel();
            return;
        }

        let mut seen = HashSet::new();
        let mut ordered = Vec::new();
        for id in ids {
            if seen.insert(id) {
                ordered.push(id);
            }
        }
        self.selection_state = SelectionState::Active { shape_ids: ordered };
        self.close_properties_panel();
    }

    /// Extends the current selection with additional ids.
    pub fn extend_selection<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = ShapeId>,
    {
        match &mut self.selection_state {
            SelectionState::Active { shape_ids } => {
                let mut seen: HashSet<ShapeId> = shape_ids.iter().copied().collect();
                for id in iter {
                    if seen.insert(id) {
                        shape_ids.push(id);
                    }
                }
                self.close_properties_panel();
            }
            _ => {
                let ids: Vec<ShapeId> = iter.into_iter().collect();
                self.set_selection(ids);
            }
        }
    }

    /// Opens a context menu at the given anchor for the provided kind.
    pub fn open_context_menu(
        &mut self,
        anchor: (i32, i32),
        shape_ids: Vec<ShapeId>,
        kind: ContextMenuKind,
    ) {
        if !self.context_menu_enabled {
            return;
        }
        self.close_properties_panel();
        if let Some(layout) = self.context_menu_layout.take() {
            self.mark_context_menu_region(layout);
        }
        self.context_menu_state = ContextMenuState::Open {
            anchor,
            shape_ids,
            kind,
            hover_index: None,
            keyboard_focus: None,
        };
        self.pending_menu_hover_recalc = true;
    }

    pub fn toggle_context_menu_via_keyboard(&mut self) {
        if !self.context_menu_enabled {
            return;
        }
        if self.is_context_menu_open() {
            self.close_context_menu();
            return;
        }

        let selection = self.selected_shape_ids().to_vec();
        if selection.is_empty() {
            let anchor = self.keyboard_canvas_menu_anchor();
            self.update_pointer_position(anchor.0, anchor.1);
            self.open_context_menu(anchor, Vec::new(), ContextMenuKind::Canvas);
            self.pending_menu_hover_recalc = false;
            self.set_context_menu_focus(None);
            self.focus_first_context_menu_entry();
        } else {
            let anchor = self.keyboard_shape_menu_anchor(&selection);
            self.update_pointer_position(anchor.0, anchor.1);
            self.open_context_menu(anchor, selection, ContextMenuKind::Shape);
            self.pending_menu_hover_recalc = false;
            self.focus_first_context_menu_entry();
        }
        self.needs_redraw = true;
    }

    /// Updates the keyboard focus entry for the context menu.
    pub fn set_context_menu_focus(&mut self, focus: Option<usize>) {
        if let ContextMenuState::Open {
            ref mut keyboard_focus,
            ref mut hover_index,
            ..
        } = self.context_menu_state
        {
            let changed = *keyboard_focus != focus;
            *keyboard_focus = focus;
            if focus.is_some() {
                *hover_index = None;
            }
            if changed {
                self.needs_redraw = true;
            }
        }
    }

    /// Closes the currently open context menu.
    pub fn close_context_menu(&mut self) {
        if let Some(layout) = self.context_menu_layout.take() {
            self.mark_context_menu_region(layout);
        }
        self.context_menu_state = ContextMenuState::Hidden;
        self.pending_menu_hover_recalc = false;
        self.needs_redraw = true;
    }

    /// Returns true if a context menu is currently visible.
    pub fn is_context_menu_open(&self) -> bool {
        matches!(self.context_menu_state, ContextMenuState::Open { .. })
    }

    /// Clears cached hit-test bounds.
    pub fn invalidate_hit_cache(&mut self) {
        self.hit_test_cache.clear();
        self.spatial_index = None;
    }

    /// Removes cached hit-test data for a single shape.
    pub fn invalidate_hit_cache_for(&mut self, id: ShapeId) {
        self.hit_test_cache.remove(&id);
        self.spatial_index = None;
    }

    /// Updates the hit-test tolerance (in pixels).
    pub fn set_hit_test_tolerance(&mut self, tolerance: f64) {
        self.hit_test_tolerance = tolerance.max(1.0);
        self.invalidate_hit_cache();
    }

    /// Updates the threshold used before building a spatial index.
    pub fn set_hit_test_threshold(&mut self, threshold: usize) {
        self.max_linear_hit_test = threshold.max(1);
    }

    /// Updates the undo stack limit for subsequent actions.
    pub fn set_undo_stack_limit(&mut self, limit: usize) {
        self.undo_stack_limit = limit.max(1);
    }

    fn hit_test_single(&mut self, index: usize, x: i32, y: i32, tolerance: f64) -> Option<ShapeId> {
        let frame = self.canvas_set.active_frame();
        if index >= frame.shapes.len() {
            return None;
        }

        let (shape_id, bounds, hit) = {
            let drawn = &frame.shapes[index];
            let cached = self.hit_test_cache.get(&drawn.id).copied();
            let bounds = cached.or_else(|| hit_test::compute_hit_bounds(drawn, tolerance));
            let hit = bounds
                .as_ref()
                .map(|rect| rect.contains(x, y) && hit_test::hit_test(drawn, (x, y), tolerance))
                .unwrap_or(false);
            (drawn.id, bounds, hit)
        };

        if let Some(bounds) = bounds {
            self.hit_test_cache.entry(shape_id).or_insert(bounds);
            if hit {
                return Some(shape_id);
            }
        }
        None
    }

    fn hit_test_indices<I>(&mut self, indices: I, x: i32, y: i32, tolerance: f64) -> Option<ShapeId>
    where
        I: IntoIterator<Item = usize>,
    {
        for index in indices {
            if let Some(shape_id) = self.hit_test_single(index, x, y, tolerance) {
                return Some(shape_id);
            }
        }
        None
    }

    /// Performs hit-testing against the active frame and returns the top-most shape id.
    pub fn hit_test_at(&mut self, x: i32, y: i32) -> Option<ShapeId> {
        let tolerance = self.hit_test_tolerance;
        let len = self.canvas_set.active_frame().shapes.len();
        let threshold = self.max_linear_hit_test;

        if len > threshold {
            let rebuild = match &self.spatial_index {
                Some(grid) if grid.shape_count == len => false,
                _ => true,
            };

            if rebuild {
                let frame = self.canvas_set.active_frame();
                self.spatial_index = SpatialGrid::build(frame, SPATIAL_GRID_CELL_SIZE);
            }

            if let Some(grid) = &self.spatial_index {
                let candidates = grid.query((x, y));
                if let Some(hit) = self.hit_test_indices(candidates.into_iter(), x, y, tolerance) {
                    return Some(hit);
                }
            }
        } else {
            self.spatial_index = None;
        }

        self.hit_test_indices((0..len).rev(), x, y, tolerance)
    }

    /// Returns the entries to render for the currently open context menu.
    pub fn context_menu_entries(&self) -> Vec<ContextMenuEntry> {
        match &self.context_menu_state {
            ContextMenuState::Hidden => Vec::new(),
            ContextMenuState::Open {
                kind, shape_ids, ..
            } => match kind {
                ContextMenuKind::Canvas => self.canvas_menu_entries(),
                ContextMenuKind::Shape => self.shape_menu_entries(shape_ids),
            },
        }
    }

    fn current_menu_focus_or_hover(&self) -> Option<usize> {
        if let ContextMenuState::Open {
            hover_index,
            keyboard_focus,
            ..
        } = &self.context_menu_state
        {
            hover_index.or(*keyboard_focus)
        } else {
            None
        }
    }

    fn select_edge_context_menu_entry(&mut self, start_front: bool) -> bool {
        if !self.is_context_menu_open() {
            return false;
        }
        let entries = self.context_menu_entries();
        let iter: Box<dyn Iterator<Item = (usize, &ContextMenuEntry)>> = if start_front {
            Box::new(entries.iter().enumerate())
        } else {
            Box::new(entries.iter().enumerate().rev())
        };
        for (index, entry) in iter {
            if !entry.disabled {
                self.set_context_menu_focus(Some(index));
                return true;
            }
        }
        false
    }

    pub(crate) fn focus_next_context_menu_entry(&mut self) -> bool {
        self.advance_context_menu_focus(true)
    }

    pub(crate) fn focus_previous_context_menu_entry(&mut self) -> bool {
        self.advance_context_menu_focus(false)
    }

    fn advance_context_menu_focus(&mut self, forward: bool) -> bool {
        if !self.is_context_menu_open() {
            return false;
        }
        let entries = self.context_menu_entries();
        if entries.is_empty() {
            return false;
        }

        let len = entries.len();
        let mut index = self
            .current_menu_focus_or_hover()
            .unwrap_or_else(|| if forward { len - 1 } else { 0 });

        for _ in 0..len {
            index = if forward {
                (index + 1) % len
            } else {
                (index + len - 1) % len
            };
            if !entries[index].disabled {
                self.set_context_menu_focus(Some(index));
                return true;
            }
        }
        false
    }

    pub(crate) fn focus_first_context_menu_entry(&mut self) -> bool {
        self.select_edge_context_menu_entry(true)
    }

    pub(crate) fn focus_last_context_menu_entry(&mut self) -> bool {
        self.select_edge_context_menu_entry(false)
    }

    pub(crate) fn activate_context_menu_selection(&mut self) -> bool {
        if !self.is_context_menu_open() {
            return false;
        }
        let entries = self.context_menu_entries();
        if entries.is_empty() {
            return false;
        }
        let index = match self.current_menu_focus_or_hover() {
            Some(idx) => idx,
            None => return false,
        };
        if let Some(entry) = entries.get(index) {
            if entry.disabled {
                return false;
            }
            if let Some(command) = entry.command {
                self.execute_menu_command(command);
            } else {
                self.close_context_menu();
            }
            true
        } else {
            false
        }
    }

    fn mark_context_menu_region(&mut self, layout: ContextMenuLayout) {
        let x = layout.origin_x.floor() as i32;
        let y = layout.origin_y.floor() as i32;
        let width = layout.width.ceil() as i32 + 2;
        let height = layout.height.ceil() as i32 + 2;
        let width = width.max(1);
        let height = height.max(1);

        if let Some(rect) = Rect::new(x, y, width, height) {
            self.dirty_tracker.mark_rect(rect);
        } else {
            self.dirty_tracker.mark_full();
        }
    }

    fn canvas_menu_entries(&self) -> Vec<ContextMenuEntry> {
        let mut entries = Vec::new();
        entries.push(ContextMenuEntry::new(
            "Clear All",
            Some("E"),
            false,
            false,
            Some(MenuCommand::ClearAll),
        ));
        entries.push(ContextMenuEntry::new(
            "Toggle Highlight Tool",
            Some("Ctrl+Alt+H"),
            false,
            false,
            Some(MenuCommand::ToggleHighlightTool),
        ));
        entries.push(ContextMenuEntry::new(
            "Toggle Click Highlight",
            Some("Ctrl+Shift+H"),
            false,
            false,
            Some(MenuCommand::ToggleClickHighlight),
        ));

        match self.canvas_set.active_mode() {
            BoardMode::Transparent => {
                entries.push(ContextMenuEntry::new(
                    "Switch to Whiteboard",
                    Some("Ctrl+W"),
                    false,
                    false,
                    Some(MenuCommand::SwitchToWhiteboard),
                ));
                entries.push(ContextMenuEntry::new(
                    "Switch to Blackboard",
                    Some("Ctrl+B"),
                    false,
                    false,
                    Some(MenuCommand::SwitchToBlackboard),
                ));
            }
            BoardMode::Whiteboard => {
                entries.push(ContextMenuEntry::new(
                    "Return to Transparent",
                    Some("Ctrl+Shift+T"),
                    false,
                    false,
                    Some(MenuCommand::ReturnToTransparent),
                ));
                entries.push(ContextMenuEntry::new(
                    "Switch to Blackboard",
                    Some("Ctrl+B"),
                    false,
                    false,
                    Some(MenuCommand::SwitchToBlackboard),
                ));
            }
            BoardMode::Blackboard => {
                entries.push(ContextMenuEntry::new(
                    "Return to Transparent",
                    Some("Ctrl+Shift+T"),
                    false,
                    false,
                    Some(MenuCommand::ReturnToTransparent),
                ));
                entries.push(ContextMenuEntry::new(
                    "Switch to Whiteboard",
                    Some("Ctrl+W"),
                    false,
                    false,
                    Some(MenuCommand::SwitchToWhiteboard),
                ));
            }
        }

        entries.push(ContextMenuEntry::new(
            "Help",
            Some("F10"),
            false,
            false,
            Some(MenuCommand::ToggleHelp),
        ));
        entries
    }

    /// Returns cached context menu layout, if available.
    pub fn context_menu_layout(&self) -> Option<&ContextMenuLayout> {
        self.context_menu_layout.as_ref()
    }

    /// Clears cached layout data (used when menu closes).
    pub fn clear_context_menu_layout(&mut self) {
        self.context_menu_layout = None;
        self.pending_menu_hover_recalc = false;
    }

    /// Recomputes context menu layout for rendering and hit-testing.
    pub fn update_context_menu_layout(
        &mut self,
        ctx: &CairoContext,
        screen_width: u32,
        screen_height: u32,
    ) {
        if !self.is_context_menu_open() {
            self.context_menu_layout = None;
            return;
        }

        let entries = self.context_menu_entries();
        if entries.is_empty() {
            self.context_menu_layout = None;
            return;
        }

        const FONT_SIZE: f64 = 14.0;
        const ROW_HEIGHT: f64 = 24.0;
        const PADDING_X: f64 = 12.0;
        const PADDING_Y: f64 = 8.0;
        const GAP_BETWEEN_COLUMNS: f64 = 20.0;
        const ARROW_WIDTH: f64 = 10.0;

        let _ = ctx.save();
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(FONT_SIZE);

        let mut max_label_width: f64 = 0.0;
        let mut max_shortcut_width: f64 = 0.0;
        for entry in &entries {
            if let Ok(extents) = ctx.text_extents(&entry.label) {
                max_label_width = max_label_width.max(extents.width());
            }
            if let Some(shortcut) = &entry.shortcut {
                if let Ok(extents) = ctx.text_extents(shortcut) {
                    max_shortcut_width = max_shortcut_width.max(extents.width());
                }
            }
        }

        let _ = ctx.restore();

        let menu_width = PADDING_X * 2.0
            + max_label_width
            + GAP_BETWEEN_COLUMNS
            + max_shortcut_width
            + ARROW_WIDTH;
        let menu_height = PADDING_Y * 2.0 + ROW_HEIGHT * entries.len() as f64;

        let mut origin_x = match &self.context_menu_state {
            ContextMenuState::Open { anchor, .. } => anchor.0 as f64,
            ContextMenuState::Hidden => 0.0,
        };
        let mut origin_y = match &self.context_menu_state {
            ContextMenuState::Open { anchor, .. } => anchor.1 as f64,
            ContextMenuState::Hidden => 0.0,
        };

        let screen_w = screen_width as f64;
        let screen_h = screen_height as f64;
        if origin_x + menu_width > screen_w - 6.0 {
            origin_x = (screen_w - menu_width - 6.0).max(6.0);
        }
        if origin_y + menu_height > screen_h - 6.0 {
            origin_y = (screen_h - menu_height - 6.0).max(6.0);
        }

        self.context_menu_layout = Some(ContextMenuLayout {
            origin_x,
            origin_y,
            width: menu_width,
            height: menu_height,
            row_height: ROW_HEIGHT,
            font_size: FONT_SIZE,
            padding_x: PADDING_X,
            padding_y: PADDING_Y,
            shortcut_width: max_shortcut_width,
            arrow_width: ARROW_WIDTH,
        });

        if self.pending_menu_hover_recalc {
            let (px, py) = self.last_pointer_position;
            self.update_context_menu_hover_from_pointer_internal(px, py, false);
            self.pending_menu_hover_recalc = false;
        }

        if let Some(layout) = self.context_menu_layout {
            self.mark_context_menu_region(layout);
        }
    }

    /// Maps pointer coordinates to a context menu entry index, if applicable.
    pub fn context_menu_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.context_menu_layout()?;
        let entries = self.context_menu_entries();
        if entries.is_empty() {
            return None;
        }

        let xf = x as f64;
        let yf = y as f64;
        if xf < layout.origin_x || xf > layout.origin_x + layout.width {
            return None;
        }
        if yf < layout.origin_y + layout.padding_y
            || yf > layout.origin_y + layout.height - layout.padding_y
        {
            return None;
        }

        let rel_y = yf - layout.origin_y - layout.padding_y;
        if rel_y < 0.0 {
            return None;
        }

        let index = (rel_y / layout.row_height).floor() as usize;
        if index < entries.len() {
            Some(index)
        } else {
            None
        }
    }

    fn update_context_menu_hover_from_pointer_internal(
        &mut self,
        x: i32,
        y: i32,
        mark_redraw: bool,
    ) {
        if !self.is_context_menu_open() {
            return;
        }
        let new_hover = self.context_menu_index_at(x, y);
        let mut changed = false;
        if let ContextMenuState::Open {
            ref mut hover_index,
            ..
        } = self.context_menu_state
        {
            if *hover_index != new_hover {
                *hover_index = new_hover;
                changed = true;
            }
        }
        if changed && mark_redraw {
            self.needs_redraw = true;
        }
        if new_hover.is_some() {
            if let ContextMenuState::Open {
                ref mut keyboard_focus,
                ..
            } = self.context_menu_state
            {
                if keyboard_focus.is_some() {
                    *keyboard_focus = None;
                }
            }
        }
    }

    /// Updates hover index based on pointer location when menu is open.
    pub fn update_context_menu_hover_from_pointer(&mut self, x: i32, y: i32) {
        self.update_context_menu_hover_from_pointer_internal(x, y, true);
    }

    /// Executes the supplied menu command, updating state and recording undo where needed.
    pub fn execute_menu_command(&mut self, command: MenuCommand) {
        if !matches!(command, MenuCommand::Properties) {
            self.close_properties_panel();
        }

        let changed = match command {
            MenuCommand::Delete => self.delete_selected_shapes(),
            MenuCommand::Duplicate => self.duplicate_selected_shapes(),
            MenuCommand::MoveToFront => self.move_selection_to_front(),
            MenuCommand::MoveToBack => self.move_selection_to_back(),
            MenuCommand::Lock => self.set_selection_locked(true),
            MenuCommand::Unlock => self.set_selection_locked(false),
            MenuCommand::Properties => self.show_properties_panel(),
            MenuCommand::EditText => {
                self.begin_text_edit_for_selection();
                true
            }
            MenuCommand::ClearAll => self.clear_active_shapes(),
            MenuCommand::ToggleHighlightTool => {
                self.toggle_highlight_tool();
                true
            }
            MenuCommand::ToggleClickHighlight => {
                self.toggle_click_highlight();
                true
            }
            MenuCommand::SwitchToWhiteboard => {
                self.switch_board_mode(BoardMode::Whiteboard);
                true
            }
            MenuCommand::SwitchToBlackboard => {
                self.switch_board_mode(BoardMode::Blackboard);
                true
            }
            MenuCommand::ReturnToTransparent => {
                self.switch_board_mode(BoardMode::Transparent);
                true
            }
            MenuCommand::ToggleHelp => {
                self.show_help = !self.show_help;
                true
            }
        };

        if changed {
            self.needs_redraw = true;
        }
        self.close_context_menu();
    }

    fn delete_selected_shapes(&mut self) -> bool {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return false;
        }

        let mut removed = Vec::new();
        for id in ids {
            if let Some((index, shape)) = self.canvas_set.active_frame_mut().remove_shape_by_id(id)
            {
                removed.push((index, shape));
            }
        }

        if removed.is_empty() {
            return false;
        }

        for (_, shape) in &removed {
            self.dirty_tracker.mark_shape(&shape.shape);
            self.invalidate_hit_cache_for(shape.id);
        }

        self.canvas_set.active_frame_mut().push_undo_action(
            UndoAction::Delete { shapes: removed },
            self.undo_stack_limit,
        );
        self.clear_selection();
        true
    }

    fn duplicate_selected_shapes(&mut self) -> bool {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return false;
        }

        let mut created = Vec::new();
        let mut new_ids = Vec::new();
        for id in ids {
            let original = {
                let frame = self.canvas_set.active_frame();
                frame.shape(id).cloned()
            };
            let Some(shape) = original else {
                continue;
            };
            if shape.locked {
                continue;
            }

            let mut cloned_shape = shape.shape.clone();
            Self::offset_shape(&mut cloned_shape, 12, 12);
            let new_id = {
                let frame = self.canvas_set.active_frame_mut();
                frame.add_shape(cloned_shape)
            };

            if let Some((index, stored)) = {
                let frame = self.canvas_set.active_frame();
                frame
                    .find_index(new_id)
                    .and_then(|idx| frame.shape(new_id).map(|s| (idx, s.clone())))
            } {
                self.dirty_tracker.mark_shape(&stored.shape);
                self.invalidate_hit_cache_for(new_id);
                created.push((index, stored));
                new_ids.push(new_id);
            }
        }

        if created.is_empty() {
            return false;
        }

        self.canvas_set.active_frame_mut().push_undo_action(
            UndoAction::Create { shapes: created },
            self.undo_stack_limit,
        );
        self.set_selection(new_ids);
        true
    }

    fn show_properties_panel(&mut self) -> bool {
        let ids = self.selected_shape_ids();
        if ids.is_empty() {
            return false;
        }

        let frame = self.canvas_set.active_frame();
        let anchor_rect = self.selection_bounding_box(ids);
        let anchor = anchor_rect
            .map(|rect| {
                (
                    (rect.x + rect.width + 12) as f64,
                    (rect.y - 12).max(12) as f64,
                )
            })
            .unwrap_or_else(|| {
                let (px, py) = self.last_pointer_position;
                ((px + 16) as f64, (py - 16) as f64)
            });

        if ids.len() > 1 {
            let total = ids.len();
            let locked = ids
                .iter()
                .filter(|id| frame.shape(**id).map(|shape| shape.locked).unwrap_or(false))
                .count();
            let mut lines = Vec::new();
            lines.push(format!("Shapes selected: {total}"));
            if locked > 0 {
                lines.push(format!("Locked: {locked}/{total}"));
            }
            if let Some(bounds) = anchor_rect {
                lines.push(format!(
                    "Bounds: {}×{} px",
                    bounds.width.max(0),
                    bounds.height.max(0)
                ));
            }
            self.set_properties_panel(ShapePropertiesPanel {
                title: "Selection Summary".to_string(),
                anchor,
                lines,
                multiple_selection: true,
            });
            return true;
        }

        let shape_id = ids[0];
        let index = match frame.find_index(shape_id) {
            Some(idx) => idx,
            None => return false,
        };
        let drawn = match frame.shape(shape_id) {
            Some(shape) => shape,
            None => return false,
        };

        let mut lines = Vec::new();
        lines.push(format!("Shape ID: {shape_id}"));
        lines.push(format!("Type: {}", drawn.shape.kind_name()));
        lines.push(format!("Layer: {} of {}", index + 1, frame.shapes.len()));
        lines.push(format!(
            "Locked: {}",
            if drawn.locked { "Yes" } else { "No" }
        ));
        if let Some(timestamp) = Self::format_timestamp(drawn.created_at) {
            lines.push(format!("Created: {timestamp}"));
        }
        if let Some(bounds) = drawn.shape.bounding_box() {
            lines.push(format!("Bounds: {}×{} px", bounds.width, bounds.height));
        }

        self.set_properties_panel(ShapePropertiesPanel {
            title: "Shape Properties".to_string(),
            anchor,
            lines,
            multiple_selection: false,
        });
        true
    }

    fn selection_bounding_box(&self, ids: &[ShapeId]) -> Option<Rect> {
        let frame = self.canvas_set.active_frame();
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut found = false;

        for id in ids {
            if let Some(shape) = frame.shape(*id) {
                if let Some(bounds) = shape.shape.bounding_box() {
                    min_x = min_x.min(bounds.x);
                    min_y = min_y.min(bounds.y);
                    max_x = max_x.max(bounds.x + bounds.width);
                    max_y = max_y.max(bounds.y + bounds.height);
                    found = true;
                }
            }
        }

        if found {
            Rect::from_min_max(min_x, min_y, max_x, max_y)
        } else {
            None
        }
    }

    fn keyboard_canvas_menu_anchor(&self) -> (i32, i32) {
        let (px, py) = self.last_pointer_position;
        if px != 0 || py != 0 {
            return self.clamp_anchor(px, py);
        }
        let cx = (self.screen_width / 2).saturating_sub(1) as i32;
        let cy = (self.screen_height / 2).saturating_sub(1) as i32;
        self.clamp_anchor(cx, cy)
    }

    fn keyboard_shape_menu_anchor(&self, ids: &[ShapeId]) -> (i32, i32) {
        if let Some(bounds) = self.selection_bounding_box(ids) {
            let x = bounds.x + bounds.width;
            let y = bounds.y;
            return self.clamp_anchor(x, y);
        }
        self.keyboard_canvas_menu_anchor()
    }

    fn clamp_anchor(&self, x: i32, y: i32) -> (i32, i32) {
        let max_x = self.screen_width.saturating_sub(8) as i32;
        let max_y = self.screen_height.saturating_sub(8) as i32;
        (x.clamp(0, max_x), y.clamp(0, max_y))
    }

    fn move_selection_to_front(&mut self) -> bool {
        self.reorder_selection(true)
    }

    fn move_selection_to_back(&mut self) -> bool {
        self.reorder_selection(false)
    }

    fn reorder_selection(&mut self, to_front: bool) -> bool {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return false;
        }

        let mut actions = Vec::new();
        let len = self.canvas_set.active_frame().shapes.len();
        for id in ids {
            let movement = {
                let frame = self.canvas_set.active_frame_mut();
                if let Some(from) = frame.find_index(id) {
                    let target = if to_front { len.saturating_sub(1) } else { 0 };
                    if from == target {
                        None
                    } else if frame.move_shape(from, target).is_some() {
                        Some((from, target))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some((from, target)) = movement {
                actions.push(UndoAction::Reorder {
                    shape_id: id,
                    from,
                    to: target,
                });
                if let Some(shape) = self.canvas_set.active_frame().shape(id) {
                    self.dirty_tracker.mark_shape(&shape.shape);
                    self.invalidate_hit_cache_for(id);
                }
            }
        }

        if actions.is_empty() {
            return false;
        }

        self.canvas_set
            .active_frame_mut()
            .push_undo_action(UndoAction::Compound(actions), self.undo_stack_limit);
        true
    }

    fn set_selection_locked(&mut self, locked: bool) -> bool {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return false;
        }

        let mut actions = Vec::new();
        for id in ids {
            let result = {
                let frame = self.canvas_set.active_frame_mut();
                if let Some(shape) = frame.shape_mut(id) {
                    if shape.locked == locked {
                        None
                    } else {
                        let before = ShapeSnapshot {
                            shape: shape.shape.clone(),
                            locked: !locked,
                        };
                        shape.locked = locked;
                        let after = ShapeSnapshot {
                            shape: shape.shape.clone(),
                            locked,
                        };
                        Some((before, after, shape.shape.clone()))
                    }
                } else {
                    None
                }
            };

            if let Some((before, after, shape_for_dirty)) = result {
                actions.push(UndoAction::Modify {
                    shape_id: id,
                    before,
                    after,
                });
                self.dirty_tracker.mark_shape(&shape_for_dirty);
                self.invalidate_hit_cache_for(id);
            }
        }

        if actions.is_empty() {
            return false;
        }

        self.canvas_set
            .active_frame_mut()
            .push_undo_action(UndoAction::Compound(actions), self.undo_stack_limit);
        true
    }

    fn clear_active_shapes(&mut self) -> bool {
        let frame = self.canvas_set.active_frame_mut();
        if frame.shapes.is_empty() {
            return false;
        }

        let removed: Vec<(usize, crate::draw::DrawnShape)> =
            frame.shapes.iter().cloned().enumerate().collect();
        frame.shapes.clear();
        frame.push_undo_action(
            UndoAction::Delete { shapes: removed },
            self.undo_stack_limit,
        );
        self.invalidate_hit_cache();
        self.clear_selection();
        true
    }

    fn begin_text_edit_for_selection(&mut self) {
        if self.selected_shape_ids().len() != 1 {
            return;
        }
        let shape_id = self.selected_shape_ids()[0];
        let frame = self.canvas_set.active_frame();
        if let Some(shape) = frame.shape(shape_id) {
            if let Shape::Text { x, y, text, .. } = &shape.shape {
                self.state = DrawingState::TextInput {
                    x: *x,
                    y: *y,
                    buffer: text.clone(),
                };
                self.update_text_preview_dirty();
            }
        }
    }

    fn offset_shape(shape: &mut Shape, dx: i32, dy: i32) {
        match shape {
            Shape::Freehand { points, .. } => {
                for point in points {
                    point.0 += dx;
                    point.1 += dy;
                }
            }
            Shape::Line { x1, y1, x2, y2, .. } => {
                *x1 += dx;
                *x2 += dx;
                *y1 += dy;
                *y2 += dy;
            }
            Shape::Rect { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Shape::Ellipse { cx, cy, .. } => {
                *cx += dx;
                *cy += dy;
            }
            Shape::Arrow { x1, y1, x2, y2, .. } => {
                *x1 += dx;
                *x2 += dx;
                *y1 += dy;
                *y2 += dy;
            }
            Shape::Text { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
        }
    }

    pub fn apply_action_side_effects(&mut self, action: &UndoAction) {
        self.invalidate_hit_cache_from_action(action);
        self.mark_dirty_from_action(action);
        self.clear_selection();
        self.needs_redraw = true;
    }

    fn mark_dirty_from_action(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Create { shapes } | UndoAction::Delete { shapes } => {
                for (_, shape) in shapes {
                    self.dirty_tracker.mark_shape(&shape.shape);
                }
            }
            UndoAction::Modify {
                before,
                after,
                shape_id,
                ..
            } => {
                self.dirty_tracker.mark_shape(&before.shape);
                self.dirty_tracker.mark_shape(&after.shape);
                self.invalidate_hit_cache_for(*shape_id);
            }
            UndoAction::Reorder { shape_id, .. } => {
                if let Some(shape) = self.canvas_set.active_frame().shape(*shape_id) {
                    self.dirty_tracker.mark_shape(&shape.shape);
                    self.invalidate_hit_cache_for(*shape_id);
                }
            }
            UndoAction::Compound(actions) => {
                for action in actions {
                    self.mark_dirty_from_action(action);
                }
            }
        }
    }

    fn invalidate_hit_cache_from_action(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Create { shapes } | UndoAction::Delete { shapes } => {
                for (_, shape) in shapes {
                    self.invalidate_hit_cache_for(shape.id);
                }
            }
            UndoAction::Modify { shape_id, .. } => {
                self.invalidate_hit_cache_for(*shape_id);
            }
            UndoAction::Reorder { shape_id, .. } => {
                self.invalidate_hit_cache_for(*shape_id);
            }
            UndoAction::Compound(actions) => {
                for action in actions {
                    self.invalidate_hit_cache_from_action(action);
                }
            }
        }
    }

    fn shape_menu_entries(&self, ids: &[ShapeId]) -> Vec<ContextMenuEntry> {
        if ids.is_empty() {
            return Vec::new();
        }

        let frame = self.canvas_set.active_frame();
        let mut locked_count = 0;
        let mut text_shape = false;

        for id in ids {
            if let Some(shape) = frame.shape(*id) {
                if shape.locked {
                    locked_count += 1;
                }
                if ids.len() == 1 && matches!(shape.shape, Shape::Text { .. }) {
                    text_shape = true;
                }
            }
        }

        let all_locked = locked_count == ids.len();
        let any_locked = locked_count > 0;
        let mut entries = Vec::new();

        if all_locked {
            entries.push(ContextMenuEntry::new(
                "Unlock Shape",
                Some("L"),
                false,
                false,
                Some(MenuCommand::Unlock),
            ));
            entries.push(ContextMenuEntry::new(
                "Properties…",
                Some("I"),
                false,
                false,
                Some(MenuCommand::Properties),
            ));
            return entries;
        }

        entries.push(ContextMenuEntry::new(
            "Delete",
            Some("Del"),
            false,
            false,
            Some(MenuCommand::Delete),
        ));
        entries.push(ContextMenuEntry::new(
            "Duplicate",
            Some("D"),
            false,
            false,
            Some(MenuCommand::Duplicate),
        ));
        entries.push(ContextMenuEntry::new(
            "Move to Front",
            Some("PgUp"),
            false,
            false,
            Some(MenuCommand::MoveToFront),
        ));
        entries.push(ContextMenuEntry::new(
            "Move to Back",
            Some("PgDn"),
            false,
            false,
            Some(MenuCommand::MoveToBack),
        ));

        let lock_label = if any_locked {
            "Unlock Shape"
        } else {
            "Lock Shape"
        };
        let lock_command = if any_locked {
            MenuCommand::Unlock
        } else {
            MenuCommand::Lock
        };
        entries.push(ContextMenuEntry::new(
            lock_label,
            Some("L"),
            false,
            false,
            Some(lock_command),
        ));
        entries.push(ContextMenuEntry::new(
            "Properties…",
            Some("I"),
            false,
            false,
            Some(MenuCommand::Properties),
        ));

        if text_shape {
            entries.push(ContextMenuEntry::new(
                "Edit Text…",
                Some("Enter"),
                false,
                false,
                Some(MenuCommand::EditText),
            ));
        }

        entries
    }

    fn format_timestamp(ms: u64) -> Option<String> {
        let seconds = (ms / 1000) as i64;
        let nanos = ((ms % 1000) * 1_000_000) as u32;
        let utc_dt = chrono::DateTime::<Utc>::from_timestamp(seconds, nanos)?;
        let local_dt = utc_dt.with_timezone(&Local);
        Some(local_dt.format("%Y-%m-%d %H:%M:%S").to_string())
    }

    pub(super) fn launch_configurator(&self) {
        let binary = legacy::configurator_override()
            .unwrap_or_else(|| "wayscriber-configurator".to_string());

        match Command::new(&binary)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                log::info!(
                    "Launched wayscriber-configurator (binary: {binary}, pid: {})",
                    child.id()
                );
            }
            Err(err) => {
                log::error!("Failed to launch wayscriber-configurator using '{binary}': {err}");
                log::error!(
                    "Set WAYSCRIBER_CONFIGURATOR (or legacy HYPRMARKER_CONFIGURATOR) to override the executable path if needed."
                );
            }
        }
    }

    /// Updates screen dimensions after backend configuration.
    ///
    /// This should be called by the backend when it receives the actual
    /// screen dimensions from the display server.
    ///
    /// # Arguments
    /// * `width` - Screen width in pixels
    /// * `height` - Screen height in pixels
    pub fn update_screen_dimensions(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Drains pending dirty rectangles for the current surface size.
    pub fn take_dirty_regions(&mut self) -> Vec<Rect> {
        let width = self.screen_width.min(i32::MAX as u32) as i32;
        let height = self.screen_height.min(i32::MAX as u32) as i32;
        self.dirty_tracker.take_regions(width, height)
    }

    /// Clears any cached provisional shape bounds and marks their damage region.
    pub(crate) fn clear_provisional_dirty(&mut self) {
        if let Some(prev) = self.last_provisional_bounds.take() {
            self.dirty_tracker.mark_rect(prev);
        }
    }

    /// Updates tracked provisional shape bounds for dirty-region purposes.
    pub(crate) fn update_provisional_dirty(&mut self, current_x: i32, current_y: i32) {
        let new_bounds = self.compute_provisional_bounds(current_x, current_y);
        let previous = self.last_provisional_bounds;

        if new_bounds != previous {
            if let Some(prev) = previous {
                self.dirty_tracker.mark_rect(prev);
            }
        }

        if let Some(bounds) = new_bounds {
            self.dirty_tracker.mark_rect(bounds);
            self.last_provisional_bounds = Some(bounds);
        } else {
            self.last_provisional_bounds = None;
        }
    }

    fn compute_provisional_bounds(&self, current_x: i32, current_y: i32) -> Option<Rect> {
        if let DrawingState::Drawing {
            tool,
            start_x,
            start_y,
            points,
        } = &self.state
        {
            match tool {
                Tool::Pen => bounding_box_for_points(points, self.current_thickness),
                Tool::Line => bounding_box_for_line(
                    *start_x,
                    *start_y,
                    current_x,
                    current_y,
                    self.current_thickness,
                ),
                Tool::Rect => {
                    let (x, w) = if current_x >= *start_x {
                        (*start_x, current_x - start_x)
                    } else {
                        (current_x, start_x - current_x)
                    };
                    let (y, h) = if current_y >= *start_y {
                        (*start_y, current_y - start_y)
                    } else {
                        (current_y, start_y - current_y)
                    };
                    bounding_box_for_rect(x, y, w, h, self.current_thickness)
                }
                Tool::Ellipse => {
                    let (cx, cy, rx, ry) =
                        util::ellipse_bounds(*start_x, *start_y, current_x, current_y);
                    bounding_box_for_ellipse(cx, cy, rx, ry, self.current_thickness)
                }
                Tool::Arrow => bounding_box_for_arrow(
                    *start_x,
                    *start_y,
                    current_x,
                    current_y,
                    self.current_thickness,
                    self.arrow_length,
                    self.arrow_angle,
                ),
                Tool::Highlight => None,
            }
        } else {
            None
        }
    }

    /// Updates dirty tracking for the live text preview/caret overlay.
    pub(crate) fn update_text_preview_dirty(&mut self) {
        let new_bounds = self.compute_text_preview_bounds();
        let previous = self.last_text_preview_bounds;

        if new_bounds != previous {
            if let Some(prev) = previous {
                self.dirty_tracker.mark_rect(prev);
            }
        }

        if let Some(bounds) = new_bounds {
            self.dirty_tracker.mark_rect(bounds);
            self.last_text_preview_bounds = Some(bounds);
        } else {
            self.last_text_preview_bounds = None;
        }
    }

    /// Clears the cached text preview bounds.
    pub(crate) fn clear_text_preview_dirty(&mut self) {
        if let Some(prev) = self.last_text_preview_bounds.take() {
            self.dirty_tracker.mark_rect(prev);
        }
    }

    fn compute_text_preview_bounds(&self) -> Option<Rect> {
        if let DrawingState::TextInput { x, y, buffer } = &self.state {
            let mut preview = buffer.clone();
            preview.push('_');
            bounding_box_for_text(
                *x,
                *y,
                &preview,
                self.current_font_size,
                &self.font_descriptor,
                self.text_background_enabled,
            )
        } else {
            None
        }
    }

    /// Returns the current board mode.
    pub fn board_mode(&self) -> BoardMode {
        self.canvas_set.active_mode()
    }

    /// Look up an action for the given key and modifiers.
    pub(super) fn find_action(&self, key_str: &str) -> Option<Action> {
        // Try to find a matching keybinding
        for (binding, action) in &self.action_map {
            if binding.matches(
                key_str,
                self.modifiers.ctrl,
                self.modifiers.shift,
                self.modifiers.alt,
            ) {
                return Some(*action);
            }
        }
        None
    }

    /// Adjusts the current font size by a delta, clamping to valid range.
    ///
    /// Font size is clamped to 8.0-72.0px range (same as config validation).
    /// Triggers a redraw to update the status bar display.
    ///
    /// # Arguments
    /// * `delta` - Amount to adjust font size (positive to increase, negative to decrease)
    pub fn adjust_font_size(&mut self, delta: f64) {
        self.current_font_size = (self.current_font_size + delta).clamp(8.0, 72.0);
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        log::debug!("Font size adjusted to {:.1}px", self.current_font_size);
    }

    /// Takes and clears any pending capture action.
    ///
    /// This is called by WaylandState to retrieve capture actions that need
    /// to be handled with access to CaptureManager.
    ///
    /// # Returns
    /// The pending capture action if any, None otherwise
    pub fn take_pending_capture_action(&mut self) -> Option<Action> {
        self.pending_capture_action.take()
    }

    /// Stores a capture action for retrieval by the backend.
    pub(super) fn set_pending_capture_action(&mut self, action: Action) {
        self.pending_capture_action = Some(action);
    }

    /// Returns whether the click highlight feature is currently enabled.
    pub fn click_highlight_enabled(&self) -> bool {
        self.click_highlight.enabled()
    }

    /// Toggle the click highlight feature and mark the frame for redraw.
    pub fn toggle_click_highlight(&mut self) -> bool {
        let enabled = self.click_highlight.toggle(&mut self.dirty_tracker);
        self.needs_redraw = true;
        enabled
    }

    /// Clears any active highlights without changing the enabled flag.
    pub fn clear_click_highlights(&mut self) {
        if self.click_highlight.has_active() {
            self.click_highlight.clear_all(&mut self.dirty_tracker);
            self.needs_redraw = true;
        }
    }

    /// Spawns a highlight at the given position if the feature is enabled.
    pub fn trigger_click_highlight(&mut self, x: i32, y: i32) {
        if self.click_highlight.spawn(x, y, &mut self.dirty_tracker) {
            self.needs_redraw = true;
        }
    }

    pub fn sync_highlight_color(&mut self) {
        if self.click_highlight.apply_pen_color(self.current_color) {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Advances highlight animations; returns true if highlights remain active.
    pub fn advance_click_highlights(&mut self, now: Instant) -> bool {
        self.click_highlight.advance(now, &mut self.dirty_tracker)
    }

    /// Render active highlights to the cairo context.
    pub fn render_click_highlights(&self, ctx: &cairo::Context, now: Instant) {
        self.click_highlight.render(ctx, now);
    }

    /// Returns the active tool considering overrides and drawing state.
    pub fn active_tool(&self) -> Tool {
        if let DrawingState::Drawing { tool, .. } = &self.state {
            *tool
        } else if let Some(tool) = self.tool_override {
            tool
        } else {
            self.modifiers.current_tool()
        }
    }

    /// Returns whether the highlight tool is currently selected.
    pub fn highlight_tool_active(&self) -> bool {
        matches!(self.tool_override, Some(Tool::Highlight))
            || matches!(
                self.state,
                DrawingState::Drawing {
                    tool: Tool::Highlight,
                    ..
                }
            )
    }

    /// Toggles highlight-only tool mode.
    pub fn toggle_highlight_tool(&mut self) -> bool {
        let enable = !self.highlight_tool_active();

        if enable {
            self.tool_override = Some(Tool::Highlight);
            // Ensure we are not mid-drawing with another tool
            if !matches!(
                self.state,
                DrawingState::Idle | DrawingState::TextInput { .. }
            ) {
                self.state = DrawingState::Idle;
            }
        } else {
            self.tool_override = None;
        }

        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        enable
    }

    /// Switches to a different board mode with color auto-adjustment.
    ///
    /// Handles mode transitions with automatic color adjustment for contrast:
    /// - Entering board mode: saves current color, applies mode default
    /// - Exiting board mode: restores previous color
    /// - Switching between boards: applies new mode default
    ///
    /// Also resets drawing state to prevent partial shapes crossing modes.
    pub fn switch_board_mode(&mut self, new_mode: BoardMode) {
        let current_mode = self.canvas_set.active_mode();

        // Toggle behavior: if already in target mode, return to transparent
        let target_mode = if current_mode == new_mode && new_mode != BoardMode::Transparent {
            BoardMode::Transparent
        } else {
            new_mode
        };

        // No-op if we're already in the target mode
        if current_mode == target_mode {
            return;
        }

        // Handle color auto-adjustment based on transition type (if enabled)
        if self.board_config.auto_adjust_pen {
            match (current_mode, target_mode) {
                // Entering board mode from transparent
                (BoardMode::Transparent, BoardMode::Whiteboard | BoardMode::Blackboard) => {
                    // Save current color and apply board default
                    self.board_previous_color = Some(self.current_color);
                    if let Some(default_color) = target_mode.default_pen_color(&self.board_config) {
                        self.current_color = default_color;
                        self.sync_highlight_color();
                    }
                }
                // Exiting board mode to transparent
                (BoardMode::Whiteboard | BoardMode::Blackboard, BoardMode::Transparent) => {
                    // Restore previous color if we saved one
                    if let Some(prev_color) = self.board_previous_color {
                        self.current_color = prev_color;
                        self.board_previous_color = None;
                        self.sync_highlight_color();
                    }
                }
                // Switching between board modes
                (BoardMode::Whiteboard, BoardMode::Blackboard)
                | (BoardMode::Blackboard, BoardMode::Whiteboard) => {
                    // Apply new board's default color
                    if let Some(default_color) = target_mode.default_pen_color(&self.board_config) {
                        self.current_color = default_color;
                        self.sync_highlight_color();
                    }
                }
                // All other transitions (shouldn't happen, but handle gracefully)
                _ => {}
            }
        }

        // Switch the active frame
        self.canvas_set.switch_mode(target_mode);

        // Reset drawing state to prevent partial shapes crossing modes
        self.state = DrawingState::Idle;

        // Trigger redraw
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;

        log::info!("Switched from {:?} to {:?} mode", current_mode, target_mode);
    }
}

impl SpatialGrid {
    fn build(frame: &Frame, cell_size: i32) -> Option<Self> {
        let cell_size = cell_size.max(1);
        if frame.shapes.is_empty() {
            return None;
        }

        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();

        for (index, drawn) in frame.shapes.iter().enumerate() {
            let Some(bounds) = drawn.shape.bounding_box() else {
                continue;
            };

            let min_cell_x = bounds.x.div_euclid(cell_size);
            let max_cell_x = (bounds.x + bounds.width - 1).div_euclid(cell_size);
            let min_cell_y = bounds.y.div_euclid(cell_size);
            let max_cell_y = (bounds.y + bounds.height - 1).div_euclid(cell_size);

            for cx in min_cell_x..=max_cell_x {
                for cy in min_cell_y..=max_cell_y {
                    cells.entry((cx, cy)).or_default().push(index);
                }
            }
        }

        if cells.is_empty() {
            return None;
        }

        Some(Self {
            cell_size,
            cells,
            shape_count: frame.shapes.len(),
        })
    }

    fn query(&self, point: (i32, i32)) -> Vec<usize> {
        let cell_x = point.0.div_euclid(self.cell_size);
        let cell_y = point.1.div_euclid(self.cell_size);

        let mut unique = HashSet::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                let key = (cell_x + dx, cell_y + dy);
                if let Some(indices) = self.cells.get(&key) {
                    unique.extend(indices.iter().copied());
                }
            }
        }

        let mut result: Vec<usize> = unique.into_iter().collect();
        result.sort_unstable_by(|a, b| b.cmp(a));
        result
    }
}
