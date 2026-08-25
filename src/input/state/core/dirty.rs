use super::base::{DrawingState, InputState, TextInputMode};
use crate::draw::Shape;
use crate::draw::shape::{
    CaretGeometry, LogicalBounds, bounding_box_for_points, bounding_box_for_sticky_note_preview,
    bounding_box_for_text,
};
use crate::input::tool::{
    PROVISIONAL_POLYGON_DAMAGE_PADDING, ToolMotionBehavior, ToolMotionSizeSource,
};
use crate::util::Rect;

const APPEND_ONLY_DAMAGE_MAX_SPAN: f64 = 128.0;

/// Padding around the edit-ghost's shape bounds for damage: the ghost draws a
/// dashed border ~4px outside the glyphs, so its damage region must reach past
/// that.
const GHOST_DAMAGE_PADDING: i32 = 6;

/// Anti-aliasing margin around the live text preview, matching the margin the
/// moving UI effects already carry (`UI_EFFECT_DAMAGE_MARGIN` in the render
/// layer). The preview is dragged under the pointer, so a shortfall of even one
/// pixel is repeated at every step and shows up as a trail of stray dots.
const TEXT_PREVIEW_DAMAGE_MARGIN: i32 = 2;

impl InputState {
    /// Clears any cached provisional shape bounds and marks their damage region.
    pub(crate) fn clear_provisional_dirty(&mut self) {
        if let Some(prev) = self.last_provisional_bounds.take() {
            self.dirty_tracker.mark_rect(prev);
        }
    }

    /// Takes cached provisional bounds without marking them dirty.
    pub(crate) fn take_provisional_dirty_bounds(&mut self) -> Option<Rect> {
        self.last_provisional_bounds.take()
    }

    /// Updates tracked provisional shape bounds for dirty-region purposes.
    pub(crate) fn update_provisional_dirty(&mut self, current_x: i32, current_y: i32) {
        if let Some((append_bounds, append_regions)) = self.compute_append_only_provisional_damage()
        {
            for region in append_regions {
                self.dirty_tracker.mark_rect(region);
            }
            self.last_provisional_bounds =
                union_optional_rect(self.last_provisional_bounds, append_bounds);
            return;
        }

        let new_bounds = self.compute_provisional_bounds(current_x, current_y);
        let previous = self.last_provisional_bounds;

        if new_bounds != previous
            && let Some(prev) = previous
        {
            self.dirty_tracker.mark_rect(prev);
        }

        if let Some(bounds) = new_bounds {
            self.dirty_tracker.mark_rect(bounds);
            self.last_provisional_bounds = Some(bounds);
        } else {
            self.last_provisional_bounds = None;
        }
    }

    /// Marks the full current provisional shape dirty.
    ///
    /// This is needed when existing provisional geometry changes in place, for
    /// example when the first tablet pressure sample backfills previous widths.
    pub(crate) fn mark_current_provisional_dirty_full(&mut self) {
        let (current_x, current_y) = self.last_canvas_pointer_position;
        if let Some(bounds) = self.compute_provisional_bounds(current_x, current_y) {
            self.dirty_tracker.mark_rect(bounds);
            self.last_provisional_bounds =
                union_optional_rect(self.last_provisional_bounds, bounds);
        }
    }

    fn compute_provisional_bounds(&self, current_x: i32, current_y: i32) -> Option<Rect> {
        match &self.state {
            DrawingState::Drawing { .. } => {
                self.provisional_tool_stroke(current_x, current_y).bounds()
            }
            DrawingState::Selecting {
                start_x, start_y, ..
            } => Self::selection_rect_from_points(*start_x, *start_y, current_x, current_y)
                .and_then(|rect| rect.inflated(2)),
            DrawingState::BuildingPolygon {
                points,
                preview,
                thick,
                ..
            } => {
                let mut preview_points = points.clone();
                if let Some(point) = preview.or(Some((current_x, current_y))) {
                    preview_points.push(point);
                }
                bounding_box_for_points(&preview_points, *thick)
                    .and_then(|rect| rect.inflated(PROVISIONAL_POLYGON_DAMAGE_PADDING))
            }
            _ => None,
        }
    }

