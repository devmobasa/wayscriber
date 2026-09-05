use super::*;

impl TextMeasurer {
    /// Hit-test a point against a rendered text run, returning the caret byte
    /// offset nearest the point. Coordinates are relative to the text's stored
    /// origin `(x, y)`: `local_x = point_x - x`, and `local_y_from_baseline =
    /// point_y - y` (the stored `y` is the first-line baseline). Layout-aware, so
    /// it is correct for wrapped and multiline text. The caret snaps to the
    /// trailing edge of a glyph when the point is on its right half. Returns `None`
    /// only when no measurement context is available.
    pub(crate) fn hit_test_text(
        &self,
        text: &str,
        font_desc_str: &str,
        wrap_width: Option<i32>,
        local_x: f64,
        local_y_from_baseline: f64,
    ) -> Option<usize> {
        if text.is_empty() {
            return Some(0);
        }
        self.with_measurement_context(|ctx| {
            let layout = configured_layout(ctx, text, font_desc_str, wrap_width);

            let scale = pango::SCALE as f64;
            // Convert the baseline-relative y into the layout's top-left frame.
            let local_y = local_y_from_baseline + layout.baseline() as f64 / scale;
            let x_pu = (local_x * scale)
                .round()
                .clamp(i32::MIN as f64, i32::MAX as f64) as i32;
            let y_pu = (local_y * scale)
                .round()
                .clamp(i32::MIN as f64, i32::MAX as f64) as i32;
            let (_inside, index, trailing) = layout.xy_to_index(x_pu, y_pu);

            // Advance past `trailing` characters so a click on a glyph's right half
            // lands the caret after it, keeping the result on a char boundary.
            hit_position_to_byte(text, index, trailing)
        })
    }
    /// Return the adjacent Pango cursor position in physical left/right order.
    /// Logical byte order is insufficient for RTL and mixed-direction text.
    pub(crate) fn caret_on_adjacent_visual_position(
        &self,
        text: &str,
        font_desc_str: &str,
        wrap_width: Option<i32>,
        byte_index: usize,
        direction: VisualCaretDirection,
    ) -> Option<usize> {
        self.with_measurement_context(|ctx| {
            let layout = configured_layout(ctx, text, font_desc_str, wrap_width);

            let index = snap_char_boundary(text, byte_index);
            let old_index = i32::try_from(index).unwrap_or(i32::MAX);
            let direction = match direction {
                VisualCaretDirection::Left => -1,
                VisualCaretDirection::Right => 1,
            };
            let (new_index, trailing) = layout.move_cursor_visually(true, old_index, 0, direction);

            // Pango uses sentinels when movement would leave either visual edge of
            // the layout. Keep the current logical position at those boundaries.
            if new_index < 0 || new_index == i32::MAX {
                return index;
            }
            hit_position_to_byte(text, new_index, trailing)
        })
    }
    /// Resolve which endpoint of a same-line selection is physically left/right.
    /// For selections crossing visual lines, preserve the editor's established
    /// document-order collapse behavior.
    pub(crate) fn caret_at_visual_selection_edge(
        &self,
        text: &str,
        font_desc_str: &str,
        wrap_width: Option<i32>,
        start: usize,
        end: usize,
        direction: VisualCaretDirection,
    ) -> Option<usize> {
        self.with_measurement_context(|ctx| {
            let layout = configured_layout(ctx, text, font_desc_str, wrap_width);

            let start = start.min(text.len());
            let end = end.min(text.len());
            let (start_line, start_x) =
                layout.index_to_line_x(i32::try_from(start).unwrap_or(i32::MAX), false);
            let (end_line, end_x) =
                layout.index_to_line_x(i32::try_from(end).unwrap_or(i32::MAX), false);
            if start_line != end_line {
                return match direction {
                    VisualCaretDirection::Left => start,
                    VisualCaretDirection::Right => end,
                };
            }
            match direction {
                VisualCaretDirection::Left if start_x <= end_x => start,
                VisualCaretDirection::Left => end,
                VisualCaretDirection::Right if start_x >= end_x => start,
                VisualCaretDirection::Right => end,
            }
        })
    }
    /// Return the logical byte offset at the start/end of the current Pango visual
    /// line, including lines introduced by soft wrapping.
    pub(crate) fn caret_on_visual_line_edge(
        &self,
        text: &str,
        font_desc_str: &str,
        wrap_width: Option<i32>,
        byte_index: usize,
        edge: VisualLineEdge,
    ) -> Option<usize> {
        self.with_measurement_context(|ctx| {
            let layout = configured_layout(ctx, text, font_desc_str, wrap_width);

            let index = snap_char_boundary(text, byte_index);
            let (line_index, _) =
                layout.index_to_line_x(i32::try_from(index).unwrap_or(i32::MAX), false);
            let Some(line) = layout.line_readonly(line_index) else {
                return index;
            };
            let start = usize::try_from(line.start_index()).unwrap_or(0);
            let mut end = start
                .saturating_add(usize::try_from(line.length()).unwrap_or(0))
                .min(text.len());
            if end > start && text.as_bytes().get(end - 1) == Some(&b'\n') {
                end -= 1;
            }
            match edge {
                VisualLineEdge::Start => start,
                VisualLineEdge::End => end,
            }
        })
    }
    /// Return the caret offset on the adjacent Pango visual line while preserving
    /// the current horizontal layout position. This follows soft wrapping as well
    /// as explicit newlines. At the first/last visual line it resolves to the
    /// document start/end, matching the editor's existing boundary behavior.
    pub(crate) fn caret_on_adjacent_visual_line(
        &self,
        text: &str,
        font_desc_str: &str,
        wrap_width: Option<i32>,
        byte_index: usize,
        direction: VisualLineDirection,
    ) -> Option<usize> {
        self.with_measurement_context(|ctx| {
            let layout = configured_layout(ctx, text, font_desc_str, wrap_width);

            let index = snap_char_boundary(text, byte_index);
            let index_i32 = i32::try_from(index).unwrap_or(i32::MAX);
            let (line_index, x) = layout.index_to_line_x(index_i32, false);
            let target_line = match direction {
                VisualLineDirection::Up if line_index == 0 => return 0,
                VisualLineDirection::Up => line_index - 1,
                VisualLineDirection::Down if line_index + 1 >= layout.line_count() => {
                    return text.len();
                }
                VisualLineDirection::Down => line_index + 1,
            };
            let Some(line) = layout.line_readonly(target_line) else {
                return index;
            };
            let hit = line.x_to_index(x);
            hit_position_to_byte(text, hit.index(), hit.trailing())
        })
    }
    /// Compute the strong caret position on wrapped and multiline text, including
    /// empty text and the string end. Off-boundary byte indices snap down to a
    /// character boundary; missing measurement resources return `None`.
    pub(crate) fn caret_geometry_text(
        &self,
        text: &str,
        font_desc_str: &str,
        wrap_width: Option<i32>,
        byte_index: usize,
    ) -> Option<CaretGeometry> {
        self.with_measurement_context(|ctx| {
            let layout = configured_layout(ctx, text, font_desc_str, wrap_width);
            caret_geometry_in(&layout, text, byte_index)
        })
    }
    /// Resolve caret geometry and logical bounds together. The damage tracker needs
    /// both for the same text whenever a selection or composition is showing, so
    /// sharing one layout saves building a second one for those states. Text bounds
    /// still go through `TextMeasurer::measure`, which lays out again on a cache miss;
    /// this only removes the duplicate pass, it does not make damage layout-free.
    pub(crate) fn text_preview_geometry(
        &self,
        text: &str,
        font_desc_str: &str,
        wrap_width: Option<i32>,
        byte_index: Option<usize>,
    ) -> Option<TextPreviewGeometry> {
        self.with_measurement_context(|ctx| {
            let layout = configured_layout(ctx, text, font_desc_str, wrap_width);
            TextPreviewGeometry {
                caret: byte_index.map(|byte_index| caret_geometry_in(&layout, text, byte_index)),
                logical: logical_bounds_in(&layout),
            }
        })
    }
}
