use crate::draw::Shape;
use crate::draw::frame::ShapeSnapshot;
use crate::input::tool::ToolPressBehavior;
use crate::input::{DragTool, Tool, events::MouseButton};
use std::sync::Arc;

use super::super::core::MenuCommand;
use super::super::{
    ContextMenuKind, DrawingState, InputState,
    interaction::{CanvasPoint, PointerPoints, PointerPress, ScreenPoint, route_pointer_press},
};

mod panels;
mod polygon;

#[derive(Clone, Copy)]
struct PressCoords {
    screen_x: i32,
    screen_y: i32,
    canvas_x: i32,
    canvas_y: i32,
}

impl InputState {
    pub(in crate::input::state) fn is_radial_menu_toggle_button(
        &self,
        button: MouseButton,
    ) -> bool {
        use crate::config::RadialMenuMouseBinding;
        match self.radial_menu_mouse_binding {
            RadialMenuMouseBinding::Middle => matches!(button, MouseButton::Middle),
            RadialMenuMouseBinding::Right => matches!(button, MouseButton::Right),
            RadialMenuMouseBinding::Disabled => false,
        }
    }

    pub(in crate::input::state) fn should_toggle_radial_menu_from_mouse(
        &self,
        button: MouseButton,
    ) -> bool {
        !self.zoom_active()
            && matches!(self.state, DrawingState::Idle)
            && self.is_radial_menu_toggle_button(button)
    }

    pub(in crate::input::state) fn handle_right_click(
        &mut self,
        screen_x: i32,
        screen_y: i32,
        canvas_x: i32,
        canvas_y: i32,
    ) {
        self.update_pointer_positions(screen_x, screen_y, canvas_x, canvas_y);
        self.last_text_click = None;
        if self.try_cancel_active_interaction() {
            return;
        }
        if self.zoom_active() {
            return;
        }
        if !self.context_menu_enabled() {
            return;
        }

        let hit_shape = self.hit_test_at(canvas_x, canvas_y);
        let mut focus_edit = false;
        if let Some(id) = hit_shape {
            if self.modifiers.shift {
                self.extend_selection([id]);
            } else if !self.selected_shape_ids().contains(&id) {
                self.set_selection(vec![id]);
            }
            let selection = self.selected_shape_ids().to_vec();
            focus_edit = selection.len() == 1
                && self
                    .boards
                    .active_frame()
                    .shape(selection[0])
                    .map(|shape| {
                        matches!(shape.shape, Shape::Text { .. } | Shape::StickyNote { .. })
                    })
                    .unwrap_or(false);
            self.open_context_menu(
                (screen_x, screen_y),
                selection,
                ContextMenuKind::Shape,
                hit_shape,
            );
        } else {
            self.clear_selection();
            self.open_context_menu(
                (screen_x, screen_y),
                Vec::new(),
                ContextMenuKind::Canvas,
                None,
            );
        }

        self.update_context_menu_hover_from_pointer(screen_x, screen_y);
        if focus_edit {
            self.focus_context_menu_command(MenuCommand::EditText);
        }
        if self.is_context_menu_open() {
            self.pending_onboarding_usage.used_context_menu_right_click = true;
        }
        self.needs_redraw = true;
    }

    /// Processes a mouse button press event.
    ///
    /// # Arguments
    /// * `button` - Which mouse button was pressed
    /// * `x` - Mouse X coordinate
    /// * `y` - Mouse Y coordinate
    ///
    /// # Behavior
    /// - Left click while Idle: Starts drawing with the current tool (based on modifiers)
    /// - Left click during TextInput: Updates text position
    /// - Right click: Cancels current action
    #[allow(dead_code)] // Retained for older callers that only have canvas coordinates.
    pub fn on_mouse_press(&mut self, button: MouseButton, x: i32, y: i32) {
        let (screen_x, screen_y) = self.screen_coords_for_canvas(x, y);
        self.on_mouse_press_with_canvas(button, screen_x, screen_y, x, y);
    }

    pub fn on_mouse_press_with_canvas(
        &mut self,
        button: MouseButton,
        screen_x: i32,
        screen_y: i32,
        canvas_x: i32,
        canvas_y: i32,
    ) {
        let points = PointerPoints::new(
            ScreenPoint::new(screen_x, screen_y),
            CanvasPoint::new(canvas_x, canvas_y),
        );
        let _ = route_pointer_press(self, PointerPress::new(button, points));
    }

    pub(in crate::input::state) fn tool_for_button_press(
        &self,
        button: MouseButton,
        binding_tool: DragTool,
    ) -> Option<Tool> {
        let configured_tool = binding_tool.as_tool();
        if configured_tool.is_some()
            && self.presenter_mode
            && matches!(
                self.presenter_mode_config.tool_behavior,
                crate::config::PresenterToolBehavior::ForceHighlightLocked
            )
        {
            return Some(Tool::Highlight);
        }

        if button == MouseButton::Left
            && let Some(override_tool) = self.tool_override()
            && (matches!(override_tool, Tool::Highlight | Tool::Eraser)
                || !self.modifiers.active_drag_modifier().is_active())
        {
            return Some(self.active_tool());
        }
        configured_tool
    }

