use crate::config::Action;
use crate::input::tool::Tool;
use log::info;

use super::super::{InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};

impl InputState {
    pub(super) fn handle_tool_action(&mut self, action: Action) -> bool {
        match action {
            Action::IncreaseThickness => match self.active_tool() {
                Tool::Eraser => {
                    self.set_eraser_size(self.eraser_size + 1.0);
                }
                Tool::Marker => {
                    self.set_marker_opacity(self.marker_opacity + 0.05);
                }
                _ => {
                    self.current_thickness =
                        (self.current_thickness + 1.0).min(MAX_STROKE_THICKNESS);
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                }
            },
            Action::DecreaseThickness => match self.active_tool() {
                Tool::Eraser => {
                    self.set_eraser_size(self.eraser_size - 1.0);
                }
                Tool::Marker => {
                    self.set_marker_opacity(self.marker_opacity - 0.05);
                }
                _ => {
                    self.current_thickness =
                        (self.current_thickness - 1.0).max(MIN_STROKE_THICKNESS);
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                }
            },
            Action::IncreaseMarkerOpacity => {
                self.set_marker_opacity(self.marker_opacity + 0.05);
            }
            Action::DecreaseMarkerOpacity => {
                self.set_marker_opacity(self.marker_opacity - 0.05);
            }
            Action::SelectMarkerTool => {
                self.set_tool_override(Some(Tool::Marker));
            }
            Action::SelectEraserTool => {
                self.set_tool_override(Some(Tool::Eraser));
            }
            Action::ToggleEraserMode => {
                if self.toggle_eraser_mode() {
                    info!("Eraser mode set to {:?}", self.eraser_mode);
                }
            }
            Action::SelectPenTool => {
                self.set_tool_override(Some(Tool::Pen));
            }
            Action::SelectLineTool => {
                self.set_tool_override(Some(Tool::Line));
            }
            Action::SelectRectTool => {
                self.set_tool_override(Some(Tool::Rect));
            }
            Action::SelectEllipseTool => {
                self.set_tool_override(Some(Tool::Ellipse));
            }
            Action::SelectArrowTool => {
                self.set_tool_override(Some(Tool::Arrow));
            }
            Action::SelectHighlightTool => {
                self.set_highlight_tool(true);
                self.set_tool_override(Some(Tool::Highlight));
            }
            Action::IncreaseFontSize => {
                self.adjust_font_size(2.0);
            }
            Action::DecreaseFontSize => {
                self.adjust_font_size(-2.0);
            }
            Action::ToggleFill => {
                let enable = !self.fill_enabled;
                if self.set_fill_enabled(enable) {
                    info!("Fill {}", if enable { "enabled" } else { "disabled" });
                }
            }
            Action::ToggleHighlightTool => {
                let enabled = self.toggle_all_highlights();
                let message = if enabled {
                    "Highlight pen enabled"
                } else {
                    "Highlight pen disabled"
                };
                info!("{}", message);
            }
            _ => return false,
        }

        true
    }
}