    fn compute_append_only_provisional_damage(&self) -> Option<(Rect, Vec<Rect>)> {
        let DrawingState::Drawing {
            tool,
            points,
            point_thicknesses,
            ..
        } = &self.state
        else {
            return None;
        };

        // A snapped marker stroke is not append-only: its far end moves with
        // the pointer and its near end moves too when the drag reverses, so
        // damaging only the newest segment would leave the rest on screen.
        if self.active_marker_snap_row.is_some() {
            return None;
        }

        let stroke_width = match tool.motion_behavior() {
            ToolMotionBehavior::NoPathAccumulation => return None,
            ToolMotionBehavior::AccumulatePath {
                size_source: ToolMotionSizeSource::ToolSize,
            } => {
                if *tool == crate::input::Tool::Marker {
                    let size = self.thickness_for_tool(*tool);
                    (size * 1.35).max(size + 1.0)
                } else if point_thicknesses.len() == points.len() && !point_thicknesses.is_empty() {
                    let start = point_thicknesses.len().saturating_sub(2);
                    point_thicknesses[start..]
                        .iter()
                        .fold(1.0f64, |max, &thickness| max.max(thickness as f64))
                } else {
                    self.thickness_for_tool(*tool)
                }
            }
            ToolMotionBehavior::AccumulatePath {
                size_source: ToolMotionSizeSource::EraserSize,
            } => self.eraser_size,
        };

        let start = points.len().saturating_sub(2);
        let tail_points = &points[start..];
        let bounds = bounding_box_for_points(tail_points, stroke_width)?;
        let regions = append_only_damage_regions(tail_points, stroke_width, bounds);
        Some((bounds, regions))
    }

    /// Updates dirty tracking for the live text preview/caret overlay.
    pub(crate) fn update_text_preview_dirty(&mut self) {
        self.text_input_cursor_rect_dirty = true;
        let new_bounds = self.compute_text_preview_bounds();
        let previous = self.last_text_preview_bounds;

        if new_bounds != previous
            && let Some(prev) = previous
        {
            self.dirty_tracker.mark_rect(prev);
        }

        if let Some(bounds) = new_bounds {
            self.dirty_tracker.mark_rect(bounds);
            self.last_text_preview_bounds = Some(bounds);
        } else {
            self.last_text_preview_bounds = None;
        }
    }

    /// Updates text preview damage for a change authored outside the input
    /// method. The backend uses this bit to publish text-input-v3's `Other`
    /// change cause with the coalesced surrounding-text/caret update.
    pub(crate) fn update_text_preview_dirty_from_editor(&mut self) {
        self.text_input_external_change_dirty = true;
        self.update_text_preview_dirty();
    }

    /// Clears the cached text preview bounds.
    pub(crate) fn clear_text_preview_dirty(&mut self) {
        self.text_input_cursor_rect_dirty = false;
        self.text_input_external_change_dirty = false;
        if let Some(prev) = self.last_text_preview_bounds.take() {
            self.dirty_tracker.mark_rect(prev);
        }
    }

    /// Drain one coalesced request to publish the current caret geometry to
    /// the compositor's text-input object.
    pub(crate) fn take_text_input_cursor_rect_dirty(&mut self) -> bool {
        std::mem::take(&mut self.text_input_cursor_rect_dirty)
    }

    /// Drains the coalesced external-editor origin for the next protocol
    /// update independently of geometry dirtiness.
    pub(crate) fn take_text_input_external_change_dirty(&mut self) -> bool {
        std::mem::take(&mut self.text_input_external_change_dirty)
    }

