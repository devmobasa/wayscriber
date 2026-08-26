//! Where the font picker's panel and rows sit.
//!
//! Pure geometry from the surface size, so the renderer and the pointer hit
//! test cannot disagree about which row is under the cursor.

/// Most rows the panel will ever show. Enough to scan, few enough that laying
/// each one out in its own font stays bounded whatever the system has installed.
///
/// A ceiling, not a promise: a short surface shows fewer. Ask the layout for
/// [`FontPickerLayout::visible_rows`] rather than assuming this many.
pub const FONT_PICKER_MAX_VISIBLE: usize = 12;

const PANEL_WIDTH: f64 = 520.0;
const MIN_PANEL_WIDTH: f64 = 240.0;
const PANEL_TOP_RATIO: f64 = 0.14;
const PADDING: f64 = 16.0;
const QUERY_HEIGHT: f64 = 44.0;
const ROW_HEIGHT: f64 = 40.0;
const CAPTION_HEIGHT: f64 = 26.0;
const LIST_GAP: f64 = 10.0;

/// Everything in the panel that is not list: its own padding, the query line,
/// the gap under it, and the caption.
const PANEL_CHROME_HEIGHT: f64 = PADDING * 2.0 + QUERY_HEIGHT + LIST_GAP + CAPTION_HEIGHT;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontPickerLayout {
    pub panel_x: f64,
    pub panel_y: f64,
    pub panel_width: f64,
    pub panel_height: f64,
    pub query_x: f64,
    pub query_y: f64,
    pub query_width: f64,
    pub query_height: f64,
    pub list_x: f64,
    pub list_y: f64,
    pub list_width: f64,
    pub row_height: f64,
    /// Rows the panel has room to draw, never more than
    /// [`FONT_PICKER_MAX_VISIBLE`] and never more than the surface fits.
    ///
    /// Zero when nothing matched: the list reserves height anyway, so the
    /// "no matches" note has a place of its own.
    pub visible_rows: usize,
    /// Height reserved for the list. At least one row even when `visible_rows`
    /// is zero.
    pub list_height: f64,
    pub caption_y: f64,
}

/// One laid-out row, in surface coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct FontPickerRow {
    pub family: String,
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub selected: bool,
    /// Whether this family is the one currently in use.
    pub current: bool,
}

/// Panel geometry for a surface, and how many rows fit in it.
///
/// Three things bound the list, and the smallest wins: how many results there
/// are, [`FONT_PICKER_MAX_VISIBLE`], and how many rows the surface itself has
/// room for. The last is why this takes the surface size — a fixed twelve rows
/// is a 592px panel, taller than some outputs an overlay comes up on, and a
/// panel taller than its surface hides the very rows the arrow keys move into.
///
/// One row is always reserved even with nothing to show, so the "no matches"
/// note lands in the list rather than on the caption.
pub fn font_picker_layout(
    surface_width: u32,
    surface_height: u32,
    row_count: usize,
) -> FontPickerLayout {
    let width = f64::from(surface_width.max(1));
    let height = f64::from(surface_height.max(1));

    let panel_width = PANEL_WIDTH.min(width - PADDING * 2.0).max(MIN_PANEL_WIDTH);

    // Height left for rows once the panel's chrome and the margin around the
    // panel are taken out. Floors at one row: a picker showing none is no use,
    // and on a surface that small the panel overflowing is the lesser problem.
    let room = height - PADDING * 2.0 - PANEL_CHROME_HEIGHT;
    let fits = ((room / ROW_HEIGHT).floor().max(1.0)) as usize;
    let visible_rows = row_count.min(FONT_PICKER_MAX_VISIBLE).min(fits);
    let list_height = visible_rows.max(1) as f64 * ROW_HEIGHT;
    let panel_height = PANEL_CHROME_HEIGHT + list_height;

    let panel_x = ((width - panel_width) / 2.0).max(0.0);
    let panel_y = (height * PANEL_TOP_RATIO)
        .min((height - panel_height - PADDING).max(0.0))
        .max(0.0);

    let query_x = panel_x + PADDING;
    let query_y = panel_y + PADDING;
    let query_width = panel_width - PADDING * 2.0;

    FontPickerLayout {
        panel_x,
        panel_y,
        panel_width,
        panel_height,
        query_x,
        query_y,
        query_width,
        query_height: QUERY_HEIGHT,
        list_x: query_x,
        list_y: query_y + QUERY_HEIGHT + LIST_GAP,
        list_width: query_width,
        row_height: ROW_HEIGHT,
        visible_rows,
        list_height,
        caption_y: panel_y + panel_height - PADDING - CAPTION_HEIGHT / 2.0,
    }
}

/// The rows a layout shows, for the window starting at `scroll`.
pub fn font_picker_rows(
    layout: FontPickerLayout,
    families: &[String],
    scroll: usize,
    selected: usize,
    current: &str,
) -> Vec<FontPickerRow> {
    families
        .iter()
        .enumerate()
        .skip(scroll)
        .take(layout.visible_rows)
        .enumerate()
        .map(|(offset, (index, family))| FontPickerRow {
            family: family.clone(),
            index,
            x: layout.list_x,
            y: layout.list_y + offset as f64 * layout.row_height,
            width: layout.list_width,
            height: layout.row_height,
            selected: index == selected,
            current: crate::draw::families_match(family, current),
        })
        .collect()
}

