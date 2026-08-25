use crate::domain::Action;
use crate::input::state::{Toast, ToastPriority};
use crate::input::tool::Tool;
use log::info;

use super::super::{InputState, PendingToolbarPersistence};

impl InputState {
    /// Step the smoothing level and say where it landed.
    ///
    /// The toast names the level because smoothing has no visible effect until
    /// the next stroke is finished: without it the key would appear dead.
    fn announce_pen_smoothing(&mut self, delta: i32) {
        if !self.nudge_pen_smoothing(delta) {
            return;
        }
        let level = self.pen_smoothing;
        let max = crate::draw::shape::MAX_PEN_SMOOTHING;
        info!("Pen smoothing set to {level}/{max}");
        let message = if level == 0 {
            "Pen smoothing off".to_string()
        } else {
            format!("Pen smoothing {level}/{max}")
        };
        self.push_toast(ToastPriority::Info, "pen-smoothing", Toast::info(message));
    }

    pub(in crate::input::state) fn handle_tool_action(&mut self, action: Action) -> bool {
        if let Some(tool) = Tool::from_select_action(action) {
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
            Action::CycleFontFamily => {
                self.cycle_font_family();
            }
            Action::IncreasePenSmoothing => self.announce_pen_smoothing(1),
            Action::DecreasePenSmoothing => self.announce_pen_smoothing(-1),
            Action::ToggleEraserMode => {
                if self.toggle_eraser_mode() {
                    info!("Eraser mode set to {:?}", self.eraser_mode);
                }
            }
            Action::SelectSpotlightTool => {
                self.set_tool_override(Some(Tool::Spotlight));
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
            Action::CycleArrowStyle => {
                // Selected arrows are the target when there are any, so one
                // key both restyles what is on screen and sets what the next
                // arrow will be, without a modifier to remember.
                if self.selection_contains_arrow() {
                    self.cycle_selected_arrow_style_from_action();
                } else if self.cycle_arrow_style() {
                    let label = self.arrow_style.label();
                    info!("Arrow style set to {label}");
                    self.push_toast(
                        ToastPriority::Info,
                        "arrow-style",
                        Toast::info(format!("Arrow style: {label}")),
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