    fn compute_text_preview_bounds(&self) -> Option<Rect> {
        let DrawingState::TextInput { x, y, .. } = &self.state else {
            return None;
        };
        let cursor_glyph = if self.text_edit_target.is_some() {
            "|"
        } else {
            "_"
        };
        let preview = self.text_input_preview(cursor_glyph)?;
        let text_bounds = match self.text_input_mode {
            TextInputMode::Plain => bounding_box_for_text(
                *x,
                *y,
                &preview.text,
                self.current_font_size,
                &self.font_descriptor,
                self.text_background_enabled,
                self.text_wrap_width,
            ),
            TextInputMode::StickyNote => bounding_box_for_sticky_note_preview(
                *x,
                *y,
                &preview.text,
                self.current_font_size,
                &self.font_descriptor,
                self.text_wrap_width,
            ),
        };

        // The caret is a full-line-height vertical bar that can extend past the
        // glyph ink box (above ascenders, below the baseline) and, mid-line,
        // sits inside it — either way its exact rect must be part of the damage,
        // or dragging the block leaves a trail of un-erased caret pixels. The
        // Pango decorations (selection background, preedit underline) are painted
        // over logical cells that reach beyond the ink box, so damage the full
        // logical extent whenever one of them is showing.
        let mut live_bounds = text_bounds;
        let decorations_showing = [preview.highlight.as_ref(), preview.underline.as_ref()]
            .into_iter()
            .flatten()
            .any(|range| range.start < range.end);
        // Caret and decoration damage read the same layout, so resolve them
        // together: with a selection or composition showing that is one layout
        // instead of two. Skipped entirely when neither is present.
        if preview.caret.is_some() || decorations_showing {
            let font = self.font_descriptor.to_pango_string(self.current_font_size);
            if let Some(geometry) = crate::draw::shape::text_preview_geometry(
                &preview.text,
                &font,
                self.text_wrap_width,
                preview.caret,
            ) {
                let caret_bounds = geometry
                    .caret
                    .and_then(|geom| caret_damage_rect(geom, *x, *y, self.current_font_size));
                let decoration_bounds = decorations_showing
                    .then(|| pango_decoration_damage_rect(geometry.logical, *x, *y))
                    .flatten();
                for extra in [caret_bounds, decoration_bounds].into_iter().flatten() {
                    live_bounds = Some(match live_bounds {
                        Some(base) => union_rect(base, extra).unwrap_or(base),
                        None => extra,
                    });
                }
            }
        }

        // When the block has been moved, the ghost renders at the *original*
        // spot, away from the live text. Fold its bounds in so the ghost is
        // erased on move-back and on commit (otherwise it lingers there).
        let ghost_bounds = self.text_edit_ghost_damage_bounds();
        let bounds = match (live_bounds, ghost_bounds) {
            (Some(live), Some(ghost)) => union_rect(live, ghost).or(Some(live)),
            (Some(live), None) => Some(live),
            (None, ghost) => ghost,
        }?;
        // The preview is the one canvas element that moves under the pointer, so
        // any sub-pixel shortfall is re-committed at every drag step and reads as
        // a trail of stray dots. Carry the same anti-aliasing margin the moving
        // UI effects use (see `UI_EFFECT_DAMAGE_MARGIN`); over-damaging a few
        // pixels around one block is far cheaper than the artifacts.
        bounds.inflated(TEXT_PREVIEW_DAMAGE_MARGIN).or(Some(bounds))
    }

    /// Whether the edit ghost (the faded original of a text/note being edited)
    /// should be shown: only once the block has been moved from where the
    /// original sits. In place it overlaps the live text and reads as undeleted
    /// text, so it stays hidden. The renderer defers to this so the damage
    /// bounds and the drawn ghost always agree.
    pub(crate) fn text_edit_ghost_visible(&self) -> bool {
        let Some((_, snapshot)) = &self.text_edit_target else {
            return false;
        };
        let DrawingState::TextInput { x, y, .. } = &self.state else {
            return false;
        };
        text_edit_block_moved((*x, *y), &snapshot.shape)
    }

    /// Damage bounds for the edit ghost when it is visible: the original shape's
    /// bounding box, padded to cover the dashed border. `None` when no ghost
    /// shows.
    fn text_edit_ghost_damage_bounds(&self) -> Option<Rect> {
        if !self.text_edit_ghost_visible() {
            return None;
        }
        let (_, snapshot) = self.text_edit_target.as_ref()?;
        snapshot
            .shape
            .bounding_box()?
            .inflated(GHOST_DAMAGE_PADDING)
    }

