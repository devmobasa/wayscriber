use crate::domain::Action;
use crate::input::state::{Toast, ToastPriority};
use crate::input::tool::Tool;
use log::info;

use super::super::{InputState, PendingToolbarPersistence};

impl InputState {
    pub(in crate::input::state) fn handle_tool_action(&mut self, action: Action) -> bool {
        if let Some(tool) = Tool::from_select_action(action) {
            // Pressing the marker's own key while it is already in hand flips
            // snap mode, the way pressing a shape tool again flips its variant.
            // Re-selecting the tool you are already holding has no other
            // meaning, so the second press is free to carry this one.
            if tool == Tool::Marker && self.active_tool() == Tool::Marker {
                self.announce_marker_snap_toggle();
                return true;
            }
            if tool == Tool::Highlight {
                // Picking the highlight tool switches the click highlight on
                // as a side effect, which is the same durable choice the
                // explicit toggle makes.
                let previous_enabled = self.click_highlight_enabled();
                let previous_tool_ring = self.highlight_tool_ring_enabled();
                self.set_highlight_tool(true);
                self.queue_toolbar_persistence(PendingToolbarPersistence::ClickHighlight {
                    previous_enabled,
                    previous_tool_ring,
                });
            }
            self.set_tool_override(Some(tool));
            return true;
        }

        match action {
            Action::IncreaseThickness => {
                self.nudge_thickness_for_active_tool(1.0);
            }
            Action::DecreaseThickness => {
                self.nudge_thickness_for_active_tool(-1.0);
            }
            Action::IncreaseMarkerOpacity => {
                self.set_marker_opacity(self.marker_opacity + 0.05);
            }
            Action::DecreaseMarkerOpacity => {
                self.set_marker_opacity(self.marker_opacity - 0.05);
            }
            Action::ToggleEraserMode => {
                if self.toggle_eraser_mode() {
                    info!("Eraser mode set to {:?}", self.eraser_mode);
                }
            }
            Action::SelectSpotlightTool => {
                self.set_tool_override(Some(Tool::Spotlight));
            }
            Action::ToggleMarkerSnapToText => {
                self.announce_marker_snap_toggle_with_tool();
            }
            Action::CycleBlurStyle => {
                if self.cycle_blur_style() {
                    let label = self.blur_style.label();
                    info!("Blur style set to {label}");
                    self.push_toast(
                        ToastPriority::Info,
                        "blur-style",
                        Toast::info(format!("Blur style: {label}")),
                    );
                }
            }
            Action::IncreaseFontSize => {
                self.adjust_font_size(2.0);
            }
            Action::DecreaseFontSize => {
                self.adjust_font_size(-2.0);
            }
            Action::ResetArrowLabelCounter => {
                if self.reset_arrow_label_counter() {
                    info!("Arrow label counter reset");
                }
            }
            Action::ResetStepMarkerCounter => {
                if self.reset_step_marker_counter() {
                    info!("Step marker counter reset");
                }
            }
            Action::ToggleFill => {
                let enable = !self.fill_enabled;
                if self.set_fill_enabled(enable) {
                    info!("Fill {}", if enable { "enabled" } else { "disabled" });
                }
            }
            Action::ToggleHighlightTool => {
                let previous_enabled = self.click_highlight_enabled();
                let previous_tool_ring = self.highlight_tool_ring_enabled();
                let enabled = self.toggle_all_highlights();
                self.queue_toolbar_persistence(PendingToolbarPersistence::ClickHighlight {
                    previous_enabled,
                    previous_tool_ring,
                });
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
