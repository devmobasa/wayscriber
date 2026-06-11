use crate::backend::wayland::state::{color_log, debug_toolbar_color_logging_enabled};
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar_intent::ToolbarIntent;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui::toolbar::model::{ToolbarSlider, ToolbarSliderSpec, ToolbarSliderTarget};

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
            | HitKind::DragToolbarItem { .. }
    );

    use crate::backend::wayland::toolbar::events::HitKind::*;
    use crate::ui::toolbar::ToolbarEvent;
    let event = match hit.kind {
        DragSetThickness { min, max } => slider_event_for_hit(
            ToolbarSliderTarget::Thickness,
            ToolbarSliderSpec {
                min,
                max,
                step: ToolbarSliderSpec::THICKNESS.step,
            },
            hit,
            x,
        ),
        DragSetMarkerOpacity { min, max } => slider_event_for_hit(
            ToolbarSliderTarget::MarkerOpacity,
            ToolbarSliderSpec {
                min,
                max,
                step: ToolbarSliderSpec::MARKER_OPACITY.step,
            },
            hit,
            x,
        ),
        DragSetFontSize => slider_event_for_hit(
            ToolbarSliderTarget::FontSize,
            ToolbarSliderSpec::FONT_SIZE,
            hit,
            x,
        ),
        PickColor { x: px, y: py, w, h } => {
            let hue = ((x - px) / w).clamp(0.0, 1.0);
            let value = (1.0 - (y - py) / h).clamp(0.0, 1.0);
            let color = crate::backend::wayland::toolbar::events::hsv_to_rgb(hue, 1.0, value);
            if debug_toolbar_color_logging_enabled() {
                color_log(format!(
                    "toolbar pick color: pos=({:.1},{:.1}) picker=({:.1},{:.1},{:.1},{:.1}) hue={:.3} value={:.3} rgb=({:.3},{:.3},{:.3})",
                    x, y, px, py, w, h, hue, value, color.r, color.g, color.b
                ));
            }
            ToolbarEvent::SetColor(color)
        }
        DragUndoDelay => slider_event_for_hit(
            ToolbarSliderTarget::UndoDelay,
            ToolbarSliderSpec::DELAY_SECONDS,
            hit,
            x,
        ),
        DragRedoDelay => slider_event_for_hit(
            ToolbarSliderTarget::RedoDelay,
            ToolbarSliderSpec::DELAY_SECONDS,
            hit,
            x,
        ),
        DragCustomUndoDelay => slider_event_for_hit(
            ToolbarSliderTarget::CustomUndoDelay,
            ToolbarSliderSpec::DELAY_SECONDS,
            hit,
            x,
        ),
        DragCustomRedoDelay => slider_event_for_hit(
            ToolbarSliderTarget::CustomRedoDelay,
            ToolbarSliderSpec::DELAY_SECONDS,
            hit,
            x,
        ),
        DragMoveTop => ToolbarEvent::MoveTopToolbar { x, y },
        DragMoveSide => ToolbarEvent::MoveSideToolbar { x, y },
        DragToolbarItem { group, id, .. } => ToolbarEvent::StartToolbarItemDrag { group, id },
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
        DragSetThickness { min, max } => Some(ToolbarIntent(slider_event_for_hit(
            ToolbarSliderTarget::Thickness,
            ToolbarSliderSpec {
                min,
                max,
                step: ToolbarSliderSpec::THICKNESS.step,
            },
            hit,
            x,
        ))),
        DragSetMarkerOpacity { min, max } => Some(ToolbarIntent(slider_event_for_hit(
            ToolbarSliderTarget::MarkerOpacity,
            ToolbarSliderSpec {
                min,
                max,
                step: ToolbarSliderSpec::MARKER_OPACITY.step,
            },
            hit,
            x,
        ))),
        DragSetFontSize => Some(ToolbarIntent(slider_event_for_hit(
            ToolbarSliderTarget::FontSize,
            ToolbarSliderSpec::FONT_SIZE,
            hit,
            x,
        ))),
        PickColor { x: px, y: py, w, h } => {
            let hue = ((x - px) / w).clamp(0.0, 1.0);
            let value = (1.0 - (y - py) / h).clamp(0.0, 1.0);
            let color = crate::backend::wayland::toolbar::events::hsv_to_rgb(hue, 1.0, value);
            if debug_toolbar_color_logging_enabled() {
                color_log(format!(
                    "toolbar drag color: pos=({:.1},{:.1}) picker=({:.1},{:.1},{:.1},{:.1}) hue={:.3} value={:.3} rgb=({:.3},{:.3},{:.3})",
                    x, y, px, py, w, h, hue, value, color.r, color.g, color.b
                ));
            }
            Some(ToolbarIntent(ToolbarEvent::SetColor(color)))
        }
        DragUndoDelay => Some(ToolbarIntent(slider_event_for_hit(
            ToolbarSliderTarget::UndoDelay,
            ToolbarSliderSpec::DELAY_SECONDS,
            hit,
            x,
        ))),
        DragRedoDelay => Some(ToolbarIntent(slider_event_for_hit(
            ToolbarSliderTarget::RedoDelay,
            ToolbarSliderSpec::DELAY_SECONDS,
            hit,
            x,
        ))),
        DragCustomUndoDelay => Some(ToolbarIntent(slider_event_for_hit(
            ToolbarSliderTarget::CustomUndoDelay,
            ToolbarSliderSpec::DELAY_SECONDS,
            hit,
            x,
        ))),
        DragCustomRedoDelay => Some(ToolbarIntent(slider_event_for_hit(
            ToolbarSliderTarget::CustomRedoDelay,
            ToolbarSliderSpec::DELAY_SECONDS,
            hit,
            x,
        ))),
        DragMoveTop => Some(ToolbarIntent(ToolbarEvent::MoveTopToolbar { x, y })),
        DragMoveSide => Some(ToolbarIntent(ToolbarEvent::MoveSideToolbar { x, y })),
        DragToolbarItem {
            group,
            target_index,
            ..
        } => Some(ToolbarIntent(ToolbarEvent::DragToolbarItemOver {
            group,
            target_index,
        })),
        _ => None,
    }
}

