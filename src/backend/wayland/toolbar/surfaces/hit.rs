use super::structs::ToolbarSurface;
use crate::backend::wayland::toolbar::hit::{
    drag_intent_for_hit, intent_for_hit, quick_color_slot_for_hit,
};
use crate::backend::wayland::toolbar_intent::ToolbarIntent;

impl ToolbarSurface {
    pub fn hit_at(&self, x: f64, y: f64) -> Option<(ToolbarIntent, bool)> {
        self.hit_regions
            .iter()
            .find_map(|hit| intent_for_hit(hit, x, y))
    }

    /// The quick-color slot under the pointer, read from the same regions the
    /// primary path uses so the recolor gesture cannot drift from what is drawn.
    pub fn quick_color_slot_at(&self, x: f64, y: f64) -> Option<usize> {
        self.hit_regions
            .iter()
            .find_map(|hit| quick_color_slot_for_hit(hit, x, y))
    }

    pub fn drag_at(&self, x: f64, y: f64) -> Option<ToolbarIntent> {
        self.hit_regions
            .iter()
            .find_map(|hit| drag_intent_for_hit(hit, x, y))
    }
}
