use super::structs::ToolbarSurface;
use crate::backend::wayland::toolbar::hit::{drag_intent_for_hit, intent_for_hit};
use crate::ui::toolbar::ToolbarEvent;

impl ToolbarSurface {
    pub fn hit_at(&self, x: f64, y: f64) -> Option<(ToolbarEvent, bool)> {
        self.hit_regions
            .iter()
            .find_map(|hit| intent_for_hit(hit, x, y))
    }

    pub fn drag_at(&self, x: f64, y: f64) -> Option<ToolbarEvent> {
        self.hit_regions
            .iter()
            .find_map(|hit| drag_intent_for_hit(hit, x, y))
    }
}