    /// Exact damage rectangle for the caret line at `caret` (a byte offset into
    /// `buffer`), measuring the layout itself. `compute_text_preview_bounds`
    /// shares a layout instead; this stands alone for callers that have only a
    /// buffer and an offset.
    #[cfg(test)]
    fn caret_damage_rect_for(&self, buffer: &str, x: i32, y: i32, caret: usize) -> Option<Rect> {
        let size = self.current_font_size;
        let font = self.font_descriptor.to_pango_string(size);
        let geom =
            crate::draw::shape::caret_geometry_text(buffer, &font, self.text_wrap_width, caret)?;
        caret_damage_rect(geom, x, y, size)
    }

    /// The caret's rectangle in canvas coordinates — a thin strip at the caret
    /// position — for reporting to the IME (`set_cursor_rectangle`) so candidate
    /// popups sit at the composition point. Uses the exact Pango caret geometry,
    /// so it is correct mid-buffer and in wrapped/multiline text, unlike the old
    /// append-only "right edge of the preview" assumption. `None` outside text
    /// input or when no measurement context exists.
    pub(crate) fn caret_cursor_rect_canvas(&self) -> Option<Rect> {
        let DrawingState::TextInput { x, y, .. } = &self.state else {
            return None;
        };
        let cursor_glyph = if self.text_edit_target.is_some() {
            "|"
        } else {
            "_"
        };
        let preview = self.text_input_preview(cursor_glyph)?;
        let cursor = preview.ime_cursor?;
        let font = self.font_descriptor.to_pango_string(self.current_font_size);
        let geom = crate::draw::shape::caret_geometry_text(
            &preview.text,
            &font,
            self.text_wrap_width,
            cursor,
        )?;
        let caret_x = x + geom.x.round() as i32;
        let top = y + geom.y_from_baseline.round() as i32;
        let height = geom.height.ceil().max(1.0) as i32;
        Rect::from_min_max(caret_x, top, caret_x + 2, top + height)
    }
}

/// Exact damage rectangle for a caret line, in canvas coordinates, with a small
/// margin covering the stroke width. Mirrors `render_caret_line`'s geometry so
/// repaints erase it fully.
///
/// The caret's centre is kept in floating point until the very last step:
/// rounding it first and then subtracting a whole-pixel half-width can move the
/// rectangle off the leftmost column the stroke actually touches.
fn caret_damage_rect(geom: CaretGeometry, x: i32, y: i32, size: f64) -> Option<Rect> {
    let half_w = crate::draw::caret_outline_width(size) / 2.0;
    let caret_x = f64::from(x) + geom.x;
    let left = (caret_x - half_w).floor() as i32 - 1;
    let right = (caret_x + half_w).ceil() as i32 + 1;
    let top = y + geom.y_from_baseline.floor() as i32 - 1;
    let bottom = y + (geom.y_from_baseline + geom.height).ceil() as i32 + 1;
    Rect::from_min_max(left, top, right, bottom)
}

/// Damage bounds for the Pango-painted decorations: the selection background and
/// the IME preedit underline. Both are laid out against logical cells
/// (whitespace included) that reach past the glyph ink box — the underline in
/// particular sits *below* the baseline, outside the ink box entirely for text
/// without descenders — so the text's full logical box is a safe superset.
fn pango_decoration_damage_rect(logical: LogicalBounds, x: i32, y: i32) -> Option<Rect> {
    let left = x + logical.x.floor() as i32 - 1;
    let top = y + logical.y_from_baseline.floor() as i32 - 1;
    let right = x + (logical.x + logical.width).ceil() as i32 + 1;
    let bottom = y + (logical.y_from_baseline + logical.height).ceil() as i32 + 1;
    Rect::from_min_max(left, top, right, bottom)
}

/// Whether the active text block has moved from its original position — the
/// condition under which the edit ghost is shown. A non-text original is never
/// a real edit target, so it reports "not moved".
fn text_edit_block_moved(current: (i32, i32), original: &Shape) -> bool {
    match original {
        Shape::Text { x, y, .. } | Shape::StickyNote { x, y, .. } => {
            current.0 != *x || current.1 != *y
        }
        _ => false,
    }
}

