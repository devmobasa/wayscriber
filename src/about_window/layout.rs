//! Geometry for the About dialog.
//!
//! The plan is computed before the window is mapped so the surface can be
//! sized to its content instead of a hardcoded 360×220 box that clips the
//! moment a row is added.

use super::content::AboutContent;

/// `(x, y, width, height)` in logical pixels.
pub(super) type Rect = (f64, f64, f64, f64);

pub(super) const WINDOW_WIDTH: f64 = 420.0;
pub(super) const MARGIN: f64 = 22.0;
pub(super) const ICON_SIZE: f64 = 44.0;
pub(super) const CLOSE_SIZE: f64 = 18.0;
const CLOSE_INSET: f64 = 12.0;
pub(super) const CARD_HEIGHT: f64 = 48.0;
pub(super) const ROW_HEIGHT: f64 = 38.0;
const ROW_GAP: f64 = 4.0;
const SECTION_GAP: f64 = 16.0;
const META_LINE_HEIGHT: f64 = 17.0;
pub(super) const BUTTON_HEIGHT: f64 = 28.0;
const BUTTON_GAP: f64 = 8.0;
const HINT_HEIGHT: f64 = 15.0;

pub(super) const TITLE_SIZE: f64 = 19.0;
pub(super) const TAGLINE_SIZE: f64 = 12.5;
pub(super) const CARD_TITLE_SIZE: f64 = 13.0;
pub(super) const ROW_TITLE_SIZE: f64 = 13.0;
pub(super) const DETAIL_SIZE: f64 = 11.5;
pub(super) const META_SIZE: f64 = 11.5;
pub(super) const BUTTON_SIZE: f64 = 12.0;
pub(super) const HINT_SIZE: f64 = 11.0;

/// Positioned elements, in painting order.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Plan {
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) icon: Rect,
    /// Baselines as `(x, y)`.
    pub(super) title: (f64, f64),
    pub(super) tagline: (f64, f64),
    pub(super) version: (f64, f64),
    pub(super) close: Rect,
    pub(super) update_card: Rect,
    pub(super) link_rows: Vec<Rect>,
    pub(super) meta_baselines: Vec<(f64, f64)>,
    pub(super) buttons: Vec<Rect>,
    pub(super) hint: (f64, f64),
}

pub(super) fn plan(content: &AboutContent) -> Plan {
    let width = WINDOW_WIDTH;
    let content_left = MARGIN;
    let content_width = width - MARGIN * 2.0;
    let text_left = content_left + ICON_SIZE + 14.0;

    let icon = (content_left, MARGIN, ICON_SIZE, ICON_SIZE);
    let title = (text_left, MARGIN + 17.0);
    let tagline = (text_left, MARGIN + 33.0);
    let version = (text_left, MARGIN + 47.0);
    let close = (
        width - CLOSE_INSET - CLOSE_SIZE,
        CLOSE_INSET,
        CLOSE_SIZE,
        CLOSE_SIZE,
    );

    let mut y = MARGIN + ICON_SIZE.max(52.0) + SECTION_GAP;

    let update_card = (content_left, y, content_width, CARD_HEIGHT);
    y += CARD_HEIGHT + SECTION_GAP;

    let link_rows: Vec<Rect> = content
        .links
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let row_y = y + index as f64 * (ROW_HEIGHT + ROW_GAP);
            (content_left, row_y, content_width, ROW_HEIGHT)
        })
        .collect();
    if let Some(last) = link_rows.last() {
        y = last.1 + last.3 + SECTION_GAP;
    }

    let meta_baselines: Vec<(f64, f64)> = content
        .meta_lines
        .iter()
        .enumerate()
        .map(|(index, _)| {
            (
                content_left,
                y + index as f64 * META_LINE_HEIGHT + META_SIZE,
            )
        })
        .collect();
    if !meta_baselines.is_empty() {
        y += content.meta_lines.len() as f64 * META_LINE_HEIGHT + SECTION_GAP - 4.0;
    }

    let button_specs = content.buttons();
    let buttons = button_row(content_left, y, content_width, button_specs.len());
    if !buttons.is_empty() {
        y += BUTTON_HEIGHT + 14.0;
    }

    let hint = (content_left, y + HINT_SIZE);
    let height = y + HINT_HEIGHT + MARGIN - 4.0;

    Plan {
        width,
        height,
        icon,
        title,
        tagline,
        version,
        close,
        update_card,
        link_rows,
        meta_baselines,
        buttons,
        hint,
    }
}

/// Evenly split pill buttons across the content width.
fn button_row(left: f64, top: f64, content_width: f64, count: usize) -> Vec<Rect> {
    if count == 0 {
        return Vec::new();
    }
    let count_f = count as f64;
    let button_width = (content_width - BUTTON_GAP * (count_f - 1.0)) / count_f;
    (0..count)
        .map(|index| {
            (
                left + index as f64 * (button_width + BUTTON_GAP),
                top,
                button_width,
                BUTTON_HEIGHT,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bottom(rect: Rect) -> f64 {
        rect.1 + rect.3
    }

    fn right(rect: Rect) -> f64 {
        rect.0 + rect.2
    }

    #[test]
    fn elements_stack_without_overlapping() {
        let content = AboutContent::build();
        let plan = plan(&content);

        assert!(bottom(plan.icon) <= plan.update_card.1);
        let mut previous_bottom = bottom(plan.update_card);
        for row in &plan.link_rows {
            assert!(row.1 >= previous_bottom, "link row overlaps the row above");
            previous_bottom = bottom(*row);
        }
        for baseline in &plan.meta_baselines {
            assert!(baseline.1 > previous_bottom);
        }
        for button in &plan.buttons {
            assert!(button.1 >= previous_bottom);
            assert!(bottom(*button) <= plan.height);
        }
        assert!(plan.hint.1 <= plan.height);
    }

    #[test]
    fn every_element_fits_inside_the_surface() {
        let content = AboutContent::build();
        let plan = plan(&content);

        assert!(right(plan.close) <= plan.width);
        assert!(right(plan.update_card) <= plan.width - MARGIN + 0.01);
        for row in &plan.link_rows {
            assert!(right(*row) <= plan.width - MARGIN + 0.01);
        }
        for button in &plan.buttons {
            assert!(button.0 >= MARGIN - 0.01);
            assert!(right(*button) <= plan.width - MARGIN + 0.01);
        }
        assert!(plan.height > 0.0 && plan.height < 800.0);
    }

    #[test]
    fn height_tracks_the_number_of_rows() {
        let mut content = AboutContent::build();
        let full_height = plan(&content).height;

        content.links.pop();
        let shorter = plan(&content).height;

        assert!(shorter < full_height);
        assert_eq!(full_height - shorter, ROW_HEIGHT + 4.0);
    }

    #[test]
    fn buttons_share_the_content_width() {
        let row = button_row(MARGIN, 100.0, WINDOW_WIDTH - MARGIN * 2.0, 2);

        assert_eq!(row.len(), 2);
        assert!((row[0].2 - row[1].2).abs() < 0.001);
        assert!(row[1].0 >= right(row[0]));
        assert!(right(row[1]) <= WINDOW_WIDTH - MARGIN + 0.001);
        assert!(button_row(MARGIN, 100.0, 300.0, 0).is_empty());
    }
}