fn slider_event_for_hit(
    target: ToolbarSliderTarget,
    spec: ToolbarSliderSpec,
    hit: &HitRegion,
    pointer_x: f64,
) -> ToolbarEvent {
    ToolbarSlider {
        target,
        spec,
        value: spec.min,
    }
    .event_for_pointer_x(pointer_x, hit.rect.0, hit.rect.2)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn click(event: ToolbarEvent) -> HitRegion {
        HitRegion {
            rect: (10.0, 20.0, 30.0, 40.0),
            event,
            kind: HitKind::Click,
            tooltip: None,
        }
    }

    fn thickness_slider() -> HitRegion {
        HitRegion {
            rect: (100.0, 0.0, 200.0, 20.0),
            event: ToolbarEvent::SetThickness(1.0),
            kind: HitKind::DragSetThickness {
                min: 10.0,
                max: 20.0,
            },
            tooltip: None,
        }
    }

    fn assert_set_thickness(event: ToolbarEvent, expected: f64) {
        match event {
            ToolbarEvent::SetThickness(value) => {
                assert!(
                    (value - expected).abs() < 0.000_001,
                    "expected {expected}, got {value}"
                );
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn slider_press_and_drag_use_same_pointer_mapping() {
        let hit = thickness_slider();

        let (press, start_drag) = intent_for_hit(&hit, 200.0, 10.0).expect("press intent");
        let drag = drag_intent_for_hit(&hit, 200.0, 10.0).expect("drag intent");

        assert!(start_drag);
        assert_set_thickness(press.0, 15.0);
        assert_set_thickness(drag.0, 15.0);
    }

    #[test]
    fn slider_pointer_mapping_clamps_to_hit_rect() {
        let hit = thickness_slider();

        let (left, _) = intent_for_hit(&hit, 100.0, 10.0).expect("left intent");
        let (right, _) = intent_for_hit(&hit, 300.0, 10.0).expect("right intent");

        assert_set_thickness(left.0, 10.0);
        assert_set_thickness(right.0, 20.0);
        assert!(intent_for_hit(&hit, 99.0, 10.0).is_none());
        assert!(drag_intent_for_hit(&hit, 301.0, 10.0).is_none());
    }

    #[test]
    fn focus_traversal_uses_click_hits_only() {
        let hits = vec![
            click(ToolbarEvent::Undo),
            thickness_slider(),
            click(ToolbarEvent::Redo),
        ];

        assert_eq!(next_focus_index(&hits, None, false), Some(0));
        assert_eq!(next_focus_index(&hits, Some(0), false), Some(2));
        assert_eq!(next_focus_index(&hits, Some(2), false), Some(0));
        assert_eq!(next_focus_index(&hits, None, true), Some(2));
        assert_eq!(next_focus_index(&hits, Some(2), true), Some(0));
    }

    #[test]
    fn focused_event_returns_the_focused_hit_event() {
        let hits = vec![
            click(ToolbarEvent::Undo),
            thickness_slider(),
            click(ToolbarEvent::Redo),
        ];

        assert!(matches!(
            focused_event(&hits, Some(0)),
            Some(ToolbarEvent::Undo)
        ));
        assert!(matches!(
            focused_event(&hits, Some(2)),
            Some(ToolbarEvent::Redo)
        ));
        assert!(focused_event(&hits, None).is_none());
    }
}