/// The row index under a surface point, if any.
pub fn font_picker_row_at(
    layout: FontPickerLayout,
    families: &[String],
    scroll: usize,
    x: f64,
    y: f64,
) -> Option<usize> {
    if x < layout.list_x || x > layout.list_x + layout.list_width {
        return None;
    }
    let offset = ((y - layout.list_y) / layout.row_height).floor();
    if offset < 0.0 || offset >= layout.visible_rows as f64 {
        return None;
    }
    let index = scroll.checked_add(offset as usize)?;
    (index < families.len()).then_some(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn families(count: usize) -> Vec<String> {
        (0..count).map(|index| format!("Family {index}")).collect()
    }

    #[test]
    fn the_panel_is_centred_and_stays_inside_the_surface() {
        let layout = font_picker_layout(1920, 1080, 40);

        assert!(layout.panel_x > 0.0);
        assert!((layout.panel_x + layout.panel_width) <= 1920.0);
        assert!((layout.panel_y + layout.panel_height) <= 1080.0);
        assert_eq!(layout.visible_rows, FONT_PICKER_MAX_VISIBLE);
    }

    #[test]
    fn a_short_list_shrinks_the_panel_instead_of_leaving_an_empty_well() {
        let short = font_picker_layout(1920, 1080, 3);
        let long = font_picker_layout(1920, 1080, 40);

        assert_eq!(short.visible_rows, 3);
        assert!(short.panel_height < long.panel_height);
    }

    #[test]
    fn a_narrow_surface_still_produces_a_usable_panel() {
        let layout = font_picker_layout(300, 400, 40);

        assert!(layout.panel_width >= MIN_PANEL_WIDTH);
        assert!(layout.panel_x >= 0.0);
        assert!(layout.panel_y >= 0.0);
    }

    #[test]
    fn a_short_surface_shows_fewer_rows_rather_than_a_panel_taller_than_it_is() {
        // Twelve rows is a 592px panel. On a 400px-tall output that hangs off
        // the bottom, and the rows past the edge are ones the arrow keys can
        // still move into.
        let layout = font_picker_layout(300, 400, 40);

        assert!(
            layout.visible_rows < FONT_PICKER_MAX_VISIBLE,
            "got {} rows on a 400px surface",
            layout.visible_rows
        );
        assert!(
            layout.panel_y + layout.panel_height <= 400.0,
            "panel runs to {}, past the 400px surface",
            layout.panel_y + layout.panel_height
        );
    }

    #[test]
    fn a_tall_surface_is_still_capped_at_the_visible_maximum() {
        let layout = font_picker_layout(1920, 2160, 400);

        assert_eq!(layout.visible_rows, FONT_PICKER_MAX_VISIBLE);
    }

    #[test]
    fn a_search_that_matches_nothing_still_reserves_a_row_of_list() {
        // The "no font matches that" note is drawn in the list. With no height
        // reserved it lands on the caption.
        let layout = font_picker_layout(1920, 1080, 0);

        assert_eq!(layout.visible_rows, 0);
        assert!(layout.list_height >= layout.row_height);
        assert!(
            layout.list_y + layout.list_height <= layout.caption_y,
            "the note's row runs into the caption baseline"
        );
    }

    #[test]
    fn rows_start_at_the_scroll_offset_and_stop_at_the_window() {
        let layout = font_picker_layout(1920, 1080, 40);
        let families = families(40);

        let rows = font_picker_rows(layout, &families, 5, 7, "Family 9");

        assert_eq!(rows.len(), FONT_PICKER_MAX_VISIBLE);
        assert_eq!(rows[0].index, 5);
        assert!(rows.iter().find(|row| row.index == 7).unwrap().selected);
        assert!(rows.iter().find(|row| row.index == 9).unwrap().current);
    }

    #[test]
    fn a_row_hit_test_agrees_with_where_the_rows_were_drawn() {
        let layout = font_picker_layout(1920, 1080, 40);
        let families = families(40);
        let rows = font_picker_rows(layout, &families, 4, 4, "");

        for row in &rows {
            let hit = font_picker_row_at(
                layout,
                &families,
                4,
                row.x + row.width / 2.0,
                row.y + row.height / 2.0,
            );
            assert_eq!(hit, Some(row.index));
        }
    }

    #[test]
    fn points_outside_the_list_hit_nothing() {
        let layout = font_picker_layout(1920, 1080, 40);
        let families = families(40);

        assert_eq!(
            font_picker_row_at(
                layout,
                &families,
                0,
                layout.list_x - 5.0,
                layout.list_y + 5.0
            ),
            None
        );
        assert_eq!(
            font_picker_row_at(
                layout,
                &families,
                0,
                layout.list_x + 5.0,
                layout.list_y - 5.0
            ),
            None
        );
        assert_eq!(
            font_picker_row_at(
                layout,
                &families,
                0,
                layout.list_x + 5.0,
                layout.list_y + layout.row_height * 100.0
            ),
            None
        );
    }

    #[test]
    fn the_last_page_of_a_short_list_does_not_invent_rows() {
        let layout = font_picker_layout(1920, 1080, 5);
        let families = families(5);

        let rows = font_picker_rows(layout, &families, 3, 3, "");

        assert_eq!(rows.len(), 2, "only two rows remain past index 3");
        // The second of those two rows is real and answers.
        assert_eq!(
            font_picker_row_at(
                layout,
                &families,
                3,
                layout.list_x + 5.0,
                layout.list_y + layout.row_height * 1.5
            ),
            Some(4)
        );
        // Past it there is nothing, even though the window has room drawn for it.
        assert_eq!(
            font_picker_row_at(
                layout,
                &families,
                3,
                layout.list_x + 5.0,
                layout.list_y + layout.row_height * 2.5
            ),
            None
        );
    }
}
