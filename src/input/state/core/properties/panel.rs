use super::super::base::InputState;
use super::panel_layout::selection_panel_anchor;
use super::types::{PropertiesPanelLayout, ShapePropertiesPanel};
use super::utils::format_timestamp;

impl InputState {
    pub fn properties_panel(&self) -> Option<&ShapePropertiesPanel> {
        self.shape_properties_panel.as_ref()
    }

    pub fn properties_panel_layout(&self) -> Option<&PropertiesPanelLayout> {
        self.properties_panel_layout.as_ref()
    }

    pub fn is_properties_panel_open(&self) -> bool {
        self.shape_properties_panel.is_some()
    }

    pub fn close_properties_panel(&mut self) {
        if self.shape_properties_panel.take().is_some() {
            self.clear_properties_panel_layout();
            self.properties_panel_needs_refresh = false;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    pub(super) fn set_properties_panel(&mut self, panel: ShapePropertiesPanel) {
        self.shape_properties_panel = Some(panel);
        self.properties_panel_layout = None;
        self.pending_properties_hover_recalc = true;
        self.properties_panel_needs_refresh = false;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn show_properties_panel(&mut self) -> bool {
        if self.selected_shape_ids().is_empty() {
            return false;
        }

        let panel = (|| {
            let ids = self.selected_shape_ids();
            let frame = self.boards.active_frame();
            let anchor_rect = self.selection_bounding_box(ids);
            let anchor = selection_panel_anchor(anchor_rect, self.last_pointer_position);
            let entries = self.build_selection_property_entries(ids);

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
                return Some(ShapePropertiesPanel {
                    title: "Selection Properties".to_string(),
                    anchor,
                    anchor_rect,
                    lines,
                    entries,
                    hover_index: None,
                    keyboard_focus: None,
                    multiple_selection: true,
                });
            }

            let shape_id = *ids.first()?;
            let index = frame.find_index(shape_id)?;
            let drawn = frame.shape(shape_id)?;

            let mut lines = Vec::new();
            lines.push(format!("Shape ID: {shape_id}"));
            lines.push(format!("Type: {}", drawn.shape.kind_name()));
            lines.push(format!("Layer: {} of {}", index + 1, frame.shapes.len()));
            lines.push(format!(
                "Locked: {}",
                if drawn.locked { "Yes" } else { "No" }
            ));
            if let Some(timestamp) = format_timestamp(drawn.created_at) {
                lines.push(format!("Created: {timestamp}"));
            }
            if let Some(bounds) = drawn.shape.bounding_box() {
                lines.push(format!("Bounds: {}×{} px", bounds.width, bounds.height));
            }

            Some(ShapePropertiesPanel {
                title: "Shape Properties".to_string(),
                anchor,
                anchor_rect,
                lines,
                entries,
                hover_index: None,
                keyboard_focus: None,
                multiple_selection: false,
            })
        })();

        let Some(panel) = panel else {
            return false;
        };
        self.set_properties_panel(panel);
        true
    }

    pub(super) fn refresh_properties_panel(&mut self) {
        self.properties_panel_needs_refresh = false;
        let update = (|| {
            let ids = self.selected_shape_ids();
            if ids.is_empty() {
                return None;
            }

            let entries = self.build_selection_property_entries(ids);
            let frame = self.boards.active_frame();
            let anchor_rect = self.selection_bounding_box(ids);
            let anchor = selection_panel_anchor(anchor_rect, self.last_pointer_position);

            let (title, lines, multiple_selection) = if ids.len() > 1 {
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
                ("Selection Properties".to_string(), lines, true)
            } else {
                let shape_id = *ids.first()?;
                let index = frame.find_index(shape_id)?;
                let drawn = frame.shape(shape_id)?;
                let mut lines = Vec::new();
                lines.push(format!("Shape ID: {shape_id}"));
                lines.push(format!("Type: {}", drawn.shape.kind_name()));
                lines.push(format!("Layer: {} of {}", index + 1, frame.shapes.len()));
                lines.push(format!(
                    "Locked: {}",
                    if drawn.locked { "Yes" } else { "No" }
                ));
                if let Some(timestamp) = format_timestamp(drawn.created_at) {
                    lines.push(format!("Created: {timestamp}"));
                }
                if let Some(bounds) = drawn.shape.bounding_box() {
                    lines.push(format!("Bounds: {}×{} px", bounds.width, bounds.height));
                }
                ("Shape Properties".to_string(), lines, false)
            };

            Some((
                title,
                lines,
                entries,
                anchor_rect,
                anchor,
                multiple_selection,
            ))
        })();

        let Some((title, lines, entries, anchor_rect, anchor, multiple_selection)) = update else {
            self.close_properties_panel();
            return;
        };

        let Some(panel) = self.shape_properties_panel.as_mut() else {
            return;
        };
        panel.title = title;
        panel.lines = lines;
        panel.entries = entries;
        panel.anchor_rect = anchor_rect;
        panel.anchor = anchor;
        panel.multiple_selection = multiple_selection;

        let valid_focus = panel
            .keyboard_focus
            .filter(|idx| *idx < panel.entries.len())
            .filter(|idx| !panel.entries[*idx].disabled);
        panel.keyboard_focus = valid_focus;
        if panel.hover_index.is_some()
            && panel
                .hover_index
                .is_some_and(|idx| idx >= panel.entries.len())
        {
            panel.hover_index = None;
        }

        self.pending_properties_hover_recalc = true;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }
}
