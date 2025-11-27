use super::base::InputState;
use crate::draw::ShapeId;
use crate::input::board_mode::BoardMode;
use crate::util::Rect;
use cairo::Context as CairoContext;

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

impl InputState {
    /// Returns the entries to render for the currently open context menu.
    pub fn context_menu_entries(&self) -> Vec<ContextMenuEntry> {
        match &self.context_menu_state {
            ContextMenuState::Hidden => Vec::new(),
            ContextMenuState::Open {
                kind,
                shape_ids,
                hovered_shape_id,
                ..
            } => match kind {
                ContextMenuKind::Canvas => self.canvas_menu_entries(),
                ContextMenuKind::Shape => self.shape_menu_entries(shape_ids, *hovered_shape_id),
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

    fn hovered_context_menu_shape(&self) -> Option<ShapeId> {
        if let ContextMenuState::Open {
            hovered_shape_id: Some(shape_id),
            ..
        } = &self.context_menu_state
        {
            Some(*shape_id)
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

    fn shape_menu_entries(
        &self,
        ids: &[ShapeId],
        hovered_shape_id: Option<ShapeId>,
    ) -> Vec<ContextMenuEntry> {
        let mut entries = Vec::new();
        let locked = ids.iter().any(|id| {
            self.canvas_set
                .active_frame()
                .shape(*id)
                .map(|shape| shape.locked)
                .unwrap_or(false)
        });

        if hovered_shape_id.is_some() {
            entries.push(ContextMenuEntry::new(
                "Select This Shape",
                Some("Alt+Click"),
                false,
                false,
                Some(MenuCommand::SelectHoveredShape),
            ));
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
            Some("Ctrl+D"),
            false,
            false,
            Some(MenuCommand::Duplicate),
        ));
        entries.push(ContextMenuEntry::new(
            "Move to Front",
            Some("]"),
            false,
            false,
            Some(MenuCommand::MoveToFront),
        ));
        entries.push(ContextMenuEntry::new(
            "Move to Back",
            Some("["),
            false,
            false,
            Some(MenuCommand::MoveToBack),
        ));
        entries.push(ContextMenuEntry::new(
            if locked { "Unlock" } else { "Lock" },
            Some("Ctrl+L"),
            false,
            false,
            Some(if locked {
                MenuCommand::Unlock
            } else {
                MenuCommand::Lock
            }),
        ));
        entries.push(ContextMenuEntry::new(
            "Properties",
            Some("Ctrl+Enter"),
            false,
            false,
            Some(MenuCommand::Properties),
        ));

        if ids.len() == 1 {
            let shape_id = ids[0];
            if self
                .canvas_set
                .active_frame()
                .shape(shape_id)
                .map(|shape| matches!(shape.shape, crate::draw::Shape::Text { .. }))
                .unwrap_or(false)
            {
                entries.push(ContextMenuEntry::new(
                    "Edit Text",
                    Some("Enter"),
                    false,
                    false,
                    Some(MenuCommand::EditText),
                ));
            }
        }

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
            if let Some(shortcut) = &entry.shortcut
                && let Ok(extents) = ctx.text_extents(shortcut) {
                    max_shortcut_width = max_shortcut_width.max(extents.width());
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

        let local_x = x as f64 - layout.origin_x;
        let local_y = y as f64 - layout.origin_y;

        if local_x < 0.0 || local_y < 0.0 || local_x > layout.width || local_y > layout.height {
            return None;
        }

        let row = ((local_y - layout.padding_y) / layout.row_height).floor() as isize;
        if row < 0 {
            return None;
        }

        let index = row as usize;
        if index >= entries.len() {
            None
        } else {
            Some(index)
        }
    }

    fn update_context_menu_hover_from_pointer_internal(
        &mut self,
        x: i32,
        y: i32,
        trigger_redraw: bool,
    ) {
        if !self.is_context_menu_open() {
            return;
        }
        let new_hover = self.context_menu_index_at(x, y);
        if let ContextMenuState::Open {
            ref mut hover_index,
            ref mut keyboard_focus,
            ..
        } = self.context_menu_state
            && *hover_index != new_hover {
                *hover_index = new_hover;
                if new_hover.is_some() {
                    *keyboard_focus = None;
                }
                if trigger_redraw {
                    self.needs_redraw = true;
                }
            }
    }

    /// Updates hover state based on the provided pointer position.
    pub fn update_context_menu_hover_from_pointer(&mut self, x: i32, y: i32) {
        self.update_context_menu_hover_from_pointer_internal(x, y, true);
    }

    /// Updates cached hover information without forcing a redraw.
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

    /// Opens a context menu at the given anchor for the provided kind.
    pub fn open_context_menu(
        &mut self,
        anchor: (i32, i32),
        shape_ids: Vec<ShapeId>,
        kind: ContextMenuKind,
        hovered_shape_id: Option<ShapeId>,
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
            hovered_shape_id,
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
            self.open_context_menu(anchor, Vec::new(), ContextMenuKind::Canvas, None);
            self.pending_menu_hover_recalc = false;
            self.set_context_menu_focus(None);
            self.focus_first_context_menu_entry();
        } else {
            let anchor = self.keyboard_shape_menu_anchor(&selection);
            self.update_pointer_position(anchor.0, anchor.1);
            self.open_context_menu(anchor, selection, ContextMenuKind::Shape, None);
            self.pending_menu_hover_recalc = false;
            self.focus_first_context_menu_entry();
        }
        self.needs_redraw = true;
    }

    fn keyboard_canvas_menu_anchor(&self) -> (i32, i32) {
        let padding = 16;
        let x = padding;
        let y = match &self.context_menu_layout {
            Some(layout) => (layout.origin_y + layout.height + padding as f64) as i32,
            None => padding,
        };
        (x, y)
    }

    fn keyboard_shape_menu_anchor(&self, ids: &[ShapeId]) -> (i32, i32) {
        if let Some(bounds) = self.selection_bounding_box(ids) {
            (
                (bounds.x + bounds.width / 2),
                (bounds.y + bounds.height / 2),
            )
        } else {
            let (px, py) = self.last_pointer_position;
            (px, py)
        }
    }

    pub fn execute_menu_command(&mut self, command: MenuCommand) {
        match command {
            MenuCommand::Delete => {
                self.delete_selection();
                self.close_context_menu();
            }
            MenuCommand::Duplicate => {
                self.duplicate_selection();
                self.close_context_menu();
            }
            MenuCommand::SelectHoveredShape => {
                if let Some(hovered_shape) = self.hovered_context_menu_shape() {
                    let previous_ids = self.selected_shape_ids().to_vec();
                    let previous_bounds = {
                        let frame = self.canvas_set.active_frame();
                        previous_ids
                            .iter()
                            .filter_map(|id| {
                                frame
                                    .shape(*id)
                                    .and_then(|shape| shape.shape.bounding_box())
                            })
                            .collect::<Vec<_>>()
                    };

                    self.set_selection(vec![hovered_shape]);

                    for bounds in previous_bounds {
                        self.mark_selection_dirty_region(Some(bounds));
                    }
                    let hovered_bounds = {
                        let frame = self.canvas_set.active_frame();
                        frame
                            .shape(hovered_shape)
                            .and_then(|shape| shape.shape.bounding_box())
                    };
                    self.mark_selection_dirty_region(hovered_bounds);

                    self.close_context_menu();
                } else {
                    self.close_context_menu();
                }
            }
            MenuCommand::MoveToFront => {
                self.move_selection_to_front();
                self.close_context_menu();
            }
            MenuCommand::MoveToBack => {
                self.move_selection_to_back();
                self.close_context_menu();
            }
            MenuCommand::Lock => {
                self.set_selection_locked(true);
                self.close_context_menu();
            }
            MenuCommand::Unlock => {
                self.set_selection_locked(false);
                self.close_context_menu();
            }
            MenuCommand::Properties => {
                if self.show_properties_panel() {
                    self.close_context_menu();
                }
            }
            MenuCommand::EditText => {
                if self.edit_selected_text() {
                    self.close_context_menu();
                }
            }
            MenuCommand::ClearAll => {
                self.clear_all();
                self.close_context_menu();
            }
            MenuCommand::ToggleHighlightTool => {
                self.toggle_highlight_tool();
                self.close_context_menu();
            }
            MenuCommand::ToggleClickHighlight => {
                self.toggle_click_highlight();
                self.close_context_menu();
            }
            MenuCommand::SwitchToWhiteboard => {
                self.switch_board_mode(BoardMode::Whiteboard);
                self.close_context_menu();
            }
            MenuCommand::SwitchToBlackboard => {
                self.switch_board_mode(BoardMode::Blackboard);
                self.close_context_menu();
            }
            MenuCommand::ReturnToTransparent => {
                self.switch_board_mode(BoardMode::Transparent);
                self.close_context_menu();
            }
            MenuCommand::ToggleHelp => {
                self.show_help = !self.show_help;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                self.close_context_menu();
            }
        }
    }

    pub fn set_context_menu_enabled(&mut self, enabled: bool) {
        self.context_menu_enabled = enabled;
        if !enabled && self.is_context_menu_open() {
            self.close_context_menu();
        }
    }

    pub fn context_menu_enabled(&self) -> bool {
        self.context_menu_enabled
    }
}
