use log::warn;

use crate::draw::Shape;
use crate::draw::frame::UndoAction;
use crate::draw::shape::bounding_box_for_points;
use crate::input::tool::{FinishedToolStroke, PolygonStrokeSnapshot, ToolStrokeSnapshot};
use crate::input::{InputState, Tool};
use crate::util::Rect;

const FINISHED_PATH_DAMAGE_MAX_SPAN: f64 = 128.0;

pub(super) struct DrawingRelease {
    pub(super) start: (i32, i32),
    pub(super) end: (i32, i32),
    pub(super) points: Vec<(i32, i32)>,
    pub(super) point_thicknesses: Vec<f32>,
}

pub(super) fn finish_drawing(state: &mut InputState, tool: Tool, release: DrawingRelease) {
    state.mark_draw_activity();
    let drawing_color = state.active_drag_color_or_tool(tool);
    let drawing_thickness = state.thickness_for_tool(tool);
    let pressure_preview_exceeds_final_width = pressure_preview_exceeds_final_freehand_width(
        release.points.len(),
        &release.point_thicknesses,
        drawing_thickness,
    );
    // Smoothing commits a different path from the one the preview drew, so the
    // preview's pixels can sit outside the committed shape's damage and stay on
    // screen as a ghost: rendering clears only the damage clip.
    //
    // Damage the raw path's own split regions rather than its bounding box. A
    // long diagonal stroke's box is nearly the whole screen, and re-marking it
    // would undo the split-damage work that `finished_path_damage_regions`
    // exists to do. Computed here because the snapshot takes the points next.
    let raw_preview_damage = (state.pen_smoothing > 0)
        .then(|| {
            raw_preview_damage_regions(
                &release.points,
                drawing_thickness,
                &release.point_thicknesses,
            )
        })
        .flatten();
    let finished = if tool.polygon_template().is_some() {
        let snapshot = PolygonStrokeSnapshot {
            tool,
            start: release.start,
            end: release.end,
            color: drawing_color,
            size: drawing_thickness,
            fill_enabled: state.fill_enabled,
            regular_sides: state.polygon_sides,
        };
        tool.finish_polygon_stroke(snapshot)
    } else {
        let snapshot = ToolStrokeSnapshot {
            tool,
            start: release.start,
            end: release.end,
            points: release.points,
            point_thicknesses: release.point_thicknesses,
            color: drawing_color,
            size: drawing_thickness,
            marker_opacity: state.marker_opacity,
            fill_enabled: state.fill_enabled,
            blur_style: state.blur_style,
            spotlight_magnification: state.spotlight_magnification,
            arrow_length: state.arrow_length,
            arrow_angle: state.arrow_angle,
            arrow_head_at_end: state.arrow_head_at_end,
            arrow_style: state.arrow_style,
            arrow_label: state.next_arrow_label(),
            step_marker_label: state.next_step_marker_label(),
            eraser_mode: state.eraser_mode,
            eraser_size: state.eraser_size,
            eraser_kind: state.eraser_kind,
            pressure_variation_threshold: state.pressure_variation_threshold,
            pen_smoothing: state.pen_smoothing,
        };
        tool.finish_stroke(snapshot)
    };

    let (shape, usage) = match finished {
        FinishedToolStroke::Shape { shape, usage } => (shape, usage),
        FinishedToolStroke::EraseStroke { path } => {
            state.clear_provisional_dirty();
            if state.erase_strokes_by_points(&path) {
                state.mark_session_dirty();
            }
            return;
        }
        FinishedToolStroke::Noop => {
            state.clear_provisional_dirty();
            return;
        }
    };

    let bounds = shape.bounding_box();
    let magnified_spotlight = matches!(
        shape,
        Shape::Spotlight { magnification, .. }
            if crate::draw::spotlight_magnification_is_active(magnification)
    );
    let path_damage = finished_path_damage_regions(&shape, bounds);
    // `Shape::Freehand` only, deliberately. This covers the case where a
    // pressure preview drew wide samples and the release then *downgraded* to a
    // plain Freehand at the tool's own thickness, leaving the preview wider than
    // anything the committed shape damages. A committed `FreehandPressure` keeps
    // the sampled thicknesses it was drawn with, so its own damage is already as
    // wide as the preview was and it needs no help here.
    let preserve_provisional_cleanup =
        matches!(shape, Shape::Freehand { .. }) && pressure_preview_exceeds_final_width;

    let mut limit_reached = false;
    let addition = {
        let frame = state.boards.active_frame_mut();
        match frame.try_add_shape_with_id(shape.clone(), state.max_shapes_per_frame) {
            Some(new_id) => {
                if let Some(index) = frame.find_index(new_id) {
                    if let Some(new_shape) = frame.shape(new_id) {
                        let snapshot = new_shape.clone();
                        frame.push_undo_action(
                            UndoAction::Create {
                                shapes: vec![(index, snapshot.clone())],
                            },
                            state.undo_stack_limit,
                        );
                        Some((new_id, snapshot))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            None => {
                limit_reached = true;
                None
            }
        }
    };

    if let Some((new_id, _snapshot)) = addition {
        state.invalidate_hit_cache_for(new_id);
        if let Some(path_damage) = path_damage {
            let provisional_bounds = state.take_provisional_dirty_bounds();
            for region in path_damage {
                state.dirty_tracker.mark_rect(region);
            }
            // Only present when smoothing moved the path out from under what
            // the preview drew.
            for region in raw_preview_damage.into_iter().flatten() {
                state.dirty_tracker.mark_rect(region);
            }
            if preserve_provisional_cleanup {
                state.dirty_tracker.mark_optional_rect(provisional_bounds);
            }
        } else {
            state.clear_provisional_dirty();
            state.dirty_tracker.mark_optional_rect(bounds);
        }
        state.clear_selection();
        state.needs_redraw = true;
        state.mark_session_dirty();
        state.record_first_stroke_done_for_onboarding();
        if magnified_spotlight {
            state.request_spotlight_magnifier_feedback();
        }
        if usage.bump_arrow_label {
            state.bump_arrow_label();
        }
        if usage.bump_step_marker {
            state.bump_step_marker();
        }
    } else {
        state.clear_provisional_dirty();
        if limit_reached {
            warn!(
                "Shape limit ({}) reached; discarding new shape",
                state.max_shapes_per_frame
            );
        }
    }
}

/// Split damage covering the raw path a live preview drew, or `None` when the
/// path is too short to have drawn anything.
///
/// The width has to be the widest the preview could have drawn, not the width
/// the release settled on. A tablet preview draws each sample at its own
/// pressure width, and a hard press early in a stroke can be many times the
/// tool's thickness; damaging only the tool width would leave the outer edge of
/// that press on screen once smoothing moved the path out from under it.
///
/// The result is inflated the way the marker's own damage is, so one
/// calculation covers the widest preview any path tool draws.
fn raw_preview_damage_regions(
    points: &[(i32, i32)],
    thickness: f64,
    point_thicknesses: &[f32],
) -> Option<Vec<Rect>> {
    if points.len() < 2 {
        return None;
    }
    let widest = point_thicknesses
        .iter()
        .fold(thickness, |widest, &sample| widest.max(f64::from(sample)));
    let width = (widest * 1.35).max(widest + 1.0);
    let fallback = bounding_box_for_points(points, width)?;
    Some(split_path_damage_regions(points, width, fallback))
}

fn pressure_preview_exceeds_final_freehand_width(
    point_count: usize,
    point_thicknesses: &[f32],
    final_width: f64,
) -> bool {
    point_thicknesses.len() == point_count
        && point_thicknesses
            .iter()
            .any(|&thickness| f64::from(thickness) > final_width)
}

fn finished_path_damage_regions(shape: &Shape, fallback: Option<Rect>) -> Option<Vec<Rect>> {
    match shape {
        Shape::Freehand { points, thick, .. } => {
            Some(split_path_damage_regions(points, *thick, fallback?))
        }
        Shape::FreehandPressure { points, .. } => {
            let max_thick = points
                .iter()
                .fold(1.0f64, |max, &(_, _, thickness)| max.max(thickness as f64));
            let points = points.iter().map(|&(x, y, _)| (x, y)).collect::<Vec<_>>();
            Some(split_path_damage_regions(&points, max_thick, fallback?))
        }
        Shape::MarkerStroke { points, thick, .. } => {
            let inflated = (*thick * 1.35).max(*thick + 1.0);
            Some(split_path_damage_regions(points, inflated, fallback?))
        }
        Shape::EraserStroke { points, brush } => Some(split_path_damage_regions(
            points,
            brush.size.max(1.0),
            fallback?,
        )),
        _ => None,
    }
}

fn split_path_damage_regions(
    points: &[(i32, i32)],
    stroke_width: f64,
    fallback: Rect,
) -> Vec<Rect> {
    if points.len() < 2 {
        return vec![fallback];
    }

    let mut regions = Vec::new();
    for segment in points.windows(2) {
        append_segment_damage_regions(segment[0], segment[1], stroke_width, &mut regions);
    }

    if regions.is_empty() {
        vec![fallback]
    } else {
        regions
    }
}

fn append_segment_damage_regions(
    start: (i32, i32),
    end: (i32, i32),
    stroke_width: f64,
    regions: &mut Vec<Rect>,
) {
    if start == end {
        if let Some(region) = bounding_box_for_points(&[start], stroke_width) {
            regions.push(region);
        }
        return;
    }

    let dx = f64::from(end.0) - f64::from(start.0);
    let dy = f64::from(end.1) - f64::from(start.1);
    let steps = (dx.abs().max(dy.abs()) / FINISHED_PATH_DAMAGE_MAX_SPAN).ceil() as usize;
    let steps = steps.max(1);

    for step in 0..steps {
        let t0 = step as f64 / steps as f64;
        let t1 = (step + 1) as f64 / steps as f64;
        let p0 = (
            (f64::from(start.0) + dx * t0).round() as i32,
            (f64::from(start.1) + dy * t0).round() as i32,
        );
        let p1 = (
            (f64::from(start.0) + dx * t1).round() as i32,
            (f64::from(start.1) + dy * t1).round() as i32,
        );
        if let Some(region) = bounding_box_for_points(&[p0, p1], stroke_width) {
            regions.push(region);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::raw_preview_damage_regions;
    use crate::draw::Shape;
    use crate::input::Tool;
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn raw_preview_damage_covers_the_widest_pressure_sample_not_the_tool_width() {
        // The tool is set to 2px, but the tablet drew one sample 40px wide.
        // Smoothing moves the committed path off that sample, so the pixels
        // the wide press painted are only repainted if this width knows about
        // it.
        let points = vec![(100, 100), (300, 100)];
        let with_pressure = raw_preview_damage_regions(&points, 2.0, &[2.0, 40.0])
            .expect("a two-point path has damage");
        let without = raw_preview_damage_regions(&points, 2.0, &[2.0, 2.0])
            .expect("a two-point path has damage");

        // 15px off the path: inside a 40px stroke, well outside a 2px one.
        let (x, y) = (200, 115);
        assert!(
            with_pressure.iter().any(|rect| rect.contains(x, y)),
            "the wide sample's own pixels must be repainted, got {with_pressure:?}"
        );
        assert!(
            !without.iter().any(|rect| rect.contains(x, y)),
            "a stroke that stayed thin must not pay for width it never drew"
        );
    }

    /// A straight run with one sample knocked sideways, as a shaky hand makes.
    fn shaky_path() -> Vec<(i32, i32)> {
        vec![(0, 0), (10, 0), (20, 12), (30, 0), (40, 0)]
    }

    /// Draw `path` with `tool` at the state's current smoothing level and return
    /// the points the committed shape ended up with.
    fn drawn_points(level: u8, tool: Tool) -> Vec<(i32, i32)> {
        let mut state = make_test_input_state();
        state.set_pen_smoothing(level);
        state.set_tool_override(Some(tool));

        let path = shaky_path();
        let first = path[0];
        let last = *path.last().unwrap();
        state.on_mouse_press(crate::input::MouseButton::Left, first.0, first.1);
        for &(x, y) in &path[1..] {
            state.on_mouse_motion(x, y);
        }
        state.on_mouse_release(crate::input::MouseButton::Left, last.0, last.1);

        let shape = state
            .boards
            .active_frame()
            .shapes
            .last()
            .expect("the release committed a shape")
            .shape
            .clone();
        match shape {
            Shape::Freehand { points, .. } | Shape::MarkerStroke { points, .. } => points,
            other => panic!("expected a path stroke, got {other:?}"),
        }
    }

    #[test]
    fn a_committed_pen_stroke_is_smoothed_at_the_configured_level() {
        let raw = drawn_points(0, Tool::Pen);
        let smoothed = drawn_points(4, Tool::Pen);

        let raw_spike = raw.iter().map(|&(_, y)| y).max().unwrap();
        let smoothed_spike = smoothed.iter().map(|&(_, y)| y).max().unwrap();
        assert!(
            smoothed_spike < raw_spike,
            "level 4 must pull the spike down from {raw_spike}, got {smoothed_spike}"
        );
    }

    #[test]
    fn level_zero_commits_the_exact_path_the_pointer_drew() {
        assert_eq!(drawn_points(0, Tool::Pen), shaky_path());
    }

    #[test]
    fn a_smoothed_stroke_still_starts_and_ends_where_the_pointer_did() {
        let path = shaky_path();
        let smoothed = drawn_points(6, Tool::Pen);

        assert_eq!(smoothed.first(), path.first());
        assert_eq!(smoothed.last(), path.last());
    }

    /// Draw the shaky path at `level` and return the damage the release left.
    fn damage_after_drawing(level: u8) -> Vec<crate::util::Rect> {
        let mut state = make_test_input_state();
        state.set_pen_smoothing(level);
        state.set_tool_override(Some(Tool::Pen));
        let path = shaky_path();
        let first = path[0];
        let last = *path.last().unwrap();
        state.on_mouse_press(crate::input::MouseButton::Left, first.0, first.1);
        for &(x, y) in &path[1..] {
            state.on_mouse_motion(x, y);
        }
        let _ = state.take_dirty_regions();
        state.on_mouse_release(crate::input::MouseButton::Left, last.0, last.1);
        state.take_dirty_regions()
    }

    fn covers(regions: &[crate::util::Rect], x: i32, y: i32) -> bool {
        regions.iter().any(|rect| rect.contains(x, y))
    }

    #[test]
    fn a_smoothed_release_repaints_where_the_preview_drew_the_raw_path() {
        // The preview drew through the spike at (20, 12); the committed stroke
        // does not go there. Nothing repaints those pixels unless the release
        // says so, and they stay on screen as a ghost.
        let regions = damage_after_drawing(6);

        assert!(
            covers(&regions, 20, 12),
            "the raw spike the preview drew must be repainted, got {regions:?}"
        );
    }

    #[test]
    fn a_smoothed_release_keeps_the_split_damage_rather_than_the_whole_path_box() {
        // The raw path is repainted by its own split regions. Re-marking its
        // bounding box instead would undo the split-damage optimization, which
        // on a long diagonal stroke is most of the screen.
        let regions = damage_after_drawing(6);
        let path = shaky_path();
        let full = crate::draw::shape::bounding_box_for_points(&path, 64.0).unwrap();

        assert!(
            !regions.iter().any(|rect| rect.width >= full.width
                && rect.height >= full.height
                && rect.x <= full.x
                && rect.y <= full.y),
            "got a region covering the whole path box: {regions:?}"
        );
    }

    #[test]
    fn an_unsmoothed_release_still_damages_the_stroke_it_committed() {
        let regions = damage_after_drawing(0);

        assert!(covers(&regions, 20, 12), "got {regions:?}");
    }

    #[test]
    fn the_marker_is_smoothed_on_the_same_setting_as_the_pen() {
        let raw = drawn_points(0, Tool::Marker);
        let smoothed = drawn_points(4, Tool::Marker);

        assert_ne!(
            raw, smoothed,
            "a highlighter is drawn by hand too and shakes the same way"
        );
        assert_eq!(smoothed.first(), raw.first());
    }
}
