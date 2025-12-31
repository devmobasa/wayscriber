use super::super::base::InputState;
use super::types::SelectionPropertyKind;

impl InputState {
    pub(crate) fn activate_properties_panel_entry(&mut self) -> bool {
        self.adjust_properties_panel_entry(0)
    }

    pub(crate) fn adjust_properties_panel_entry(&mut self, direction: i32) -> bool {
        let index = self.current_properties_focus_or_hover();
        let Some(index) = index else {
            return false;
        };

        self.apply_properties_entry(index, direction)
    }

    fn apply_properties_entry(&mut self, index: usize, direction: i32) -> bool {
        let entry = {
            let Some(panel) = self.shape_properties_panel.as_ref() else {
                return false;
            };
            let Some(entry) = panel.entries.get(index) else {
                return false;
            };
            if entry.disabled {
                return false;
            }
            entry.clone()
        };

        let changed = match entry.kind {
            SelectionPropertyKind::Color => self.apply_selection_color(direction),
            SelectionPropertyKind::Thickness => {
                self.apply_selection_thickness(direction_or_default(direction))
            }
            SelectionPropertyKind::Fill => self.apply_selection_fill(direction),
            SelectionPropertyKind::FontSize => {
                self.apply_selection_font_size(direction_or_default(direction))
            }
            SelectionPropertyKind::ArrowHead => self.apply_selection_arrow_head(direction),
            SelectionPropertyKind::ArrowLength => {
                self.apply_selection_arrow_length(direction_or_default(direction))
            }
            SelectionPropertyKind::ArrowAngle => {
                self.apply_selection_arrow_angle(direction_or_default(direction))
            }
            SelectionPropertyKind::TextBackground => {
                self.apply_selection_text_background(direction)
            }
        };

        if changed {
            self.refresh_properties_panel();
        }

        changed
    }
}

fn direction_or_default(direction: i32) -> i32 {
    // Treat activation (0) as a forward step; preserve negative direction.
    if direction < 0 { -1 } else { 1 }
}
