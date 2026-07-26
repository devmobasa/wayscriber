//! Pointer hit geometry for one help-overlay owner.
//!
//! The overlay's geometry is measured with real text metrics, so the true
//! row/search rectangles only exist after rendering. A render frame carries
//! this owned map back to the matching [`crate::input::InputState`], which then
//! uses it for pointer releases and cursor hints until the overlay closes or a
//! newer frame replaces it.

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

struct HelpHitGeometry {
    box_rect: HitRect,
    search_rect: Option<HitRect>,
    rows: Vec<(HitRect, Action)>,
}

/// Last-rendered hit geometry owned by one input state.
pub(crate) struct HelpOverlayHitMap {
    geometry: Option<HelpHitGeometry>,
}

impl HelpOverlayHitMap {
    pub(crate) fn new() -> Self {
        Self { geometry: None }
    }

    /// Build the map carried by one completed render frame.
    pub(super) fn from_rendered_frame(
        box_rect: (f64, f64, f64, f64),
        search_rect: Option<(f64, f64, f64, f64)>,
        rows: &[HelpRowHit],
    ) -> Self {
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

        Self {
            geometry: Some(HelpHitGeometry {
                box_rect: rect(box_rect),
                search_rect: search_rect.map(rect),
                rows,
            }),
        }
    }

    /// Region under `(x, y)` in this owner's last-rendered help overlay.
    pub(crate) fn region_at(&self, x: f64, y: f64) -> Option<HelpOverlayRegion> {
        let map = self.geometry.as_ref()?;
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
    }

    /// Drop this owner's stored geometry so a later open cannot use stale
    /// rectangles before its first fresh render.
    pub(crate) fn clear(&mut self) {
        self.geometry = None;
    }

    /// Install fixture geometry without requiring a Cairo render pass.
    #[cfg(test)]
    pub(crate) fn install_for_test(
        &mut self,
        box_rect: (f64, f64, f64, f64),
        search_rect: Option<(f64, f64, f64, f64)>,
        rows: &[(f64, f64, f64, f64, Action)],
    ) {
        let rows: Vec<HelpRowHit> = rows
            .iter()
            .map(|&(x, y, w, h, action)| HelpRowHit { x, y, w, h, action })
            .collect();
        *self = Self::from_rendered_frame(box_rect, search_rect, &rows);
    }
}

fn rect(tuple: (f64, f64, f64, f64)) -> HitRect {
    HitRect {
        x: tuple.0,
        y: tuple.1,
        w: tuple.2,
        h: tuple.3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_hit(x: f64, y: f64, w: f64, h: f64, action: Action) -> HelpRowHit {
        HelpRowHit { x, y, w, h, action }
    }

    #[test]
    fn region_reports_none_outside_the_box() {
        let map = HelpOverlayHitMap::from_rendered_frame((100.0, 100.0, 200.0, 200.0), None, &[]);

        assert_eq!(map.region_at(50.0, 50.0), None);
        assert_eq!(map.region_at(320.0, 150.0), None);
        assert_eq!(map.region_at(150.0, 150.0), Some(HelpOverlayRegion::Inside));
    }

    #[test]
    fn rows_win_over_search_and_chrome() {
        let rows = [row_hit(120.0, 180.0, 160.0, 30.0, Action::ToggleHelp)];
        let map = HelpOverlayHitMap::from_rendered_frame(
            (100.0, 100.0, 200.0, 200.0),
            Some((110.0, 130.0, 180.0, 24.0)),
            &rows,
        );

        assert_eq!(
            map.region_at(150.0, 190.0),
            Some(HelpOverlayRegion::Row(Action::ToggleHelp))
        );
        assert_eq!(map.region_at(150.0, 140.0), Some(HelpOverlayRegion::Search));
        assert_eq!(map.region_at(150.0, 250.0), Some(HelpOverlayRegion::Inside));
    }

    #[test]
    fn cleared_map_answers_none() {
        let mut map = HelpOverlayHitMap::from_rendered_frame((0.0, 0.0, 100.0, 100.0), None, &[]);
        map.clear();

        assert_eq!(map.region_at(10.0, 10.0), None);
    }
}
