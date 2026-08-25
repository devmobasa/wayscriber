//! Snapping the marker to a detected row of screen text.
//!
//! The map holds one box per detected text line in **logical screen**
//! coordinates, which is the space the detection was produced in and the only
//! one that stays correct while the canvas transform moves underneath it. The
//! caller converts the pointer into screen space to ask a question and converts
//! the answer back; see `InputState::marker_snap_row_at`.
//!
//! Everything here is pure: no compositor, no captured pixels, no text.

/// One detected line of screen text, in logical screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSnapLine {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl TextSnapLine {
    pub fn height(self) -> f64 {
        (self.bottom - self.top).max(0.0)
    }

    pub fn center_y(self) -> f64 {
        (self.top + self.bottom) / 2.0
    }

    fn width(self) -> f64 {
        (self.right - self.left).max(0.0)
    }

    fn is_usable(self) -> bool {
        self.left.is_finite()
            && self.top.is_finite()
            && self.right.is_finite()
            && self.bottom.is_finite()
            && self.right > self.left
            && self.bottom > self.top
    }

    /// Whether this box is shaped like a row of text rather than like a blob.
    fn is_row_shaped(self) -> bool {
        self.height() >= MIN_LINE_HEIGHT && self.height() <= self.width() * MAX_HEIGHT_TO_WIDTH
    }
}

/// The row a stroke snaps to, in logical screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnappedTextRow {
    /// Vertical center of the text: where the highlight's spine sits.
    pub center_y: f64,
    /// Horizontal extent the stroke is clamped to, already padded.
    pub left: f64,
    pub right: f64,
    /// Detected line height, which sets the highlight's thickness.
    pub height: f64,
}

/// How far above and below a line's box the pointer may sit and still snap to
/// it, as a multiple of the line height. Text rows are usually separated by
/// roughly half a line of leading, so half a line reaches the gap without
/// letting one row claim its neighbor's ink.
const VERTICAL_REACH: f64 = 0.5;
/// How far past a line's horizontal ends the pointer may sit and still snap,
/// in logical pixels. Generous enough to start a highlight from the margin.
const HORIZONTAL_REACH: f64 = 48.0;
/// How far a committed highlight may overhang the ink, as a multiple of the
/// line height. A highlighter drawn by hand always overshoots slightly, and a
/// stroke that stops exactly on the last glyph reads as clipped.
const OVERHANG: f64 = 0.25;
/// Highlight thickness as a multiple of the detected line height. Slightly over
/// 1 so ascenders and descenders are covered rather than skimmed.
const THICKNESS_FACTOR: f64 = 1.15;
/// Lines shorter than this in logical pixels are noise, not text.
const MIN_LINE_HEIGHT: f64 = 4.0;
/// How much taller than it is wide a row may be and still count as text.
///
/// A row of text runs across the screen; even a one-word line is roughly as
/// wide as it is tall. A box taller than it is wide is a blob the engine
/// mistook for a character — a busy image with no text in it produced exactly
/// one such row, 31x57 — and snapping to it would put a highlight through
/// something that is not a line.
const MAX_HEIGHT_TO_WIDTH: f64 = 1.2;

/// Detected text rows for the current screen image.
///
/// Empty is the normal, meaningful state: it is what "no scan yet", "the scan
/// found nothing", and "snapping is unavailable here" all reduce to, and every
/// one of them means the marker draws freehand.
#[derive(Debug, Clone, Default)]
pub struct TextSnapMap {
    lines: Vec<TextSnapLine>,
}

