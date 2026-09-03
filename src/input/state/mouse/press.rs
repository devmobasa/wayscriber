use crate::draw::Shape;
use crate::input::tool::ToolPressBehavior;
use crate::input::{DragTool, Tool, events::MouseButton};
use std::sync::Arc;

use super::super::core::{IdleHandle, MenuCommand};
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
        match self.radial_menu.mouse_binding {
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
        self.text_editing.set_last_click(None);
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
        // Any press ends a wheel adjustment of a loupe, so the burst lands in
        // history as its own entry rather than merging with what follows.
        self.flush_spotlight_magnification_gesture();
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
            && self.presenter_mode_active()
            && matches!(
                self.presenter_mode_config().tool_behavior,
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

        // A left click in text mode positions the caret (Shift extends the
        // selection) via glyph hit-testing, rather than relocating the block.
        // Holding Alt turns the drag into a move of the whole block, which frees
        // plain-drag for text selection later.
        if button == MouseButton::Left
            && self.handle_text_input_left_press(coords.canvas_x, coords.canvas_y, color)
        {
            return;
        }

        match &mut self.state {
            DrawingState::Idle => {
                self.handle_idle_tool_click(button, tool, color, coords.canvas_x, coords.canvas_y)
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
            | DrawingState::ResizingSelection { .. }
            | DrawingState::BendingArrow { .. }
            | DrawingState::AdjustingSpotlightMagnification { .. } => {}
        }
    }

    /// Apply the editor-owned meaning of a left press independently of drawing
    /// tool bindings: click positions/extends the caret, Alt+drag moves the
    /// block. Returns whether an active text editor consumed the press.
    pub(in crate::input::state) fn handle_text_input_left_press(
        &mut self,
        canvas_x: i32,
        canvas_y: i32,
        color: Option<crate::draw::Color>,
    ) -> bool {
        if !matches!(self.state, DrawingState::TextInput { .. }) {
            return false;
        }
        if self.modifiers.alt {
            self.begin_text_block_drag(canvas_x, canvas_y, color);
        } else if !self.modifiers.shift
            && self.text_editing.edit_target().is_none()
            && matches!(&self.state, DrawingState::TextInput { buffer, .. } if buffer.is_empty())
        {
            // A newly selected text tool is seeded at the screen center so its
            // caret can render immediately. Preserve the established first
            // click workflow by moving that still-empty draft to the click;
            // existing shapes and populated drafts use caret hit-testing.
            if let DrawingState::TextInput {
                x,
                y,
                caret,
                selection_anchor,
                ..
            } = &mut self.state
            {
                *x = canvas_x;
                *y = canvas_y;
                *caret = 0;
                *selection_anchor = None;
            }
            self.needs_redraw = true;
            self.update_text_preview_dirty_from_editor();
        } else {
            self.place_text_caret_at_canvas(canvas_x, canvas_y, self.modifiers.shift);
        }
        true
    }

    /// Position the text caret from a canvas-space click by hit-testing the
    /// rendered glyphs. With `extend` (Shift held), grows the selection from
    /// the current caret instead of collapsing it. Both plain text and sticky
    /// notes render their glyph run at the stored `(x, y)` baseline origin, so
    /// one formula covers both. Active IME composition is hit-tested against
    /// the same effective preview as rendering, then mapped back to committed
    /// buffer coordinates.
    fn place_text_caret_at_canvas(&mut self, canvas_x: i32, canvas_y: i32, extend: bool) {
        let offset = {
            let DrawingState::TextInput { x, y, .. } = &self.state else {
                return;
            };
            let cursor_glyph = if self.text_editing.edit_target().is_some() {
                "|"
            } else {
                "_"
            };
            let Some(preview) = self.text_input_preview(cursor_glyph) else {
                return;
            };
            let font = self
                .style
                .font_descriptor
                .to_pango_string(self.style.current_font_size);
            let Some(preview_offset) = crate::draw::shape::hit_test_text(
                &preview.text,
                &font,
                self.style.text_wrap_width,
                (canvas_x - *x) as f64,
                (canvas_y - *y) as f64,
            ) else {
                return;
            };
            preview.buffer_offset_for_preview_offset(preview_offset)
        };

        if let DrawingState::TextInput {
            caret,
            selection_anchor,
            ..
        } = &mut self.state
        {
            if extend {
                if selection_anchor.is_none() {
                    *selection_anchor = Some(*caret);
                }
            } else {
                *selection_anchor = None;
            }
            *caret = offset;
            self.needs_redraw = true;
            self.update_text_preview_dirty_from_editor();
        }
    }

    /// Begin an Alt+left-drag that moves the whole active text block. Records
    /// the grab offset so the block tracks the cursor exactly under the grabbed
    /// point, and arms the pointer drag so motion/release route back here.
    fn begin_text_block_drag(
        &mut self,
        canvas_x: i32,
        canvas_y: i32,
        color: Option<crate::draw::Color>,
    ) {
        if self
            .text_editing
            .begin_block_drag(&self.state, canvas_x, canvas_y)
        {
            self.begin_pointer_drag(MouseButton::Left, color);
            self.needs_redraw = true;
        }
    }

    /// Whether an Alt+drag block move is in progress. The motion and release
    /// routers use this to keep the (otherwise passive) `TextInput` state
    /// draggable while the flag is set.
    pub(in crate::input::state) fn text_block_drag_active(&self) -> bool {
        self.text_editing.text_block_drag().is_some()
    }

    /// Update the active text block's origin from a canvas-space pointer during
    /// an Alt+drag, preserving the grab offset. No-op when not dragging.
    pub(in crate::input::state) fn drag_text_block_to(&mut self, canvas_x: i32, canvas_y: i32) {
        if self
            .text_editing
            .drag_block_to(&mut self.state, canvas_x, canvas_y)
        {
            self.update_text_preview_dirty();
            self.needs_redraw = true;
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

        // Handles are claimed before tool dispatch, in the order
        // `hit_idle_handle` fixes — the same order the pointer cursor previews,
        // so what the user sees is what the press does.
        match self.hit_idle_handle(x, y) {
            Some(IdleHandle::SpotlightMagnification(shape_id)) => {
                if let Some(snapshot) = self.shape_snapshot(shape_id) {
                    self.text_editing.set_last_click(None);
                    self.begin_pointer_drag(button, color);
                    self.state =
                        DrawingState::AdjustingSpotlightMagnification { shape_id, snapshot };
                    // Jump to where the user pressed, so a click anywhere on
                    // the track is itself an adjustment rather than dead travel.
                    self.drag_spotlight_magnification_to(x);
                    return;
                }
            }
            Some(IdleHandle::ArrowBend(shape_id)) => {
                if let Some(snapshot) = self.shape_snapshot(shape_id) {
                    self.text_editing.set_last_click(None);
                    self.begin_pointer_drag(button, color);
                    self.state = DrawingState::BendingArrow { shape_id, snapshot };
                    // Jump the arc to where the user pressed, so a click beside
                    // the handle is itself an adjustment rather than dead travel.
                    self.drag_arrow_bend_to(x, y, self.modifiers.shift);
                    return;
                }
            }
            Some(IdleHandle::TextResize(shape_id)) => {
                if let Some(snapshot) = self.shape_snapshot(shape_id) {
                    let (base_x, size) = match &snapshot.shape {
                        Shape::Text { x, size, .. } => (*x, *size),
                        Shape::StickyNote { x, size, .. } => (*x, *size),
                        _ => return,
                    };
                    self.text_editing.set_last_click(None);
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
            Some(IdleHandle::SelectionResize(handle)) => {
                if let Some(original_bounds) = self.selection_bounds() {
                    let snapshots = self.capture_resize_selection_snapshots();
                    if !snapshots.is_empty() {
                        self.text_editing.set_last_click(None);
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
            }
            None => {}
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

        self.text_editing.set_last_click(None);
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
                self.pointer.replace_provisional_bounds(None);
                self.update_provisional_dirty(x, y);
                self.needs_redraw = true;
                return;
            }
        }

        match tool.press_behavior() {
            ToolPressBehavior::Selection | ToolPressBehavior::HighlightNoop => {}
            ToolPressBehavior::StartFreeformPolygon => {
                self.mark_draw_activity();
                self.start_building_polygon(x, y);
            }
            ToolPressBehavior::StartDrawing {
                request_blur_capture,
            } => {
                self.mark_draw_activity();
                if request_blur_capture
                    && self.style.blur_style.needs_backdrop()
                    && !self.frozen_active()
                    && !self.pending_frozen_toggle()
                {
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
                self.pointer.replace_provisional_bounds(None);
                self.update_provisional_dirty(x, y);
                self.needs_redraw = true;
            }
        }
    }
}
