//! Text-line geometry extracted from a screen crop.
//!
//! The marker's snap mode needs to know *where* the text rows are, not what
//! they say. Tesseract's TSV output carries both, so the parser here reads only
//! the geometry columns and drops every row's `text` field without storing,
//! returning, or logging it. That is a stronger contract than the copy-to-
//! clipboard path in `mod.rs`, which at least has to hand the text to `wl-copy`:
//! nothing downstream of this module can leak screen content, because no screen
//! content leaves it.
//!
//! ## Why line rows, confirmed by word rows
//!
//! TSV is hierarchical: level 4 is a text line, level 5 is a word inside one.
//! The box comes from the **line** row, because Tesseract has already grouped
//! the words and its box covers all of them — including the ones it read badly.
//! Unioning the word boxes ourselves and dropping low-confidence words instead
//! makes a highlight stop short of the end of a line whenever the last token is
//! something Tesseract struggles with, which over source code is most lines.
//!
//! The word rows still matter, as the test of whether a line is text at all:
//! a line is kept only when it holds at least one non-blank word the engine had
//! some confidence in. That is what keeps icons and gradients from producing
//! rows to snap to.

/// One detected line of text, in source-image pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextLineBox {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

/// Tesseract's TSV hierarchy levels.
const LEVEL_LINE: i32 = 4;
const LEVEL_WORD: i32 = 5;
/// Columns in a Tesseract TSV row, through `text`.
const TSV_COLUMNS: usize = 12;

// Column indices, named so the parser reads as the format rather than as
// arithmetic.
const COL_LEVEL: usize = 0;
const COL_PAGE: usize = 1;
const COL_BLOCK: usize = 2;
const COL_PARAGRAPH: usize = 3;
const COL_LINE: usize = 4;
const COL_LEFT: usize = 6;
const COL_TOP: usize = 7;
const COL_WIDTH: usize = 8;
const COL_HEIGHT: usize = 9;
const COL_CONFIDENCE: usize = 10;
/// The recognized word. Read only to test for blankness; never stored.
const COL_TEXT: usize = 11;

/// How confident Tesseract must be about a line's best word for that line to
/// count as text.
///
/// Deliberately low, and chosen against the engine rather than guessed. Over
/// source code a whole line can read badly — a `println!("{value}");` measured
/// at 23.8 — while a busy image with no text at all produced one row whose best
/// word measured 29.8. The two overlap, so no threshold separates them, and the
/// costs are not symmetric: a missed line means the user cannot snap to the code
/// they wanted to highlight, while a spurious one means an I-beam appears over
/// something that is not text and a press there draws straight instead of
/// freehand. The floor is set to keep the hard lines.
pub(crate) const DEFAULT_MIN_WORD_CONFIDENCE: f64 = 20.0;

/// The line a row belongs to. Tesseract numbers lines within a paragraph and
/// paragraphs within a block, so all four parts are needed to be unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct LineKey {
    page: i32,
    block: i32,
    paragraph: i32,
    line: i32,
}

/// One line's box and whether any word on it confirmed it as text.
#[derive(Debug, Clone, Copy)]
struct LineEntry {
    bounds: Option<TextLineBox>,
    confirmed: bool,
}

/// Parse Tesseract TSV into per-line boxes.
///
/// A line is emitted when it has both a box and at least one non-blank word at
/// or above `min_confidence`. Malformed rows are skipped rather than failing the
/// parse: a single unreadable row should cost one line of snapping, not the
/// whole scan. Lines come back in reading order (top, then left).
pub(crate) fn parse_text_line_boxes(tsv: &str, min_confidence: f64) -> Vec<TextLineBox> {
    let mut keys: Vec<LineKey> = Vec::new();
    let mut entries: Vec<LineEntry> = Vec::new();

    for row in tsv.lines() {
        let Some((key, contribution)) = parse_row(row, min_confidence) else {
            continue;
        };
        let index = match keys.binary_search(&key) {
            Ok(index) => index,
            Err(index) => {
                keys.insert(index, key);
                entries.insert(
                    index,
                    LineEntry {
                        bounds: None,
                        confirmed: false,
                    },
                );
                index
            }
        };
        match contribution {
            RowContribution::LineBounds(bounds) => entries[index].bounds = Some(bounds),
            RowContribution::ConfirmingWord => entries[index].confirmed = true,
        }
    }

    let mut boxes: Vec<TextLineBox> = entries
        .into_iter()
        .filter(|entry| entry.confirmed)
        .filter_map(|entry| entry.bounds)
        .collect();
    boxes.sort_by_key(|line| (line.top, line.left));
    boxes
}

/// What one TSV row tells us about its line.
enum RowContribution {
    /// A level-4 row: the line's authoritative box.
    LineBounds(TextLineBox),
    /// A level-5 row good enough to call the line real. Its box is not used.
    ConfirmingWord,
}

