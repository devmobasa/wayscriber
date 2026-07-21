//! Pointer hit map for the help overlay.
//!
//! The overlay's geometry is measured with real text metrics, so the only place
//! the true row/search rectangles exist is inside the render pass. Each frame
//! stores the drawn rectangles here (screen space); pointer releases and cursor
//! hints then test against the actual layout instead of an approximate bounding
//! box. Populated and read on the single Wayland event-loop thread, so a plain
//! thread-local is sufficient.

use std::cell::RefCell;

use super::super::types::HelpRowHit;
use crate::config::Action;

/// What sits under a point inside the help overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpOverlayRegion {
    /// Over the search input well (I-beam / text cursor).
    Search,
    /// Over a clickable action row (or the "Replay tour" footer); carries the
    /// action a click should run.
    Row(Action),
    /// Inside the overlay chrome but not over an interactive element.
    Inside,
}

#[derive(Clone, Copy)]
struct HitRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl HitRect {
    fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

struct HelpHitMap {
    box_rect: HitRect,
    search_rect: Option<HitRect>,
    rows: Vec<(HitRect, Action)>,
}

thread_local! {
    static HIT_MAP: RefCell<Option<HelpHitMap>> = const { RefCell::new(None) };
}

fn rect(tuple: (f64, f64, f64, f64)) -> HitRect {
    HitRect {
        x: tuple.0,
        y: tuple.1,
        w: tuple.2,
        h: tuple.3,
    }
}

/// Store the rectangles drawn this frame so pointer code can hit-test the real
/// layout. Called once at the end of the render pass.
pub(super) fn store_help_hit_map(
    box_rect: (f64, f64, f64, f64),
    search_rect: Option<(f64, f64, f64, f64)>,
    rows: &[HelpRowHit],
) {
    let rows = rows
        .iter()
        .map(|hit| {
            (
                HitRect {
                    x: hit.x,
                    y: hit.y,
                    w: hit.w,
                    h: hit.h,
                },
                hit.action,
            )
        })
        .collect();
    HIT_MAP.with(|cell| {
        *cell.borrow_mut() = Some(HelpHitMap {
            box_rect: rect(box_rect),
            search_rect: search_rect.map(rect),
            rows,
        });
    });
}

/// Region under `(x, y)` in the last-rendered help overlay, or `None` when the
/// point is outside the overlay box (or the overlay has not rendered yet).
///
/// Rows win over the search well, which wins over bare chrome, so the most
/// specific interactive target is always reported.
pub fn help_overlay_region_at(x: f64, y: f64) -> Option<HelpOverlayRegion> {
    HIT_MAP.with(|cell| {
        let map = cell.borrow();
        let map = map.as_ref()?;
        if !map.box_rect.contains(x, y) {
            return None;
        }
        for (rect, action) in &map.rows {
            if rect.contains(x, y) {
                return Some(HelpOverlayRegion::Row(*action));
            }
        }
        if let Some(search) = &map.search_rect
            && search.contains(x, y)
        {
            return Some(HelpOverlayRegion::Search);
        }
        Some(HelpOverlayRegion::Inside)
    })
}

/// Drop the stored hit map (called when the overlay closes) so stale rectangles
/// can never answer a hit test.
pub fn clear_help_overlay_hit_map() {
    HIT_MAP.with(|cell| *cell.borrow_mut() = None);
}

/// Install a known hit map for tests that exercise the pointer plumbing without
/// a real render pass. Takes plain tuples so callers outside this module need
/// not name the crate-private [`HelpRowHit`].
#[cfg(test)]
pub fn install_help_hit_map_for_test(
    box_rect: (f64, f64, f64, f64),
    search_rect: Option<(f64, f64, f64, f64)>,
    rows: &[(f64, f64, f64, f64, Action)],
) {
    let rows: Vec<HelpRowHit> = rows
        .iter()
        .map(|&(x, y, w, h, action)| HelpRowHit { x, y, w, h, action })
        .collect();
    store_help_hit_map(box_rect, search_rect, &rows);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_hit(x: f64, y: f64, w: f64, h: f64, action: Action) -> HelpRowHit {
        HelpRowHit { x, y, w, h, action }
    }

    #[test]
    fn region_reports_none_outside_the_box() {
        store_help_hit_map((100.0, 100.0, 200.0, 200.0), None, &[]);
        assert_eq!(help_overlay_region_at(50.0, 50.0), None);
        assert_eq!(help_overlay_region_at(320.0, 150.0), None);
        assert_eq!(
            help_overlay_region_at(150.0, 150.0),
            Some(HelpOverlayRegion::Inside)
        );
        clear_help_overlay_hit_map();
    }

    #[test]
    fn rows_win_over_search_and_chrome() {
        let rows = [row_hit(120.0, 180.0, 160.0, 30.0, Action::ToggleHelp)];
        store_help_hit_map(
            (100.0, 100.0, 200.0, 200.0),
            Some((110.0, 130.0, 180.0, 24.0)),
            &rows,
        );

        assert_eq!(
            help_overlay_region_at(150.0, 190.0),
            Some(HelpOverlayRegion::Row(Action::ToggleHelp))
        );
        assert_eq!(
            help_overlay_region_at(150.0, 140.0),
            Some(HelpOverlayRegion::Search)
        );
        assert_eq!(
            help_overlay_region_at(150.0, 250.0),
            Some(HelpOverlayRegion::Inside)
        );
        clear_help_overlay_hit_map();
    }

    #[test]
    fn cleared_map_answers_none() {
        store_help_hit_map((0.0, 0.0, 100.0, 100.0), None, &[]);
        clear_help_overlay_hit_map();
        assert_eq!(help_overlay_region_at(10.0, 10.0), None);
    }
}
