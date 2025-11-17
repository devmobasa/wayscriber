use super::base::InputState;
use chrono::{DateTime, Local, Utc};

#[derive(Debug, Clone)]
pub struct ShapePropertiesPanel {
    pub title: String,
    pub anchor: (f64, f64),
    pub lines: Vec<String>,
    pub multiple_selection: bool,
}

impl InputState {
    pub fn properties_panel(&self) -> Option<&ShapePropertiesPanel> {
        self.shape_properties_panel.as_ref()
    }

    pub fn close_properties_panel(&mut self) {
        if self.shape_properties_panel.take().is_some() {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    pub(super) fn set_properties_panel(&mut self, panel: ShapePropertiesPanel) {
        self.shape_properties_panel = Some(panel);
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(super) fn show_properties_panel(&mut self) -> bool {
        let ids = self.selected_shape_ids();
        if ids.is_empty() {
            return false;
        }

        let frame = self.canvas_set.active_frame();
        let anchor_rect = self.selection_bounding_box(ids);
        let anchor = anchor_rect
            .map(|rect| {
                (
                    (rect.x + rect.width + 12) as f64,
                    (rect.y - 12).max(12) as f64,
                )
            })
            .unwrap_or_else(|| {
                let (px, py) = self.last_pointer_position;
                ((px + 16) as f64, (py - 16) as f64)
            });

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
            self.set_properties_panel(ShapePropertiesPanel {
                title: "Selection Summary".to_string(),
                anchor,
                lines,
                multiple_selection: true,
            });
            return true;
        }

        let shape_id = ids[0];
        let index = match frame.find_index(shape_id) {
            Some(idx) => idx,
            None => return false,
        };
        let drawn = match frame.shape(shape_id) {
            Some(shape) => shape,
            None => return false,
        };

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

        self.set_properties_panel(ShapePropertiesPanel {
            title: "Shape Properties".to_string(),
            anchor,
            lines,
            multiple_selection: false,
        });
        true
    }
}

fn format_timestamp(ms: u64) -> Option<String> {
    let seconds = (ms / 1000) as i64;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    let utc_dt = DateTime::<Utc>::from_timestamp(seconds, nanos)?;
    let local_dt = utc_dt.with_timezone(&Local);
    Some(local_dt.format("%Y-%m-%d %H:%M:%S").to_string())
}
