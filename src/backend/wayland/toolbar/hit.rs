use crate::backend::wayland::state::{color_log, debug_toolbar_color_logging_enabled};
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar_intent::ToolbarIntent;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui::toolbar::model::{ToolbarSlider, ToolbarSliderSpec, ToolbarSliderTarget};

/// Minimum pointer-target size in logical pixels. Visual rects stay as
/// drawn; hit-testing inflates smaller rects to at least this square so
/// small chrome (collapse chevrons, drag grips, micro-buttons) stays
/// comfortably clickable with mouse, stylus, and touch. Inflation is
/// bounded by the toolbar surface, so it never leaks onto the canvas.
pub const MIN_HIT_TARGET: f64 = 24.0;

#[derive(Clone, Debug)]
pub struct HitRegion {
    /// Stable tree identity for keyboard focus. Hand-rendered side hits do
    /// not have one yet and fall back to their visual-order index.
    pub focus_id: Option<String>,
    pub rect: (f64, f64, f64, f64), // x, y, w, h
    pub event: ToolbarEvent,
    pub kind: crate::backend::wayland::toolbar::events::HitKind,
    pub tooltip: Option<String>,
}

/// Row-style hits at least this long (section headers, sliders, list rows)
/// are already easy targets along their major axis and usually abut the
/// neighboring row, so they are not inflated across their minor axis.
const ROW_HIT_LENGTH: f64 = MIN_HIT_TARGET * 3.0;

/// Shared pointer-target predicate: true when (x, y) lands inside `rect`
/// inflated to the minimum target size. Used by both the legacy HitRegion
/// path and the view-engine tree so the two can never disagree.
pub fn rect_contains_with_min_target(rect: (f64, f64, f64, f64), x: f64, y: f64) -> bool {
    let (rx, ry, rw, rh) = rect;
    let pad_x = if rh < ROW_HIT_LENGTH {
        ((MIN_HIT_TARGET - rw) / 2.0).max(0.0)
    } else {
        0.0
    };
    let pad_y = if rw < ROW_HIT_LENGTH {
        ((MIN_HIT_TARGET - rh) / 2.0).max(0.0)
    } else {
        0.0
    };
    x >= rx - pad_x && x <= rx + rw + pad_x && y >= ry - pad_y && y <= ry + rh + pad_y
}

impl HitRegion {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        rect_contains_with_min_target(self.rect, x, y)
    }
}

/// Clip a hit region to visible surface content. Small clipped targets are
/// expanded inward (never outside `bounds`) so the shared minimum-target
/// predicate cannot make them clickable through fixed chrome or the canvas.
pub fn clip_hit_region_to_bounds(hit: &mut HitRegion, bounds: (f64, f64, f64, f64)) -> bool {
    let (bx, by, bw, bh) = bounds;
    let (rx, ry, rw, rh) = hit.rect;
    let x0 = rx.max(bx);
    let y0 = ry.max(by);
    let x1 = (rx + rw).min(bx + bw);
    let y1 = (ry + rh).min(by + bh);
    if x1 <= x0 || y1 <= y0 {
        return false;
    }

    let mut x = x0;
    let mut y = y0;
    let mut w = x1 - x0;
    let mut h = y1 - y0;
    if h < ROW_HIT_LENGTH && w < MIN_HIT_TARGET && bw >= MIN_HIT_TARGET {
        let center = x + w / 2.0;
        w = MIN_HIT_TARGET;
        x = (center - w / 2.0).clamp(bx, bx + bw - w);
    }
    if w < ROW_HIT_LENGTH && h < MIN_HIT_TARGET && bh >= MIN_HIT_TARGET {
        let center = y + h / 2.0;
        h = MIN_HIT_TARGET;
        y = (center - h / 2.0).clamp(by, by + bh - h);
    }
    hit.rect = (x, y, w, h);
    true
}

