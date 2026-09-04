//! Owned interaction geometry produced by a help-overlay paint pass.

use crate::config::Action;

/// What sits under a point inside the help overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpOverlayRegion {
    /// The search input well.
    Search,
    /// A clickable action row or footer action.
    Row(Action),
    /// Overlay chrome outside an interactive element.
    Inside,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HitRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl HitRect {
    fn from_tuple((x, y, w, h): (f64, f64, f64, f64)) -> Self {
        Self { x, y, w, h }
    }

    fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}

/// Last-painted screen-space geometry for one help overlay.
///
/// The default map is empty. Rows are tested in insertion order before the
/// search well and bare chrome, with the outer bounds checked first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HelpHitMap {
    box_rect: Option<HitRect>,
    search_rect: Option<HitRect>,
    rows: Vec<(HitRect, Action)>,
}

impl HelpHitMap {
    /// Own the actual rectangles produced by painting. Coordinates are logical
    /// screen pixels; rectangle tuples are `(x, y, width, height)`.
    pub fn new(
        box_rect: (f64, f64, f64, f64),
        search_rect: Option<(f64, f64, f64, f64)>,
        rows: impl IntoIterator<Item = ((f64, f64, f64, f64), Action)>,
    ) -> Self {
        Self {
            box_rect: Some(HitRect::from_tuple(box_rect)),
            search_rect: search_rect.map(HitRect::from_tuple),
            rows: rows
                .into_iter()
                .map(|(rect, action)| (HitRect::from_tuple(rect), action))
                .collect(),
        }
    }

    /// Return the most specific target at a point, including rectangle edges.
    pub fn region_at(&self, x: f64, y: f64) -> Option<HelpOverlayRegion> {
        if !self.box_rect?.contains(x, y) {
            return None;
        }
        for &(rect, action) in &self.rows {
            if rect.contains(x, y) {
                return Some(HelpOverlayRegion::Row(action));
            }
        }
        if self.search_rect.is_some_and(|rect| rect.contains(x, y)) {
            return Some(HelpOverlayRegion::Search);
        }
        Some(HelpOverlayRegion::Inside)
    }
}

/// Owned scroll extent and interaction geometry from a single paint pass.
#[derive(Clone, Debug, PartialEq)]
pub struct HelpRenderResult {
    pub scroll_max: f64,
    pub hit_map: HelpHitMap,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outer_bounds_reject_even_rows_that_extend_outside() {
        let map = HelpHitMap::new(
            (10.0, 10.0, 20.0, 20.0),
            None,
            [((0.0, 0.0, 40.0, 40.0), Action::ToggleHelp)],
        );
        assert_eq!(map.region_at(9.0, 15.0), None);
        assert_eq!(map.region_at(31.0, 15.0), None);
        assert_eq!(
            map.region_at(10.0, 10.0),
            Some(HelpOverlayRegion::Row(Action::ToggleHelp))
        );
        assert_eq!(
            map.region_at(30.0, 30.0),
            Some(HelpOverlayRegion::Row(Action::ToggleHelp))
        );
    }

    #[test]
    fn overlapping_rows_keep_order_and_precede_search_and_chrome() {
        let map = HelpHitMap::new(
            (0.0, 0.0, 100.0, 100.0),
            Some((10.0, 10.0, 60.0, 60.0)),
            [
                ((20.0, 20.0, 20.0, 20.0), Action::ToggleHelp),
                ((20.0, 20.0, 30.0, 30.0), Action::OpenAbout),
            ],
        );
        assert_eq!(
            map.region_at(30.0, 30.0),
            Some(HelpOverlayRegion::Row(Action::ToggleHelp))
        );
        assert_eq!(
            map.region_at(45.0, 45.0),
            Some(HelpOverlayRegion::Row(Action::OpenAbout))
        );
        assert_eq!(map.region_at(60.0, 60.0), Some(HelpOverlayRegion::Search));
        assert_eq!(map.region_at(90.0, 90.0), Some(HelpOverlayRegion::Inside));
    }

    #[test]
    fn independent_maps_and_empty_map_keep_their_own_geometry() {
        let first = HelpHitMap::new((0.0, 0.0, 20.0, 20.0), None, []);
        let second = HelpHitMap::new((100.0, 100.0, 20.0, 20.0), None, []);
        let retained = first.clone();
        drop(first);
        assert_eq!(HelpHitMap::default().region_at(0.0, 0.0), None);
        assert_eq!(
            retained.region_at(5.0, 5.0),
            Some(HelpOverlayRegion::Inside)
        );
        assert_eq!(second.region_at(5.0, 5.0), None);
        assert_eq!(
            second.region_at(105.0, 105.0),
            Some(HelpOverlayRegion::Inside)
        );
    }
}
