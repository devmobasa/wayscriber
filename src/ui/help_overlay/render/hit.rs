//! Temporary owner-free interaction adapter for existing help callers.
//! New result-returning paint entry points keep their hit maps with the caller.

use crate::help_overlay_interaction::{HelpHitMap, HelpOverlayRegion};
use std::cell::RefCell;

thread_local! {
    static HIT_MAP: RefCell<Option<HelpHitMap>> = const { RefCell::new(None) };
}

pub(super) fn store_help_hit_map(map: HelpHitMap) {
    HIT_MAP.with(|cell| *cell.borrow_mut() = Some(map));
}

/// Region under a point in the last overlay painted through the legacy renderer.
/// Result-returning renderers instead provide an independent owned map.
pub fn help_overlay_region_at(x: f64, y: f64) -> Option<HelpOverlayRegion> {
    HIT_MAP.with(|cell| cell.borrow().as_ref()?.region_at(x, y))
}

/// Clear the legacy renderer's last-painted interaction geometry.
pub fn clear_help_overlay_hit_map() {
    HIT_MAP.with(|cell| *cell.borrow_mut() = None);
}

/// Install geometry for tests of the legacy pointer plumbing.
#[cfg(test)]
pub fn install_help_hit_map_for_test(
    box_rect: (f64, f64, f64, f64),
    search_rect: Option<(f64, f64, f64, f64)>,
    rows: &[(f64, f64, f64, f64, crate::config::Action)],
) {
    store_help_hit_map(HelpHitMap::new(
        box_rect,
        search_rect,
        rows.iter()
            .map(|&(x, y, w, h, action)| ((x, y, w, h), action)),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::super::types::HelpRowHit;
    use crate::config::Action;

    fn store_help_hit_map(
        box_rect: (f64, f64, f64, f64),
        search_rect: Option<(f64, f64, f64, f64)>,
        rows: &[HelpRowHit],
    ) {
        super::store_help_hit_map(HelpHitMap::new(
            box_rect,
            search_rect,
            rows.iter()
                .map(|hit| ((hit.x, hit.y, hit.w, hit.h), hit.action)),
        ));
    }

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