pub fn clip_hit_regions_to_bounds(
    hits: &mut Vec<HitRegion>,
    start: usize,
    bounds: (f64, f64, f64, f64),
) {
    let mut index = start;
    while index < hits.len() {
        if clip_hit_region_to_bounds(&mut hits[index], bounds) {
            index += 1;
        } else {
            hits.remove(index);
        }
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
            | HitKind::PickSatVal { .. }
            | HitKind::PickHue { .. }
            | HitKind::DragUndoDelay
            | HitKind::DragRedoDelay
            | HitKind::DragCustomUndoDelay
            | HitKind::DragCustomRedoDelay
            | HitKind::DragMoveTop
            | HitKind::DragMoveSide
            | HitKind::DragScrollSide { .. }
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
        PickSatVal { hue } => sat_val_event_for_hit(hue, hit, x, y),
        PickHue { sat, val } => hue_event_for_hit(sat, val, hit, x),
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
        DragScrollSide { max_scroll } => scroll_event_for_hit(max_scroll, hit, y),
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
        PickSatVal { hue } => Some(ToolbarIntent(sat_val_event_for_hit(hue, hit, x, y))),
        PickHue { sat, val } => Some(ToolbarIntent(hue_event_for_hit(sat, val, hit, x))),
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
        DragScrollSide { max_scroll } => {
            Some(ToolbarIntent(scroll_event_for_hit(max_scroll, hit, y)))
        }
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

/// Map a pointer position inside the saturation/value area to a full HSV
/// color; saturation follows x, value follows inverted y, hue is fixed.
fn sat_val_event_for_hit(hue: f64, hit: &HitRegion, x: f64, y: f64) -> ToolbarEvent {
    let s = ((x - hit.rect.0) / hit.rect.2.max(1.0)).clamp(0.0, 1.0);
    let v = (1.0 - (y - hit.rect.1) / hit.rect.3.max(1.0)).clamp(0.0, 1.0);
    if debug_toolbar_color_logging_enabled() {
        color_log(format!(
            "toolbar pick sat/val: pos=({x:.1},{y:.1}) rect={:?} h={hue:.3} s={s:.3} v={v:.3}",
            hit.rect
        ));
    }
    ToolbarEvent::SetColorHsv { h: hue, s, v }
}

/// Map a pointer x inside the hue bar to a full HSV color; hue follows x,
/// saturation and value are fixed.
fn hue_event_for_hit(sat: f64, val: f64, hit: &HitRegion, x: f64) -> ToolbarEvent {
    let h = ((x - hit.rect.0) / hit.rect.2.max(1.0)).clamp(0.0, 1.0);
    if debug_toolbar_color_logging_enabled() {
        color_log(format!(
            "toolbar pick hue: x={x:.1} rect={:?} h={h:.3} s={sat:.3} v={val:.3}",
            hit.rect
        ));
    }
    ToolbarEvent::SetColorHsv { h, s: sat, v: val }
}

/// Map a pointer y within the scrollbar track to an absolute scroll offset.
fn scroll_event_for_hit(max_scroll: f64, hit: &HitRegion, pointer_y: f64) -> ToolbarEvent {
    let track_y = hit.rect.1;
    let track_h = hit.rect.3.max(1.0);
    let fraction = ((pointer_y - track_y) / track_h).clamp(0.0, 1.0);
    ToolbarEvent::ScrollSidePane(fraction * max_scroll)
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
    let mut indices: Vec<_> = hits
        .iter()
        .enumerate()
        .filter(|(_, hit)| matches!(hit.kind, HitKind::Click))
        .map(|(idx, _)| idx)
        .collect();
    // Hit vectors are pointer-z ordered (topmost first), while keyboard
    // focus follows visual reading order. Choose the major visual axis so
    // vertically centered swatches/chrome do not scramble a horizontal bar.
    let centers: Vec<_> = indices
        .iter()
        .map(|index| {
            let (x, y, w, h) = hits[*index].rect;
            (x + w / 2.0, y + h / 2.0)
        })
        .collect();
    let x_span = centers
        .iter()
        .map(|center| center.0)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
            (min.min(value), max.max(value))
        });
    let y_span = centers
        .iter()
        .map(|center| center.1)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
            (min.min(value), max.max(value))
        });
    let horizontal = x_span.1 - x_span.0 >= y_span.1 - y_span.0;
    indices.sort_by(|left, right| {
        let a = hits[*left].rect;
        let b = hits[*right].rect;
        if horizontal {
            a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1))
        } else {
            a.1.total_cmp(&b.1).then_with(|| a.0.total_cmp(&b.0))
        }
    });
    indices
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

