use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar_intent::ToolbarIntent;
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
            | HitKind::DragMoveTop
            | HitKind::DragMoveSide
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
        DragMoveTop => ToolbarEvent::MoveTopToolbar { x, y },
        DragMoveSide => ToolbarEvent::MoveSideToolbar { x, y },
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
        DragMoveTop => Some(ToolbarIntent(ToolbarEvent::MoveTopToolbar { x, y })),
        DragMoveSide => Some(ToolbarIntent(ToolbarEvent::MoveSideToolbar { x, y })),
        _ => None,
    }
}

fn focusable_indices(hits: &[HitRegion]) -> Vec<usize> {
    hits.iter()
        .enumerate()
        .filter(|(_, hit)| matches!(hit.kind, HitKind::Click))
        .map(|(idx, _)| idx)
        .collect()
}

pub fn next_focus_index(
    hits: &[HitRegion],
    current: Option<usize>,
    reverse: bool,
) -> Option<usize> {
    let indices = focusable_indices(hits);
    if indices.is_empty() {
        return None;
    }
    let pos = current.and_then(|idx| indices.iter().position(|entry| *entry == idx));
    let next_pos = match (pos, reverse) {
        (Some(pos), false) => (pos + 1) % indices.len(),
        (Some(pos), true) => (pos + indices.len() - 1) % indices.len(),
        (None, false) => 0,
        (None, true) => indices.len() - 1,
    };
    indices.get(next_pos).copied()
}

pub fn focus_hover_point(hits: &[HitRegion], focus: Option<usize>) -> Option<(f64, f64)> {
    let hit = focus.and_then(|idx| hits.get(idx))?;
    Some((hit.rect.0 + hit.rect.2 / 2.0, hit.rect.1 + hit.rect.3 / 2.0))
}

pub fn focused_event(hits: &[HitRegion], focus: Option<usize>) -> Option<ToolbarEvent> {
    focus
        .and_then(|idx| hits.get(idx))
        .map(|hit| hit.event.clone())
}