fn parse_row(row: &str, min_confidence: f64) -> Option<(LineKey, RowContribution)> {
    let fields: Vec<&str> = row.split('\t').collect();
    if fields.len() < TSV_COLUMNS {
        return None;
    }
    // The header row fails this parse, which is how it is skipped.
    let level = parse_i32(fields[COL_LEVEL])?;
    if level != LEVEL_LINE && level != LEVEL_WORD {
        return None;
    }

    let key = LineKey {
        page: parse_i32(fields[COL_PAGE])?,
        block: parse_i32(fields[COL_BLOCK])?,
        paragraph: parse_i32(fields[COL_PARAGRAPH])?,
        line: parse_i32(fields[COL_LINE])?,
    };

    if level == LEVEL_WORD {
        // Whether the word is blank is a geometry question here: Tesseract
        // emits empty-text word rows for whitespace runs, and one of those
        // would confirm a line nothing was actually read on. The value itself
        // is not retained.
        if fields[COL_TEXT].trim().is_empty() {
            return None;
        }
        let confidence: f64 = fields[COL_CONFIDENCE].trim().parse().ok()?;
        if !confidence.is_finite() || confidence < min_confidence {
            return None;
        }
        return Some((key, RowContribution::ConfirmingWord));
    }

    let width = parse_i32(fields[COL_WIDTH])?;
    let height = parse_i32(fields[COL_HEIGHT])?;
    if width <= 0 || height <= 0 {
        return None;
    }
    Some((
        key,
        RowContribution::LineBounds(TextLineBox {
            left: parse_i32(fields[COL_LEFT])?,
            top: parse_i32(fields[COL_TOP])?,
            width,
            height,
        }),
    ))
}

