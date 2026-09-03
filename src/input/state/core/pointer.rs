//! Pointer positions, hover invalidation, activity timing, and provisional bounds.

use crate::util::Rect;
use std::time::Instant;

#[derive(Debug, Clone)]
pub(in crate::input::state) struct PointerTracking {
    screen: (i32, i32),
    canvas: (i32, i32),
    seen: bool,
    menu_hover_recalc_pending: bool,
    last_draw_activity: Instant,
    provisional_bounds: Option<Rect>,
}

impl Default for PointerTracking {
    fn default() -> Self {
        Self {
            screen: (0, 0),
            canvas: (0, 0),
            seen: false,
            menu_hover_recalc_pending: false,
            last_draw_activity: Instant::now(),
            provisional_bounds: None,
        }
    }
}

impl PointerTracking {
    pub(in crate::input::state) fn screen(&self) -> (i32, i32) {
        self.screen
    }

    pub(in crate::input::state) fn canvas(&self) -> (i32, i32) {
        self.canvas
    }

    pub(in crate::input::state) fn seen(&self) -> bool {
        self.seen
    }

    pub(in crate::input::state) fn update(&mut self, screen: (i32, i32), canvas: (i32, i32)) {
        self.screen = screen;
        self.canvas = canvas;
        self.seen = true;
    }

    pub(in crate::input::state) fn update_synthetic(
        &mut self,
        screen: (i32, i32),
        canvas: (i32, i32),
    ) {
        self.screen = screen;
        self.canvas = canvas;
    }

    pub(in crate::input::state) fn set_canvas(&mut self, canvas: (i32, i32)) {
        self.canvas = canvas;
    }

    pub(in crate::input::state) fn request_menu_hover_recalc(&mut self) {
        self.menu_hover_recalc_pending = true;
    }

    pub(in crate::input::state) fn take_menu_hover_recalc(&mut self) -> bool {
        std::mem::take(&mut self.menu_hover_recalc_pending)
    }

    pub(in crate::input::state) fn clear_menu_hover_recalc(&mut self) {
        self.menu_hover_recalc_pending = false;
    }

    pub(in crate::input::state) fn mark_draw_activity(&mut self, now: Instant) {
        self.last_draw_activity = now;
    }

    pub(in crate::input::state) fn last_draw_activity(&self) -> Instant {
        self.last_draw_activity
    }

    pub(in crate::input::state) fn provisional_bounds(&self) -> Option<Rect> {
        self.provisional_bounds
    }

    pub(in crate::input::state) fn take_provisional_bounds(&mut self) -> Option<Rect> {
        self.provisional_bounds.take()
    }

    pub(in crate::input::state) fn replace_provisional_bounds(
        &mut self,
        bounds: Option<Rect>,
    ) -> Option<Rect> {
        std::mem::replace(&mut self.provisional_bounds, bounds)
    }

    pub(in crate::input::state) fn union_provisional_bounds(&mut self, bounds: Rect) {
        self.provisional_bounds = match self.provisional_bounds {
            Some(current) => union_rect(current, bounds),
            None => Some(bounds),
        };
    }
}

fn union_rect(a: Rect, b: Rect) -> Option<Rect> {
    let min_x = a.x.min(b.x);
    let min_y = a.y.min(b.y);
    let max_x = a.x.saturating_add(a.width).max(b.x.saturating_add(b.width));
    let max_y =
        a.y.saturating_add(a.height)
            .max(b.y.saturating_add(b.height));
    Rect::from_min_max(min_x, min_y, max_x, max_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::Rect;

    #[test]
    fn real_pointer_updates_mark_seen_while_synthetic_updates_do_not() {
        let mut pointer = PointerTracking::default();

        pointer.update_synthetic((10, 20), (30, 40));
        assert_eq!(pointer.screen(), (10, 20));
        assert_eq!(pointer.canvas(), (30, 40));
        assert!(!pointer.seen());

        pointer.update((50, 60), (70, 80));
        assert_eq!(pointer.screen(), (50, 60));
        assert_eq!(pointer.canvas(), (70, 80));
        assert!(pointer.seen());
    }

    #[test]
    fn menu_hover_recalculation_is_taken_once_and_can_be_cleared() {
        let mut pointer = PointerTracking::default();

        pointer.request_menu_hover_recalc();
        assert!(pointer.take_menu_hover_recalc());
        assert!(!pointer.take_menu_hover_recalc());

        pointer.request_menu_hover_recalc();
        pointer.clear_menu_hover_recalc();
        assert!(!pointer.take_menu_hover_recalc());
    }

    #[test]
    fn provisional_bounds_union_grows_and_replace_returns_the_previous_bounds() {
        let mut pointer = PointerTracking::default();
        let first = Rect::new(10, 20, 30, 40).unwrap();
        let second = Rect::new(0, 50, 20, 30).unwrap();

        assert_eq!(pointer.replace_provisional_bounds(Some(first)), None);
        pointer.union_provisional_bounds(second);
        let combined = Rect::new(0, 20, 40, 60).unwrap();
        assert_eq!(pointer.provisional_bounds(), Some(combined));
        assert_eq!(pointer.replace_provisional_bounds(None), Some(combined));
        assert_eq!(pointer.provisional_bounds(), None);
    }
}
