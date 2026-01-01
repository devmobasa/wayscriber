use super::super::super::base::InputState;

impl InputState {
    pub fn properties_panel_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.properties_panel_layout?;
        let panel = self.shape_properties_panel.as_ref()?;
        if panel.entries.is_empty() {
            return None;
        }

        let local_x = x as f64 - layout.origin_x;
        let local_y = y as f64 - layout.origin_y;
        if local_x < 0.0 || local_y < 0.0 || local_x > layout.width || local_y > layout.height {
            return None;
        }

        let row_y = y as f64 - layout.entry_start_y;
        if row_y < 0.0 {
            return None;
        }
        let index = (row_y / layout.entry_row_height).floor() as usize;
        if index >= panel.entries.len() {
            None
        } else {
            Some(index)
        }
    }

    pub(super) fn update_properties_panel_hover_from_pointer_internal(
        &mut self,
        x: i32,
        y: i32,
        trigger_redraw: bool,
    ) {
        let new_hover = self.properties_panel_index_at(x, y);
        let Some(panel) = self.shape_properties_panel.as_mut() else {
            return;
        };

        let new_hover = new_hover.filter(|idx| *idx < panel.entries.len());
        let new_hover = new_hover.filter(|idx| !panel.entries[*idx].disabled);

        if panel.hover_index != new_hover {
            panel.hover_index = new_hover;
            if trigger_redraw {
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
        }
    }

    pub fn update_properties_panel_hover_from_pointer(&mut self, x: i32, y: i32) {
        self.update_properties_panel_hover_from_pointer_internal(x, y, true);
    }
}
