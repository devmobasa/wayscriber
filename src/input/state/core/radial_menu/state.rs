use super::{ACTIONS_CHILDREN, RadialMenuState, RadialSegmentId, SHAPES_CHILDREN, TEXT_CHILDREN};
use crate::config::Action;
use crate::draw::color;
use crate::input::DrawingState;
use crate::input::state::InputState;
use crate::input::state::TextInputMode;
use crate::input::tool::Tool;

impl InputState {
    /// Whether the radial menu is currently visible.
    pub fn is_radial_menu_open(&self) -> bool {
        matches!(self.radial_menu_state, RadialMenuState::Open { .. })
    }

    /// Open the radial menu centered on the given surface coordinates.
    pub fn open_radial_menu(&mut self, x: f64, y: f64) {
        // Mutual exclusion with other popups
        if self.show_help {
            self.toggle_help_overlay();
        }
        if self.is_context_menu_open() {
            self.close_context_menu();
        }
        if self.is_color_picker_popup_open() {
            self.close_color_picker_popup(true);
        }
        if self.is_properties_panel_open() {
            self.close_properties_panel();
        }
        if self.command_palette_open {
            self.command_palette_open = false;
        }

        self.radial_menu_state = RadialMenuState::Open {
            center_x: x,
            center_y: y,
            hover: None,
            expanded_sub_ring: None,
        };
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Close the radial menu.
    pub fn close_radial_menu(&mut self) {
        if self.is_radial_menu_open() {
            self.radial_menu_state = RadialMenuState::Hidden;
            self.radial_menu_layout = None;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Toggle the radial menu open/closed at the given position.
    pub fn toggle_radial_menu(&mut self, x: f64, y: f64) {
        if self.is_radial_menu_open() {
            self.close_radial_menu();
        } else {
            self.open_radial_menu(x, y);
        }
    }

    /// Update the hovered segment based on pointer position.
    pub fn update_radial_menu_hover(&mut self, x: f64, y: f64) {
        if let RadialMenuState::Open {
            ref mut hover,
            ref mut expanded_sub_ring,
            ..
        } = self.radial_menu_state
            && let Some(layout) = &self.radial_menu_layout
        {
            let segment = super::hit_test::hit_test_radial(layout, *expanded_sub_ring, x, y);
            let old_hover = *hover;
            let old_expanded_sub_ring = *expanded_sub_ring;
            *hover = segment;

            // Expand/collapse sub-ring based on hovered segment
            match segment {
                Some(RadialSegmentId::Tool(4)) => {
                    *expanded_sub_ring = Some(4);
                }
                Some(RadialSegmentId::Tool(5)) => {
                    *expanded_sub_ring = Some(5);
                }
                Some(RadialSegmentId::Tool(8)) => {
                    *expanded_sub_ring = Some(8);
                }
                // Keep sub-ring expanded while hovering its children
                Some(RadialSegmentId::SubTool(_, _)) => {}
                // Keep sub-ring expanded when cursor is in/near the sub-ring
                // band (None from gap or outside parent angle range)
                None if expanded_sub_ring.is_some() => {
                    let dx = x - layout.center_x;
                    let dy = y - layout.center_y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    // Only collapse if cursor left the sub-ring distance band
                    if dist < layout.tool_inner || dist > layout.sub_outer {
                        *expanded_sub_ring = None;
                    }
                }
                // Collapse when hovering a different tool or color segment
                Some(RadialSegmentId::Tool(_))
                | Some(RadialSegmentId::Color(_))
                | Some(RadialSegmentId::Center) => {
                    *expanded_sub_ring = None;
                }
                _ => {}
            }

            if old_hover != *hover || old_expanded_sub_ring != *expanded_sub_ring {
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
        }
    }

    /// Select the currently hovered segment and close the menu.
    pub fn radial_menu_select_hovered(&mut self) {
        let hover = match &self.radial_menu_state {
            RadialMenuState::Open { hover, .. } => *hover,
            _ => return,
        };

        match hover {
            Some(RadialSegmentId::Tool(idx)) if sub_ring_child_count(idx) > 0 => {
                // Parent with children — expand sub-ring, don't close
                if let RadialMenuState::Open {
                    ref mut expanded_sub_ring,
                    ..
                } = self.radial_menu_state
                {
                    *expanded_sub_ring = Some(idx);
                }
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                return;
            }
            Some(RadialSegmentId::Tool(idx)) => {
                self.dispatch_tool_segment(idx);
            }
            Some(RadialSegmentId::SubTool(parent, child)) => {
                self.dispatch_sub_tool_segment(parent, child);
            }
            Some(RadialSegmentId::Color(idx)) => {
                self.dispatch_color_segment(idx);
            }
            Some(RadialSegmentId::Center) | None => {
                // Dismiss only
            }
        }

        self.close_radial_menu();
    }

    /// Adjust thickness via scroll wheel while the menu is open.
    pub fn radial_menu_adjust_thickness(&mut self, delta: f64) -> bool {
        if !self.nudge_thickness_for_active_tool(delta) {
            return false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    // ── dispatch helpers ──

    fn dispatch_tool_segment(&mut self, idx: u8) {
        match idx {
            0 => {
                self.set_tool_override(Some(Tool::Pen));
            }
            1 => {
                self.set_tool_override(Some(Tool::Marker));
            }
            2 => {
                self.set_tool_override(Some(Tool::Line));
            }
            3 => {
                self.set_tool_override(Some(Tool::Arrow));
            }
            4 => {
                // Shapes parent — default to Rect
                self.set_tool_override(Some(Tool::Rect));
            }
            5 => {
                // Text parent — default to text mode
                self.enter_text_mode_at_center();
            }
            6 => {
                self.set_tool_override(Some(Tool::Eraser));
            }
            7 => {
                self.set_tool_override(Some(Tool::Select));
            }
            8 => {
                // Actions parent — sub-ring expanded by radial_menu_select_hovered
            }
            _ => {}
        }
    }

    fn dispatch_sub_tool_segment(&mut self, parent: u8, child: u8) {
        match parent {
            4 => {
                // Shapes sub-ring
                match child {
                    0 => {
                        self.set_tool_override(Some(Tool::Rect));
                    }
                    1 => {
                        self.set_tool_override(Some(Tool::Ellipse));
                    }
                    _ => {}
                }
            }
            5 => {
                // Text sub-ring
                match child {
                    0 => self.enter_text_mode_at_center(),
                    1 => self.enter_sticky_note_mode_at_center(),
                    2 => {
                        self.set_tool_override(Some(Tool::StepMarker));
                    }
                    _ => {}
                }
            }
            8 => {
                // Actions sub-ring
                match child {
                    0 => self.handle_action(Action::Undo),
                    1 => self.handle_action(Action::Redo),
                    2 => self.handle_action(Action::ClearCanvas),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn dispatch_color_segment(&mut self, idx: u8) {
        let c = radial_color_for_index(idx);
        self.apply_color_from_ui(c);
    }

    fn enter_text_mode_at_center(&mut self) {
        if matches!(self.state, DrawingState::Idle) {
            self.text_input_mode = TextInputMode::Plain;
            self.text_edit_target = None;
            self.text_wrap_width = None;
            self.state = DrawingState::TextInput {
                x: (self.screen_width / 2) as i32,
                y: (self.screen_height / 2) as i32,
                buffer: String::new(),
            };
            self.last_text_preview_bounds = None;
            self.update_text_preview_dirty();
            self.needs_redraw = true;
        }
    }

    fn enter_sticky_note_mode_at_center(&mut self) {
        if matches!(self.state, DrawingState::Idle) {
            self.text_input_mode = TextInputMode::StickyNote;
            self.text_edit_target = None;
            self.text_wrap_width = None;
            self.state = DrawingState::TextInput {
                x: (self.screen_width / 2) as i32,
                y: (self.screen_height / 2) as i32,
                buffer: String::new(),
            };
            self.last_text_preview_bounds = None;
            self.update_text_preview_dirty();
            self.needs_redraw = true;
        }
    }
}

/// Map a color segment index to the corresponding draw color constant.
pub fn radial_color_for_index(idx: u8) -> crate::draw::Color {
    match idx {
        0 => color::RED,
        1 => color::GREEN,
        2 => color::BLUE,
        3 => color::YELLOW,
        4 => color::ORANGE,
        5 => color::PINK,
        6 => color::WHITE,
        7 => color::BLACK,
        _ => color::RED,
    }
}

/// Return the number of sub-ring children for a given tool index, or 0 if none.
pub fn sub_ring_child_count(parent_idx: u8) -> usize {
    match parent_idx {
        4 => SHAPES_CHILDREN.len(),
        5 => TEXT_CHILDREN.len(),
        8 => ACTIONS_CHILDREN.len(),
        _ => 0,
    }
}

/// Return the label for a sub-ring child.
pub fn sub_ring_child_label(parent_idx: u8, child_idx: u8) -> &'static str {
    match parent_idx {
        4 => SHAPES_CHILDREN
            .get(child_idx as usize)
            .copied()
            .unwrap_or(""),
        5 => TEXT_CHILDREN.get(child_idx as usize).copied().unwrap_or(""),
        8 => ACTIONS_CHILDREN
            .get(child_idx as usize)
            .copied()
            .unwrap_or(""),
        _ => "",
    }
}