fn parse_i32(field: &str) -> Option<i32> {
    field.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext";

    /// A level-4 line row. `id` is `(block, paragraph, line)`, `at` is
    /// `(left, top, width, height)`.
    fn line_row(id: (i32, i32, i32), at: (i32, i32, i32, i32)) -> String {
        let (block, paragraph, line) = id;
        let (left, top, width, height) = at;
        format!("4\t1\t{block}\t{paragraph}\t{line}\t0\t{left}\t{top}\t{width}\t{height}\t-1\t")
    }

    /// A level-5 word row. `id` is `(block, paragraph, line, word)`, `at` is
    /// `(left, top, width, height)`.
    fn word_row(
        id: (i32, i32, i32, i32),
        at: (i32, i32, i32, i32),
        confidence: &str,
        text: &str,
    ) -> String {
        let (block, paragraph, line, word) = id;
        let (left, top, width, height) = at;
        format!(
            "5\t1\t{block}\t{paragraph}\t{line}\t{word}\t{left}\t{top}\t{width}\t{height}\t{confidence}\t{text}"
        )
    }

    fn tsv(rows: &[String]) -> String {
        let mut out = String::from(HEADER);
        for row in rows {
            out.push('\n');
            out.push_str(row);
        }
        out.push('\n');
        out
    }

    #[test]
    fn a_confirmed_line_reports_the_engines_own_box() {
        let input = tsv(&[
            line_row((1, 1, 1), (31, 27, 654, 17)),
            word_row((1, 1, 1, 1), (31, 30, 22, 13), "95.489403", "pub"),
        ]);

        assert_eq!(
            parse_text_line_boxes(&input, DEFAULT_MIN_WORD_CONFIDENCE),
            vec![TextLineBox {
                left: 31,
                top: 27,
                width: 654,
                height: 17,
            }]
        );
    }

    #[test]
    fn a_badly_read_word_at_the_end_of_a_line_does_not_shorten_the_highlight() {
        // Captured from Tesseract 5.5.3 over a code screenshot: the trailing
        // `Vec::new();` measured 43.4 while the rest of the line was above 60.
        // The line row already covers it, which is the whole reason the box
        // comes from there.
        let input = tsv(&[
            line_row((1, 1, 2), (63, 53, 317, 16)),
            word_row((1, 1, 2, 1), (63, 56, 22, 10), "62.013535", "Tet"),
            word_row((1, 1, 2, 6), (295, 53, 85, 16), "43.384228", "Vec::new();"),
        ]);

        let lines = parse_text_line_boxes(&input, DEFAULT_MIN_WORD_CONFIDENCE);

        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0].left + lines[0].width,
            380,
            "the highlight reaches the end of the statement"
        );
    }

    #[test]
    fn a_whole_line_the_engine_read_badly_is_still_a_line() {
        // `println!("{value}");` came back as one word at 23.8 confidence.
        // Snapping to it is exactly what someone demoing code wants.
        let input = tsv(&[
            line_row((2, 1, 1), (85, 128, 164, 21)),
            word_row(
                (2, 1, 1, 1),
                (85, 128, 164, 21),
                "23.752304",
                "printin!(*{value}\");",
            ),
        ]);

        assert_eq!(
            parse_text_line_boxes(&input, DEFAULT_MIN_WORD_CONFIDENCE).len(),
            1
        );
    }

    #[test]
    fn a_line_row_no_word_confirmed_is_not_text() {
        let input = tsv(&[
            line_row((1, 1, 1), (10, 10, 200, 20)),
            word_row((1, 1, 1, 1), (10, 10, 200, 20), "4.5", "~~"),
            word_row((1, 1, 1, 2), (10, 10, 200, 20), "95", "   "),
        ]);

        assert!(
            parse_text_line_boxes(&input, DEFAULT_MIN_WORD_CONFIDENCE).is_empty(),
            "an unreadable smear and a whitespace run do not make a row to snap to"
        );
    }

    #[test]
    fn a_confirming_word_without_a_line_row_yields_no_box() {
        let input = tsv(&[word_row((1, 1, 1, 1), (10, 10, 40, 14), "92", "orphan")]);

        assert!(parse_text_line_boxes(&input, DEFAULT_MIN_WORD_CONFIDENCE).is_empty());
    }

    #[test]
    fn distinct_lines_paragraphs_and_blocks_stay_separate() {
        let input = tsv(&[
            line_row((1, 1, 1), (10, 10, 20, 10)),
            word_row((1, 1, 1, 1), (10, 10, 20, 10), "90", "a"),
            line_row((1, 1, 2), (10, 30, 20, 10)),
            word_row((1, 1, 2, 1), (10, 30, 20, 10), "90", "b"),
            line_row((1, 2, 1), (10, 50, 20, 10)),
            word_row((1, 2, 1, 1), (10, 50, 20, 10), "90", "c"),
            line_row((2, 1, 1), (10, 70, 20, 10)),
            word_row((2, 1, 1, 1), (10, 70, 20, 10), "90", "d"),
        ]);

        let lines = parse_text_line_boxes(&input, DEFAULT_MIN_WORD_CONFIDENCE);

        assert_eq!(
            lines.iter().map(|line| line.top).collect::<Vec<_>>(),
            vec![10, 30, 50, 70]
        );
    }

    #[test]
    fn lines_come_back_in_reading_order_regardless_of_row_order() {
        let input = tsv(&[
            line_row((1, 1, 3), (10, 90, 20, 10)),
            word_row((1, 1, 3, 1), (10, 90, 20, 10), "90", "third"),
            line_row((1, 1, 1), (60, 10, 20, 10)),
            word_row((1, 1, 1, 1), (60, 10, 20, 10), "90", "second"),
            line_row((1, 1, 2), (10, 10, 20, 10)),
            word_row((1, 1, 2, 1), (10, 10, 20, 10), "90", "first"),
        ]);

        let lines = parse_text_line_boxes(&input, DEFAULT_MIN_WORD_CONFIDENCE);

        assert_eq!(
            lines
                .iter()
                .map(|line| (line.top, line.left))
                .collect::<Vec<_>>(),
            vec![(10, 10), (10, 60), (90, 10)]
        );
    }

    #[test]
    fn page_block_and_paragraph_rows_are_ignored() {
        let input = format!(
            "{HEADER}\n\
             1\t1\t0\t0\t0\t0\t0\t0\t1920\t1080\t-1\t\n\
             2\t1\t1\t0\t0\t0\t60\t44\t255\t63\t-1\t\n\
             3\t1\t1\t1\t0\t0\t60\t44\t255\t63\t-1\t\n\
             {}\n{}\n",
            line_row((1, 1, 1), (60, 44, 98, 21)),
            word_row((1, 1, 1, 1), (60, 44, 17, 16), "84.29", "fn")
        );

        let lines = parse_text_line_boxes(&input, DEFAULT_MIN_WORD_CONFIDENCE);

        assert_eq!(lines.len(), 1, "only the level-4 row contributes a box");
        assert_eq!(lines[0].width, 98);
    }

    #[test]
    fn malformed_rows_cost_only_themselves() {
        let input = format!(
            "{HEADER}\n\
             not a row at all\n\
             4\t1\t9\t1\t1\t0\tNaN\t50\t40\t14\t-1\t\n\
             4\t1\t8\t1\t1\t0\t10\t20\t0\t14\t-1\t\n\
             {}\n{}\n",
            line_row((1, 1, 1), (100, 50, 40, 14)),
            word_row((1, 1, 1, 1), (100, 50, 40, 14), "92", "good")
        );

        assert_eq!(
            parse_text_line_boxes(&input, DEFAULT_MIN_WORD_CONFIDENCE),
            vec![TextLineBox {
                left: 100,
                top: 50,
                width: 40,
                height: 14,
            }]
        );
    }

    #[test]
    fn empty_and_header_only_input_produce_no_lines() {
        assert!(parse_text_line_boxes("", DEFAULT_MIN_WORD_CONFIDENCE).is_empty());
        assert!(parse_text_line_boxes(HEADER, DEFAULT_MIN_WORD_CONFIDENCE).is_empty());
    }
}