impl TextSnapMap {
    /// Build a map, dropping degenerate boxes. Order is not relied upon.
    pub fn new(lines: impl IntoIterator<Item = TextSnapLine>) -> Self {
        Self {
            lines: lines
                .into_iter()
                .filter(|line| line.is_usable() && line.is_row_shaped())
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// The row a pointer at `screen` snaps to, or `None` to draw freehand.
    ///
    /// Ties break toward the row whose center is nearest, so the boundary
    /// between two rows lands where a reader would put it.
    pub fn row_at(&self, screen: (f64, f64)) -> Option<SnappedTextRow> {
        if !screen.0.is_finite() || !screen.1.is_finite() {
            return None;
        }

        self.lines
            .iter()
            .filter(|line| within_reach(**line, screen))
            .min_by(|a, b| {
                let da = (screen.1 - a.center_y()).abs();
                let db = (screen.1 - b.center_y()).abs();
                da.total_cmp(&db)
            })
            .map(|line| snapped_row(*line))
    }
}

fn within_reach(line: TextSnapLine, screen: (f64, f64)) -> bool {
    let vertical = line.height() * VERTICAL_REACH;
    screen.1 >= line.top - vertical
        && screen.1 <= line.bottom + vertical
        && screen.0 >= line.left - HORIZONTAL_REACH
        && screen.0 <= line.right + HORIZONTAL_REACH
}

fn snapped_row(line: TextSnapLine) -> SnappedTextRow {
    let overhang = line.height() * OVERHANG;
    SnappedTextRow {
        center_y: line.center_y(),
        left: line.left - overhang,
        right: line.right + overhang,
        height: line.height(),
    }
}

impl SnappedTextRow {
    /// Stroke thickness for this row, in the same space as the row.
    pub fn thickness(self) -> f64 {
        (self.height * THICKNESS_FACTOR).max(1.0)
    }

    /// Clamp a pointer x to the row's padded extent.
    pub fn clamp_x(self, x: f64) -> f64 {
        x.clamp(self.left, self.right)
    }

    /// The two endpoints of a highlight dragged from `start_x` to `end_x`.
    ///
    /// Both are clamped to the row and the result is always left-to-right, so a
    /// backwards drag produces the same stroke as a forwards one.
    pub fn span(self, start_x: f64, end_x: f64) -> (f64, f64) {
        let first = self.clamp_x(start_x);
        let second = self.clamp_x(end_x);
        (first.min(second), first.max(second))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(left: f64, top: f64, right: f64, bottom: f64) -> TextSnapLine {
        TextSnapLine {
            left,
            top,
            right,
            bottom,
        }
    }

    fn map() -> TextSnapMap {
        // Three 20px rows with 10px of leading, as a text editor would lay out.
        TextSnapMap::new([
            line(100.0, 100.0, 400.0, 120.0),
            line(100.0, 130.0, 300.0, 150.0),
            line(100.0, 160.0, 500.0, 180.0),
        ])
    }

    #[test]
    fn a_pointer_inside_a_row_snaps_to_that_row() {
        let row = map().row_at((200.0, 112.0)).expect("row under the pointer");

        assert_eq!(row.center_y, 110.0);
        assert_eq!(row.height, 20.0);
    }

    #[test]
    fn a_pointer_in_the_leading_snaps_to_the_nearer_row() {
        let map = map();

        let above = map.row_at((200.0, 123.0)).expect("still in reach");
        assert_eq!(above.center_y, 110.0, "closer to the row above");

        let below = map.row_at((200.0, 127.0)).expect("still in reach");
        assert_eq!(below.center_y, 140.0, "closer to the row below");
    }

    #[test]
    fn a_pointer_far_from_every_row_does_not_snap() {
        let map = map();

        assert!(
            map.row_at((200.0, 400.0)).is_none(),
            "well below the last row"
        );
        assert!(
            map.row_at((1200.0, 110.0)).is_none(),
            "past the end of the row's ink and its margin"
        );
    }

    #[test]
    fn a_pointer_just_off_the_end_still_snaps_so_a_highlight_can_start_in_the_margin() {
        let row = map()
            .row_at((330.0, 140.0))
            .expect("within the horizontal reach of the 100..300 row");

        assert_eq!(row.center_y, 140.0);
    }

    #[test]
    fn the_stroke_is_clamped_to_the_row_and_always_runs_left_to_right() {
        let row = map().row_at((200.0, 110.0)).unwrap();

        let (start, end) = row.span(250.0, 180.0);
        assert!(start < end, "a backwards drag still yields a forward span");
        assert_eq!((start, end), (180.0, 250.0));

        let (clamped_start, clamped_end) = row.span(-5000.0, 5000.0);
        assert_eq!(clamped_start, row.left);
        assert_eq!(clamped_end, row.right);
    }

    #[test]
    fn the_committed_stroke_overhangs_the_ink_slightly() {
        let row = map().row_at((200.0, 110.0)).unwrap();

        assert!(row.left < 100.0, "starts a little before the first glyph");
        assert!(row.right > 400.0, "ends a little after the last glyph");
    }

    #[test]
    fn thickness_covers_the_line_rather_than_skimming_it() {
        let row = map().row_at((200.0, 110.0)).unwrap();

        assert!(row.thickness() > row.height);
    }

    #[test]
    fn a_box_taller_than_it_is_wide_is_not_a_row_of_text() {
        // Measured: a busy image with no text produced exactly this row.
        let map = TextSnapMap::new([line(1064.0, 469.0, 1095.0, 526.0)]);

        assert!(map.is_empty());
    }

    #[test]
    fn a_short_real_line_still_counts_as_a_row() {
        // Measured: the shortest line on a code screenshot was 69x13.
        let map = TextSnapMap::new([line(127.0, 134.0, 196.0, 147.0)]);

        assert_eq!(map.len(), 1);
    }

    #[test]
    fn degenerate_and_hairline_boxes_are_dropped_at_construction() {
        let map = TextSnapMap::new([
            line(10.0, 10.0, 10.0, 30.0),
            line(10.0, 30.0, 100.0, 30.0),
            line(10.0, 40.0, 100.0, 42.0),
            line(f64::NAN, 0.0, 100.0, 20.0),
            line(10.0, 60.0, 100.0, 80.0),
        ]);

        assert_eq!(map.len(), 1, "only the one real row survives");
        assert!(map.row_at((50.0, 70.0)).is_some());
    }

    #[test]
    fn an_empty_map_never_snaps() {
        let map = TextSnapMap::default();

        assert!(map.is_empty());
        assert!(map.row_at((10.0, 10.0)).is_none());
    }

    #[test]
    fn a_non_finite_pointer_never_snaps() {
        assert!(map().row_at((f64::NAN, 110.0)).is_none());
        assert!(map().row_at((200.0, f64::INFINITY)).is_none());
    }
}
