use crate::ui::toolbar::ToolbarEvent;
use crate::{
    backend::wayland::toolbar::events::HitKind, backend::wayland::toolbar_intent::ToolbarIntent,
};

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

pub fn intent_for_hit(hit: &HitRegion, x: f64, y: f64) -> Option<(ToolbarIntent, bool)> {
    if !hit.contains(x, y) {
        return None;
    }

    let start_drag = matches!(
        hit.kind,
        HitKind::DragSetThickness { .. }
            | HitKind::DragSetMarkerOpacity { .. }
            | HitKind::DragSetFontSize
            | HitKind::PickColor { .. }
            | HitKind::DragUndoDelay
            | HitKind::DragRedoDelay
            | HitKind::DragCustomUndoDelay
            | HitKind::DragCustomRedoDelay
    );

    use crate::backend::wayland::toolbar::events::HitKind::*;
    use crate::ui::toolbar::ToolbarEvent;
    let event = match hit.kind {
        DragSetThickness { min, max } => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            let value = min + t * (max - min);
            ToolbarEvent::SetThickness(value)
        }
        DragSetMarkerOpacity { min, max } => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            let value = min + t * (max - min);
            ToolbarEvent::SetMarkerOpacity(value)
        }
        DragSetFontSize => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            let value = 8.0 + t * (72.0 - 8.0);
            ToolbarEvent::SetFontSize(value)
        }
        PickColor { x: px, y: py, w, h } => {
            let hue = ((x - px) / w).clamp(0.0, 1.0);
            let value = (1.0 - (y - py) / h).clamp(0.0, 1.0);
            ToolbarEvent::SetColor(crate::backend::wayland::toolbar::events::hsv_to_rgb(
                hue, 1.0, value,
            ))
        }
        DragUndoDelay => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            ToolbarEvent::SetUndoDelay(crate::backend::wayland::toolbar::events::delay_secs_from_t(
                t,
            ))
        }
        DragRedoDelay => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            ToolbarEvent::SetRedoDelay(crate::backend::wayland::toolbar::events::delay_secs_from_t(
                t,
            ))
        }
        DragCustomUndoDelay => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            ToolbarEvent::SetCustomUndoDelay(
                crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
            )
        }
        DragCustomRedoDelay => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            ToolbarEvent::SetCustomRedoDelay(
                crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
            )
        }
        crate::backend::wayland::toolbar::events::HitKind::Click => hit.event.clone(),
    };

    Some((ToolbarIntent(event), start_drag))
}

pub fn drag_intent_for_hit(hit: &HitRegion, x: f64, y: f64) -> Option<ToolbarIntent> {
    if !hit.contains(x, y) {
        return None;
    }

    use crate::backend::wayland::toolbar::events::HitKind::*;
    use crate::ui::toolbar::ToolbarEvent;
    match hit.kind {
        DragSetThickness { min, max } => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            let value = min + t * (max - min);
            Some(ToolbarIntent(ToolbarEvent::SetThickness(value)))
        }
        DragSetMarkerOpacity { min, max } => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            let value = min + t * (max - min);
            Some(ToolbarIntent(ToolbarEvent::SetMarkerOpacity(value)))
        }
        DragSetFontSize => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            let value = 8.0 + t * (72.0 - 8.0);
            Some(ToolbarIntent(ToolbarEvent::SetFontSize(value)))
        }
        PickColor { x: px, y: py, w, h } => {
            let hue = ((x - px) / w).clamp(0.0, 1.0);
            let value = (1.0 - (y - py) / h).clamp(0.0, 1.0);
            Some(ToolbarIntent(ToolbarEvent::SetColor(
                crate::backend::wayland::toolbar::events::hsv_to_rgb(hue, 1.0, value),
            )))
        }
        DragUndoDelay => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            Some(ToolbarIntent(ToolbarEvent::SetUndoDelay(
                crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
            )))
        }
        DragRedoDelay => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            Some(ToolbarIntent(ToolbarEvent::SetRedoDelay(
                crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
            )))
        }
        DragCustomUndoDelay => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            Some(ToolbarIntent(ToolbarEvent::SetCustomUndoDelay(
                crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
            )))
        }
        DragCustomRedoDelay => {
            let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
            Some(ToolbarIntent(ToolbarEvent::SetCustomRedoDelay(
                crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
            )))
        }
        _ => None,
    }
}
