use crate::ui::toolbar::ToolbarEvent;

#[derive(Clone, Debug)]
pub struct HitRegion {
    pub rect: (f64, f64, f64, f64), // x, y, w, h
    pub event: ToolbarEvent,
    pub kind: crate::backend::wayland::toolbar::events::HitKind,
    pub tooltip: Option<String>,
}

impl HitRegion {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.rect.0
            && x <= self.rect.0 + self.rect.2
            && y >= self.rect.1
            && y <= self.rect.1 + self.rect.3
    }
}