fn union_optional_rect(current: Option<Rect>, next: Rect) -> Option<Rect> {
    match current {
        Some(current) => union_rect(current, next),
        None => Some(next),
    }
}

fn union_rect(a: Rect, b: Rect) -> Option<Rect> {
    let min_x = a.x.min(b.x);
    let min_y = a.y.min(b.y);
    let max_x = a.x.saturating_add(a.width).max(b.x.saturating_add(b.width));
    let max_y =
        a.y.saturating_add(a.height)
            .max(b.y.saturating_add(b.height));
    Rect::from_min_max(min_x, min_y, max_x, max_y)
}

fn append_only_damage_regions(
    points: &[(i32, i32)],
    stroke_width: f64,
    fallback: Rect,
) -> Vec<Rect> {
    let Some((&start, &end)) = points.first().zip(points.last()) else {
        return vec![fallback];
    };
    if start == end {
        return vec![fallback];
    }

    let dx = f64::from(end.0 - start.0);
    let dy = f64::from(end.1 - start.1);
    let steps = (dx.abs().max(dy.abs()) / APPEND_ONLY_DAMAGE_MAX_SPAN).ceil() as usize;
    let steps = steps.max(1);
    if steps == 1 {
        return vec![fallback];
    }

    let mut regions = Vec::with_capacity(steps);
    for step in 0..steps {
        let t0 = step as f64 / steps as f64;
        let t1 = (step + 1) as f64 / steps as f64;
        let p0 = (
            (start.0 as f64 + dx * t0).round() as i32,
            (start.1 as f64 + dy * t0).round() as i32,
        );
        let p1 = (
            (start.0 as f64 + dx * t1).round() as i32,
            (start.1 as f64 + dy * t1).round() as i32,
        );
        if let Some(region) = bounding_box_for_points(&[p0, p1], stroke_width) {
            regions.push(region);
        }
    }

    if regions.is_empty() {
        vec![fallback]
    } else {
        regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{Color, FontDescriptor};
    use crate::input::state::test_support::make_test_input_state;

    fn text_shape(x: i32, y: i32) -> Shape {
        Shape::Text {
            x,
            y,
            text: "some text".to_string(),
            color: Color::new(1.0, 0.5, 0.0, 1.0),
            size: 20.0,
            font_descriptor: FontDescriptor::default(),
            background_enabled: false,
            wrap_width: None,
        }
    }

    #[test]
    fn ghost_stays_hidden_while_editing_in_place() {
        let original = text_shape(100, 100);
        assert!(
            !text_edit_block_moved((100, 100), &original),
            "an unmoved block shows no ghost, so deletes look clean"
        );
    }

    #[test]
    fn ghost_appears_once_the_block_moves() {
        let original = text_shape(100, 100);
        assert!(text_edit_block_moved((120, 100), &original), "moved in x");
        assert!(text_edit_block_moved((100, 115), &original), "moved in y");
    }

    #[test]
    fn sticky_note_origin_is_tracked_too() {
        let original = Shape::StickyNote {
            x: 10,
            y: 20,
            text: "note".to_string(),
            background: Color::new(0.9, 0.9, 0.2, 1.0),
            size: 18.0,
            font_descriptor: FontDescriptor::default(),
            wrap_width: None,
        };
        assert!(!text_edit_block_moved((10, 20), &original));
        assert!(text_edit_block_moved((11, 20), &original));
    }

    #[test]
    fn leading_whitespace_selection_damages_to_the_logical_left_edge() {
        // Selecting leading spaces highlights logical cells that start at the
        // text origin (x), left of the ink box; the damage must reach them.
        let mut state = make_test_input_state();
        state.state = DrawingState::text_input(100, 100, "   hi".to_string());
        if let DrawingState::TextInput {
            caret,
            selection_anchor,
            buffer,
            ..
        } = &mut state.state
        {
            *selection_anchor = Some(0);
            *caret = buffer.len();
        }

        let bounds = state
            .compute_text_preview_bounds()
            .expect("active text input has preview bounds");
        assert!(
            bounds.x <= 100,
            "selection damage reaches the origin (logical left), got x={}",
            bounds.x
        );
    }

    #[test]
    fn a_preedit_underline_is_damaged_below_the_baseline() {
        // Pango draws the composition underline *below* the baseline. With text
        // that has no descenders the ink box stops at the baseline, so damage
        // built from ink alone leaves the underline behind when the block moves.
        let mut state = make_test_input_state();
        state.state = DrawingState::text_input(100, 100, String::new());
        state.ime_queue_preedit(Some("kako".to_string()), 4, 4);
        assert!(state.ime_apply_done());

        let bounds = state
            .compute_text_preview_bounds()
            .expect("a composing block has preview bounds");
        let ink = bounding_box_for_text(
            100,
            100,
            "kako",
            state.current_font_size,
            &state.font_descriptor,
            state.text_background_enabled,
            state.text_wrap_width,
        )
        .expect("non-empty preedit measures");
        // Past the ink box *and* past the plain anti-aliasing margin, i.e. only
        // satisfied by folding in the logical box the underline is drawn against.
        assert!(
            bounds.y + bounds.height > ink.y + ink.height + TEXT_PREVIEW_DAMAGE_MARGIN,
            "damage must reach past the ink box for the underline: \
             damage bottom {} vs ink bottom {}",
            bounds.y + bounds.height,
            ink.y + ink.height
        );
    }

    #[test]
    fn preview_damage_carries_the_antialiasing_margin() {
        // The preview is dragged under the pointer, so it damages a margin
        // around its exact geometry; a shortfall would trail stray pixels.
        let mut state = make_test_input_state();
        state.state = DrawingState::text_input(100, 100, "kako".to_string());
        let bounds = state
            .compute_text_preview_bounds()
            .expect("active text input has preview bounds");
        let ink = bounding_box_for_text(
            100,
            100,
            "kako",
            state.current_font_size,
            &state.font_descriptor,
            state.text_background_enabled,
            state.text_wrap_width,
        )
        .expect("non-empty text measures");
        assert!(
            bounds.x <= ink.x - TEXT_PREVIEW_DAMAGE_MARGIN
                && bounds.y <= ink.y - TEXT_PREVIEW_DAMAGE_MARGIN
                && bounds.x + bounds.width >= ink.x + ink.width + TEXT_PREVIEW_DAMAGE_MARGIN
                && bounds.y + bounds.height >= ink.y + ink.height + TEXT_PREVIEW_DAMAGE_MARGIN,
            "preview damage {bounds:?} must clear the glyph box {ink:?} on every side"
        );
    }

    #[test]
    fn caret_damage_covers_the_stroke_around_a_fractional_caret_position() {
        // The caret is stroked centred on its position: rounding the centre to a
        // pixel before subtracting a whole-pixel half-width can drop the leftmost
        // column the stroke touches, which is a 1px sliver per drag step.
        let mut state = make_test_input_state();
        state.state = DrawingState::text_input(100, 100, "hi".to_string());
        let font = state
            .font_descriptor
            .to_pango_string(state.current_font_size);
        let geom = crate::draw::shape::caret_geometry_text("hi", &font, None, 1).unwrap();
        let rect = state
            .caret_damage_rect_for("hi", 100, 100, 1)
            .expect("caret has a damage rect");
        let half = crate::draw::caret_outline_width(state.current_font_size) / 2.0;
        let painted_left = 100.0 + geom.x - half;
        let painted_right = 100.0 + geom.x + half;
        assert!(
            f64::from(rect.x) <= painted_left,
            "damage left {} must cover the stroke at {painted_left}",
            rect.x
        );
        assert!(
            f64::from(rect.x + rect.width) >= painted_right,
            "damage right {} must cover the stroke at {painted_right}",
            rect.x + rect.width
        );
    }

    #[test]
    fn caret_cursor_rect_tracks_the_caret_not_the_buffer_end() {
        let mut state = make_test_input_state();
        state.state = DrawingState::text_input(100, 100, "hello".to_string());
        let end = state
            .caret_cursor_rect_canvas()
            .expect("caret at end has a rect");

        if let DrawingState::TextInput { caret, .. } = &mut state.state {
            *caret = 0;
        }
        let start = state
            .caret_cursor_rect_canvas()
            .expect("caret at start has a rect");

        assert!(
            end.x > start.x,
            "the reported caret follows the caret offset, not the buffer end"
        );
        assert!(
            (start.x - 100).abs() <= 2,
            "the start caret sits at the origin, got x={}",
            start.x
        );
        assert!(start.width >= 1 && start.height >= 1);
    }

    #[test]
    fn ime_cursor_rect_uses_the_selection_replacement_point() {
        let mut state = make_test_input_state();
        state.state = DrawingState::text_input(100, 100, "hello world".to_string());
        if let DrawingState::TextInput {
            caret,
            selection_anchor,
            ..
        } = &mut state.state
        {
            *selection_anchor = Some(0);
            *caret = 5;
        }
        state.ime_queue_preedit(Some("X".to_string()), 1, 1);
        state.ime_apply_done();

        let rect = state
            .caret_cursor_rect_canvas()
            .expect("active preedit cursor has a rectangle");

        assert!(
            rect.x < 150,
            "the candidate cursor follows the replacement at the selection start, got x={}",
            rect.x
        );
    }

    #[test]
    fn text_preview_damage_covers_the_full_caret_line() {
        let mut state = make_test_input_state();
        state.state = DrawingState::text_input(100, 100, "hi".to_string());

        let bounds = state
            .compute_text_preview_bounds()
            .expect("an active text input has preview bounds");
        let caret = state
            .caret_damage_rect_for("hi", 100, 100, 2)
            .expect("the caret has geometry");

        // The caret's exact rect (mirroring the renderer) must fall inside the
        // damage bounds, so moving the block repaints the whole caret line and
        // leaves no trail of un-erased pixels.
        assert!(bounds.x <= caret.x, "caret left edge is damaged");
        assert!(bounds.y <= caret.y, "caret top is damaged");
        assert!(
            bounds.x + bounds.width >= caret.x + caret.width,
            "caret right edge is damaged"
        );
        assert!(
            bounds.y + bounds.height >= caret.y + caret.height,
            "caret bottom is damaged"
        );
    }

    #[test]
    fn text_preview_updates_coalesce_one_backend_cursor_request() {
        let mut state = make_test_input_state();
        state.state = DrawingState::text_input(100, 100, "hi".to_string());

        state.update_text_preview_dirty();
        state.update_text_preview_dirty();

        assert!(state.take_text_input_cursor_rect_dirty());
        assert!(
            !state.take_text_input_cursor_rect_dirty(),
            "multiple editor changes coalesce before the backend drains them"
        );
    }

    #[test]
    fn external_editor_changes_are_tracked_separately_from_ime_damage() {
        let mut state = make_test_input_state();
        state.state = DrawingState::text_input(100, 100, "hi".to_string());

        state.update_text_preview_dirty();
        assert!(state.take_text_input_cursor_rect_dirty());
        assert!(
            !state.take_text_input_external_change_dirty(),
            "IME-driven preview damage keeps the protocol's InputMethod cause"
        );

        state.update_text_preview_dirty_from_editor();
        assert!(state.take_text_input_cursor_rect_dirty());
        assert!(state.take_text_input_external_change_dirty());
        assert!(
            !state.take_text_input_external_change_dirty(),
            "the external cause coalesces and drains with one protocol update"
        );
    }

    #[test]
    fn empty_sticky_note_damage_covers_the_background_not_only_the_caret() {
        let mut state = make_test_input_state();
        state.text_input_mode = TextInputMode::StickyNote;
        state.state = DrawingState::text_input(100, 100, String::new());

        let bounds = state
            .compute_text_preview_bounds()
            .expect("empty sticky-note preview has damage bounds");

        assert!(
            bounds.width > 10 && bounds.height > 10,
            "sticky-note damage must cover its padded background: {bounds:?}"
        );
    }
}
