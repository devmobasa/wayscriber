//! Keeps a live text or sticky-note draft inside the visible output.

use super::base::{DrawingState, InputState, TextInputMode};
use crate::draw::TextMeasurer;
use crate::draw::shape::{bounding_box_for_sticky_note_preview_with, bounding_box_for_text_with};
use crate::util::Rect;

/// Gap kept between a draft and the output edge, so the caret drawn just past
/// the last glyph is not clipped either.
const TEXT_DRAFT_EDGE_MARGIN: i32 = 4;

impl InputState {
    /// The glyph the live preview shows at the caret: a bar when editing an
    /// existing block, an underscore for a new one.
    pub(in crate::input::state::core) fn text_preview_cursor_glyph(&self) -> &'static str {
        if self.text_editing.edit_target().is_some() {
            "|"
        } else {
            "_"
        }
    }

    /// Canvas bounds of a draft body laid out at `(x, y)` with the current
    /// style: the glyphs and optional background, or the note card and shadow.
    pub(in crate::input::state::core) fn text_draft_body_bounds_with(
        &self,
        measurer: &TextMeasurer,
        x: i32,
        y: i32,
        text: &str,
    ) -> Option<Rect> {
        match self.text_editing.mode() {
            TextInputMode::Plain => bounding_box_for_text_with(
                measurer,
                x,
                y,
                text,
                self.style.current_font_size,
                &self.style.font_descriptor,
                self.style.text_background_enabled,
                self.style.text_wrap_width,
            ),
            TextInputMode::StickyNote => bounding_box_for_sticky_note_preview_with(
                measurer,
                x,
                y,
                text,
                self.style.current_font_size,
                &self.style.font_descriptor,
                self.style.text_wrap_width,
            ),
        }
    }

    /// Moves the draft anchor so the block it renders stays inside the
    /// visible output, in canvas coordinates so pan and zoom are respected.
    ///
    /// Runs on every draft change, so a block placed near an edge or growing
    /// past one while typing shifts left or up and keeps its caret on screen,
    /// and the committed shape lands where the preview was.
    pub(in crate::input::state) fn keep_text_draft_inside_output_with(
        &mut self,
        measurer: &TextMeasurer,
    ) {
        let DrawingState::TextInput { x, y, .. } = self.state else {
            return;
        };
        let (screen_width, screen_height) = self.view.screen_size();
        if screen_width == 0 || screen_height == 0 {
            // The output size is not known yet, so there is no edge to keep to.
            return;
        }
        let Some(preview) = self.text_input_preview(self.text_preview_cursor_glyph()) else {
            return;
        };
        let Some(bounds) = self.text_draft_body_bounds_with(measurer, x, y, &preview.text) else {
            return;
        };

        let (dx, dy) = shift_inside(bounds, self.text_draft_area_with(measurer));

        if let DrawingState::TextInput { x, y, .. } = &mut self.state {
            *x = x.saturating_add(dx);
            *y = y.saturating_add(dy);
        }
    }

    /// Where the draft may sit: the visible output, widened while editing an
    /// existing block to wherever that block already reached. Opening a note
    /// that straddles an edge (after a pan, say) does not move it; only
    /// growing it further past the edge does.
    fn text_draft_area_with(&self, measurer: &TextMeasurer) -> Rect {
        let visible = self.visible_canvas_rect();
        let visible = visible.inflated(-TEXT_DRAFT_EDGE_MARGIN).unwrap_or(visible);
        self.text_editing
            .edit_target()
            .and_then(|(_, snapshot)| snapshot.shape.bounding_box_with(measurer))
            .and_then(|original| visible.union(original))
            .unwrap_or(visible)
    }
}

/// Offset that moves `bounds` inside `area`. A block larger than the area
/// keeps its leading edge, where its text starts, inside.
fn shift_inside(bounds: Rect, area: Rect) -> (i32, i32) {
    (
        axis_shift(bounds.x, bounds.width, area.x, area.width),
        axis_shift(bounds.y, bounds.height, area.y, area.height),
    )
}

fn axis_shift(start: i32, length: i32, area_start: i32, area_length: i32) -> i32 {
    let start = i64::from(start);
    let end = start + i64::from(length);
    let area_start = i64::from(area_start);
    let area_end = area_start + i64::from(area_length);

    let shift = if start < area_start || length >= area_length {
        area_start - start
    } else if end > area_end {
        area_end - end
    } else {
        0
    };
    i32::try_from(shift).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect::new(x, y, width, height).expect("test rect")
    }

    #[test]
    fn a_block_inside_the_area_stays_put() {
        assert_eq!(
            shift_inside(rect(10, 10, 50, 20), rect(0, 0, 100, 100)),
            (0, 0)
        );
    }

    #[test]
    fn a_block_past_the_right_and_bottom_edges_moves_back_inside() {
        assert_eq!(
            shift_inside(rect(80, 90, 50, 20), rect(0, 0, 100, 100)),
            (-30, -10)
        );
    }

    #[test]
    fn a_block_past_the_left_and_top_edges_moves_back_inside() {
        assert_eq!(
            shift_inside(rect(-15, -5, 50, 20), rect(0, 0, 100, 100)),
            (15, 5)
        );
    }

    #[test]
    fn a_block_wider_than_the_area_keeps_its_start_inside() {
        assert_eq!(
            shift_inside(rect(40, 10, 150, 20), rect(0, 0, 100, 100)),
            (-40, 0)
        );
    }

    #[test]
    fn offsets_follow_a_translated_area() {
        assert_eq!(
            shift_inside(rect(480, 250, 60, 20), rect(100, 200, 400, 300)),
            (-40, 0)
        );
    }
}