pub fn resolve_focus_index(
    hits: &[HitRegion],
    index: Option<usize>,
    focus_id: Option<&str>,
) -> Option<usize> {
    if let Some(focus_id) = focus_id {
        return hits
            .iter()
            .position(|hit| hit.focus_id.as_deref() == Some(focus_id));
    }
    index.filter(|index| *index < hits.len())
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
            focus_id: None,
            rect: (10.0, 20.0, 30.0, 40.0),
            event,
            kind: HitKind::Click,
            tooltip: None,
        }
    }

    fn thickness_slider() -> HitRegion {
        HitRegion {
            focus_id: None,
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
    fn small_hit_rects_inflate_to_min_target() {
        let hit = HitRegion {
            focus_id: None,
            rect: (100.0, 100.0, 14.0, 14.0),
            event: ToolbarEvent::Undo,
            kind: HitKind::Click,
            tooltip: None,
        };

        // 14x14 inflates by 5px on each side to reach 24x24.
        assert!(hit.contains(96.0, 96.0));
        assert!(hit.contains(118.0, 118.0));
        assert!(!hit.contains(94.0, 107.0));
        assert!(!hit.contains(107.0, 120.0));
    }

    #[test]
    fn clipped_hit_cannot_reach_outside_visible_bounds() {
        let mut hit = click(ToolbarEvent::Undo);
        hit.rect = (10.0, 15.0, 12.0, 12.0);
        assert!(clip_hit_region_to_bounds(
            &mut hit,
            (0.0, 20.0, 100.0, 40.0)
        ));

        assert!(hit.rect.1 >= 20.0);
        assert!(hit.rect.1 + hit.rect.3 <= 60.0);
        assert!(!hit.contains(hit.rect.0 + hit.rect.2 / 2.0, 19.9));
        assert!(hit.contains(hit.rect.0 + hit.rect.2 / 2.0, 20.0));
    }

    #[test]
    fn large_hit_rects_do_not_inflate() {
        let hit = HitRegion {
            focus_id: None,
            rect: (100.0, 100.0, 40.0, 40.0),
            event: ToolbarEvent::Undo,
            kind: HitKind::Click,
            tooltip: None,
        };

        assert!(hit.contains(100.0, 100.0));
        assert!(hit.contains(140.0, 140.0));
        assert!(!hit.contains(99.0, 120.0));
        assert!(!hit.contains(120.0, 141.0));
    }

    #[test]
    fn row_hits_do_not_inflate_across_their_minor_axis() {
        // A full-width 21px section-header row must not swallow the first
        // body row's boundary below it.
        let hit = HitRegion {
            focus_id: None,
            rect: (10.0, 100.0, 236.0, 21.0),
            event: ToolbarEvent::Undo,
            kind: HitKind::Click,
            tooltip: None,
        };

        assert!(hit.contains(120.0, 100.0));
        assert!(hit.contains(120.0, 121.0));
        assert!(!hit.contains(120.0, 122.0));
        assert!(!hit.contains(120.0, 99.0));
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
    fn sat_val_hit_maps_pointer_to_full_hsv_color() {
        let hit = HitRegion {
            focus_id: None,
            rect: (100.0, 50.0, 200.0, 80.0),
            event: ToolbarEvent::SetColorHsv {
                h: 0.5,
                s: 0.5,
                v: 0.5,
            },
            kind: HitKind::PickSatVal { hue: 0.5 },
            tooltip: None,
        };

        let (intent, start_drag) = intent_for_hit(&hit, 200.0, 90.0).expect("press intent");
        assert!(start_drag);
        match intent.0 {
            ToolbarEvent::SetColorHsv { h, s, v } => {
                assert!((h - 0.5).abs() < 1e-9);
                assert!((s - 0.5).abs() < 1e-9);
                assert!((v - 0.5).abs() < 1e-9);
            }
            other => panic!("unexpected event: {other:?}"),
        }

        // The top-right corner is full saturation and value; the mapping
        // derives from the hit rect itself, not an embedded payload rect.
        let drag = drag_intent_for_hit(&hit, 300.0, 50.0).expect("drag intent");
        match drag.0 {
            ToolbarEvent::SetColorHsv { h, s, v } => {
                assert!((h - 0.5).abs() < 1e-9);
                assert!((s - 1.0).abs() < 1e-9);
                assert!((v - 1.0).abs() < 1e-9);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn hue_hit_maps_pointer_x_and_keeps_sat_val() {
        let hit = HitRegion {
            focus_id: None,
            rect: (100.0, 150.0, 200.0, 14.0),
            event: ToolbarEvent::SetColorHsv {
                h: 0.0,
                s: 0.25,
                v: 0.75,
            },
            kind: HitKind::PickHue {
                sat: 0.25,
                val: 0.75,
            },
            tooltip: None,
        };

        let (intent, start_drag) = intent_for_hit(&hit, 150.0, 157.0).expect("press intent");
        assert!(start_drag);
        match intent.0 {
            ToolbarEvent::SetColorHsv { h, s, v } => {
                assert!((h - 0.25).abs() < 1e-9);
                assert!((s - 0.25).abs() < 1e-9);
                assert!((v - 0.75).abs() < 1e-9);
            }
            other => panic!("unexpected event: {other:?}"),
        }
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
    fn stable_focus_id_survives_hit_reordering() {
        let mut undo = click(ToolbarEvent::Undo);
        undo.focus_id = Some("top.utility.undo".to_string());
        let mut redo = click(ToolbarEvent::Redo);
        redo.focus_id = Some("top.utility.redo".to_string());
        let hits = vec![redo, undo];

        assert_eq!(
            resolve_focus_index(&hits, Some(0), Some("top.utility.undo")),
            Some(1)
        );
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