    fn handle_tool_button_press(
        &mut self,
        button: MouseButton,
        tool: Tool,
        color: Option<crate::draw::Color>,
        coords: PressCoords,
    ) {
        self.update_pointer_positions(
            coords.screen_x,
            coords.screen_y,
            coords.canvas_x,
            coords.canvas_y,
        );
        self.trigger_click_highlight(coords.canvas_x, coords.canvas_y);

        if self.handle_context_menu_press(coords.screen_x, coords.screen_y) {
            return;
        }

        match &mut self.state {
            DrawingState::Idle => {
                self.handle_idle_tool_click(button, tool, color, coords.canvas_x, coords.canvas_y)
            }
            DrawingState::TextInput { x: tx, y: ty, .. } if button == MouseButton::Left => {
                *tx = coords.canvas_x;
                *ty = coords.canvas_y;
                self.update_text_preview_dirty();
                self.needs_redraw = true;
            }
            DrawingState::BuildingPolygon { .. } if button == MouseButton::Left => {
                self.handle_building_polygon_left_click(coords.canvas_x, coords.canvas_y);
            }
            DrawingState::TextInput { .. }
            | DrawingState::BuildingPolygon { .. }
            | DrawingState::Drawing { .. }
            | DrawingState::MovingSelection { .. }
            | DrawingState::Selecting { .. }
            | DrawingState::PendingTextClick { .. }
            | DrawingState::ResizingText { .. }
            | DrawingState::ResizingSelection { .. } => {}
        }
    }

    pub(in crate::input::state) fn handle_tool_button_press_at(
        &mut self,
        button: MouseButton,
        tool: Tool,
        color: Option<crate::draw::Color>,
        screen: (i32, i32),
        canvas: (i32, i32),
    ) {
        self.handle_tool_button_press(
            button,
            tool,
            color,
            PressCoords {
                screen_x: screen.0,
                screen_y: screen.1,
                canvas_x: canvas.0,
                canvas_y: canvas.1,
            },
        );
    }

    fn handle_idle_tool_click(
        &mut self,
        button: MouseButton,
        tool: Tool,
        color: Option<crate::draw::Color>,
        x: i32,
        y: i32,
    ) {
        let selection_click =
            self.modifiers.alt || matches!(tool.press_behavior(), ToolPressBehavior::Selection);
        let hit_id = self.hit_test_at(x, y);

        if let Some(shape_id) = self.hit_text_resize_handle(x, y) {
            let snapshot = {
                let frame = self.boards.active_frame();
                frame.shape(shape_id).map(|shape| ShapeSnapshot {
                    shape: shape.shape.clone(),
                    locked: shape.locked,
                })
            };
            if let Some(snapshot) = snapshot {
                let (base_x, size) = match &snapshot.shape {
                    Shape::Text { x, size, .. } => (*x, *size),
                    Shape::StickyNote { x, size, .. } => (*x, *size),
                    _ => return,
                };
                self.last_text_click = None;
                self.begin_pointer_drag(button, color);
                self.state = DrawingState::ResizingText {
                    shape_id,
                    snapshot,
                    base_x,
                    size,
                };
                return;
            }
        }

        if let Some(handle) = self.hit_selection_handle(x, y)
            && let Some(original_bounds) = self.selection_bounds()
        {
            let snapshots = self.capture_resize_selection_snapshots();
            if !snapshots.is_empty() {
                self.last_text_click = None;
                self.begin_pointer_drag(button, color);
                self.state = DrawingState::ResizingSelection {
                    handle,
                    original_bounds,
                    start_x: x,
                    start_y: y,
                    snapshots: Arc::new(snapshots),
                };
                return;
            }
        }

        if !selection_click && let Some(hit_id) = hit_id {
            let is_text = self
                .boards
                .active_frame()
                .shape(hit_id)
                .map(|shape| {
                    !shape.locked
                        && matches!(shape.shape, Shape::Text { .. } | Shape::StickyNote { .. })
                })
                .unwrap_or(false);
            if is_text {
                self.begin_pointer_drag(button, color);
                self.state = DrawingState::PendingTextClick {
                    x,
                    y,
                    tool,
                    shape_id: hit_id,
                };
                return;
            }
        }

        self.last_text_click = None;
        if selection_click {
            if let Some(hit_id) = hit_id {
                if !self.selected_shape_ids().contains(&hit_id) {
                    if self.modifiers.shift {
                        self.extend_selection([hit_id]);
                    } else {
                        self.set_selection(vec![hit_id]);
                    }
                }

                let snapshots = self.capture_movable_selection_snapshots();
                if !snapshots.is_empty() {
                    self.begin_pointer_drag(button, color);
                    self.state = DrawingState::MovingSelection {
                        last_x: x,
                        last_y: y,
                        snapshots,
                        moved: false,
                    };
                    return;
                }
            } else {
                self.begin_pointer_drag(button, color);
                self.state = DrawingState::Selecting {
                    start_x: x,
                    start_y: y,
                    additive: self.modifiers.shift,
                };
                self.last_provisional_bounds = None;
                self.update_provisional_dirty(x, y);
                self.needs_redraw = true;
                return;
            }
        }

        match tool.press_behavior() {
            ToolPressBehavior::Selection | ToolPressBehavior::HighlightNoop => {}
            ToolPressBehavior::StartFreeformPolygon => {
                self.start_building_polygon(x, y);
            }
            ToolPressBehavior::StartDrawing {
                request_blur_capture,
            } => {
                if request_blur_capture && !self.frozen_active() && !self.pending_frozen_toggle() {
                    self.request_frozen_toggle();
                }
                self.sync_current_settings_for_tool(tool);
                let drawing_thickness = self.thickness_for_tool(tool);
                self.begin_pointer_drag(button, color);
                self.state = DrawingState::Drawing {
                    tool,
                    start_x: x,
                    start_y: y,
                    points: vec![(x, y)],
                    point_thicknesses: vec![drawing_thickness as f32],
                };
                self.last_provisional_bounds = None;
                self.update_provisional_dirty(x, y);
                self.needs_redraw = true;
            }
        }
    }
}
