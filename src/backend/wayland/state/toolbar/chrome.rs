use std::time::{Duration, Instant};

use crate::{
    backend::wayland::{
        toolbar::{
            ToolbarCursorHint,
            hit::{
                HitRegion, drag_intent_for_hit, focus_hover_point, focused_event, intent_for_hit,
                next_focus_index, quick_color_slot_for_hit, resolve_focus_index,
            },
            render::TOOLTIP_DELAY,
        },
        toolbar_intent::ToolbarIntent,
    },
    ui::toolbar::{
        ToolbarEvent,
        snapshot::fade::{TopStripFade, TopStripFadeInputs},
    },
};

const TOOLBAR_CONFIGURE_FAIL_THRESHOLD: u32 = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum ConfigureVerdict {
    Ok,
    StillWaiting,
    FallBackToInline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) struct HoverChange {
    pub target_changed: bool,
    pub position_changed: bool,
}

#[derive(Debug, Default)]
struct InlineTopStrip {
    hits: Vec<HitRegion>,
    rect: Option<(f64, f64, f64, f64)>,
    hover: Option<(f64, f64)>,
    hover_start: Option<Instant>,
    tooltip_pending: bool,
    focus_index: Option<usize>,
    focus_id: Option<String>,
}

impl InlineTopStrip {
    fn hit_index_at(&self, position: (f64, f64)) -> Option<usize> {
        self.hits
            .iter()
            .position(|hit| hit.contains(position.0, position.1))
    }

    fn contains(&self, position: (f64, f64)) -> bool {
        self.rect.is_some_and(|(x, y, w, h)| {
            super::geometry::point_in_rect(position.0, position.1, x, y, w, h)
        })
    }

    fn primary_hit_at(&self, position: (f64, f64)) -> Option<(ToolbarIntent, bool)> {
        self.hits
            .iter()
            .find_map(|hit| intent_for_hit(hit, position.0, position.1))
    }

    fn quick_color_slot_at(&self, position: (f64, f64)) -> Option<usize> {
        self.hits
            .iter()
            .find_map(|hit| quick_color_slot_for_hit(hit, position.0, position.1))
    }

    fn drag_hit_at(&self, position: (f64, f64)) -> Option<ToolbarIntent> {
        self.hits
            .iter()
            .find_map(|hit| drag_intent_for_hit(hit, position.0, position.1))
    }

    fn set_hover(&mut self, hover: Option<(f64, f64)>, now: Instant) -> HoverChange {
        let previous_hover = self.hover;
        let previous_hit = previous_hover.and_then(|position| self.hit_index_at(position));

        if hover.is_some() && previous_hover.is_none() {
            self.hover_start = Some(now);
        } else if hover.is_none() {
            self.hover_start = None;
        }
        self.hover = hover;

        let hit = hover.and_then(|position| self.hit_index_at(position));
        let target_changed = previous_hover.is_some() != hover.is_some() || previous_hit != hit;
        if target_changed {
            let hit_has_tooltip = hit
                .and_then(|index| self.hits.get(index))
                .is_some_and(|hit| hit.tooltip.is_some());
            self.tooltip_pending = hit_has_tooltip
                && self
                    .hover_start
                    .is_some_and(|start| now.saturating_duration_since(start) < TOOLTIP_DELAY);
        }

        HoverChange {
            target_changed,
            position_changed: previous_hover != hover,
        }
    }

    fn clear_hover(&mut self) -> bool {
        let changed = self.hover.is_some() || self.tooltip_pending;
        self.hover = None;
        self.hover_start = None;
        self.tooltip_pending = false;
        changed
    }

    fn clear_hits(&mut self) {
        self.hits.clear();
        self.rect = None;
    }

    fn set_rendered(&mut self, hits: Vec<HitRegion>, rect: (f64, f64, f64, f64)) {
        self.hits = hits;
        self.rect = Some(rect);
    }

    fn resolved_focus_index(&self) -> Option<usize> {
        resolve_focus_index(&self.hits, self.focus_index, self.focus_id.as_deref())
    }

    fn focus_next(&mut self, reverse: bool) -> bool {
        let current = self.resolved_focus_index();
        let mut next = next_focus_index(&self.hits, current, reverse);
        for _ in 0..self.hits.len() {
            let Some(index) = next else {
                break;
            };
            if self.hits[index].focus_id.is_some() {
                break;
            }
            next = next_focus_index(&self.hits, next, reverse);
        }
        if next == current || next.is_some_and(|index| self.hits[index].focus_id.is_none()) {
            return false;
        }
        self.focus_id = next.and_then(|index| self.hits[index].focus_id.clone());
        self.focus_index = next;
        true
    }

    fn clear_focus(&mut self) -> bool {
        let changed = self.focus_index.is_some() || self.focus_id.is_some();
        self.focus_index = None;
        self.focus_id = None;
        changed
    }

    fn focused_event(&self) -> Option<ToolbarEvent> {
        focused_event(&self.hits, self.resolved_focus_index())
    }

    fn focus_hover(&self) -> Option<(f64, f64)> {
        focus_hover_point(&self.hits, self.resolved_focus_index())
    }

    fn cursor_hint(&self) -> Option<ToolbarCursorHint> {
        let (x, y) = self.hover?;
        self.hits
            .iter()
            .find(|hit| hit.contains(x, y))
            .map_or(Some(ToolbarCursorHint::Default), |hit| {
                Some(hit.kind.cursor_hint())
            })
    }

    fn tooltip_timeout(&self, now: Instant) -> Option<Duration> {
        if !self.tooltip_pending {
            return None;
        }
        self.hover_start.map(|start| {
            start
                .checked_add(TOOLTIP_DELAY)
                .unwrap_or(start)
                .saturating_duration_since(now)
        })
    }

    fn take_tooltip_due(&mut self, now: Instant) -> bool {
        if self.tooltip_timeout(now) != Some(Duration::ZERO) {
            return false;
        }
        self.tooltip_pending = false;
        true
    }
}

pub(in crate::backend::wayland) struct ToolbarChrome {
    pointer_over_toolbar: bool,
    needs_recreate: bool,
    layer_shell_missing_logged: bool,
    inline_toolbars: bool,
    top_offset: (f64, f64),
    configure_miss_count: u32,
    last_applied_top_margin: Option<(i32, i32)>,
    top_strip_fade: TopStripFade,
    gtk_top_hover: bool,
    focus_active: bool,
    inline: InlineTopStrip,
}

impl ToolbarChrome {
    pub(in crate::backend::wayland) fn new(inline_toolbars: bool, top_offset: (f64, f64)) -> Self {
        Self {
            pointer_over_toolbar: false,
            needs_recreate: true,
            layer_shell_missing_logged: false,
            inline_toolbars,
            top_offset,
            configure_miss_count: 0,
            last_applied_top_margin: None,
            top_strip_fade: TopStripFade::new(),
            gtk_top_hover: false,
            focus_active: false,
            inline: InlineTopStrip::default(),
        }
    }

    pub(in crate::backend::wayland) fn pointer_over_toolbar(&self) -> bool {
        self.pointer_over_toolbar
    }

    pub(in crate::backend::wayland) fn set_pointer_over_toolbar(&mut self, value: bool) {
        self.pointer_over_toolbar = value;
    }

    pub(in crate::backend::wayland) fn needs_recreate(&self) -> bool {
        self.needs_recreate
    }

    pub(in crate::backend::wayland) fn set_needs_recreate(&mut self, value: bool) {
        self.needs_recreate = value;
    }

    pub(in crate::backend::wayland) fn inline_toolbars(&self) -> bool {
        self.inline_toolbars
    }

    pub(in crate::backend::wayland) fn top_offset(&self) -> (f64, f64) {
        self.top_offset
    }

    pub(in crate::backend::wayland) fn set_top_offset(&mut self, offset: (f64, f64)) {
        self.top_offset = offset;
    }

    pub(in crate::backend::wayland) fn add_top_offset(&mut self, delta: (f64, f64)) {
        self.top_offset.0 += delta.0;
        self.top_offset.1 += delta.1;
    }

    pub(in crate::backend::wayland) fn note_layer_shell_missing(&mut self) -> bool {
        if self.layer_shell_missing_logged {
            return false;
        }
        self.layer_shell_missing_logged = true;
        true
    }

    pub(in crate::backend::wayland) fn note_configure_result(
        &mut self,
        configured: bool,
    ) -> ConfigureVerdict {
        if configured {
            self.configure_miss_count = 0;
            return ConfigureVerdict::Ok;
        }
        self.configure_miss_count = self.configure_miss_count.saturating_add(1);
        if self.configure_miss_count > TOOLBAR_CONFIGURE_FAIL_THRESHOLD {
            self.configure_miss_count = 0;
            self.inline_toolbars = true;
            ConfigureVerdict::FallBackToInline
        } else {
            ConfigureVerdict::StillWaiting
        }
    }

    pub(in crate::backend::wayland) fn configure_miss_count(&self) -> u32 {
        self.configure_miss_count
    }

    pub(in crate::backend::wayland) fn reset_configure_misses(&mut self) {
        self.configure_miss_count = 0;
    }

    pub(in crate::backend::wayland) fn apply_margins(&mut self, margins: (i32, i32)) -> bool {
        if self.last_applied_top_margin == Some(margins) {
            return false;
        }
        self.last_applied_top_margin = Some(margins);
        true
    }

    pub(in crate::backend::wayland) fn last_applied_margins(&self) -> Option<(i32, i32)> {
        self.last_applied_top_margin
    }

    pub(in crate::backend::wayland) fn reset_margins(&mut self) {
        self.last_applied_top_margin = None;
    }

    pub(in crate::backend::wayland) fn set_gtk_top_hover(&mut self, hovered: bool) {
        self.gtk_top_hover = hovered;
    }

    pub(in crate::backend::wayland) fn focus_active(&self) -> bool {
        self.focus_active
    }

    pub(in crate::backend::wayland) fn set_focus_active(&mut self, active: bool) {
        self.focus_active = active;
    }

    pub(in crate::backend::wayland) fn inline_rect(&self) -> Option<(f64, f64, f64, f64)> {
        self.inline.rect
    }

    pub(in crate::backend::wayland) fn inline_hover(&self) -> Option<(f64, f64)> {
        self.inline.hover
    }

    pub(in crate::backend::wayland) fn inline_hover_start(&self) -> Option<Instant> {
        self.inline.hover_start
    }

    pub(in crate::backend::wayland) fn inline_contains(&self, position: (f64, f64)) -> bool {
        self.inline.contains(position)
    }

    pub(in crate::backend::wayland) fn inline_primary_hit_at(
        &self,
        position: (f64, f64),
    ) -> Option<(ToolbarIntent, bool)> {
        self.inline.primary_hit_at(position)
    }

    pub(in crate::backend::wayland) fn inline_quick_color_slot_at(
        &self,
        position: (f64, f64),
    ) -> Option<usize> {
        self.inline.quick_color_slot_at(position)
    }

    pub(in crate::backend::wayland) fn inline_drag_hit_at(
        &self,
        position: (f64, f64),
    ) -> Option<ToolbarIntent> {
        self.inline.drag_hit_at(position)
    }

    pub(in crate::backend::wayland) fn set_inline_hover(
        &mut self,
        hover: Option<(f64, f64)>,
        now: Instant,
    ) -> HoverChange {
        self.inline.set_hover(hover, now)
    }

    pub(in crate::backend::wayland) fn clear_inline_hover(&mut self) -> bool {
        self.inline.clear_hover()
    }

    pub(in crate::backend::wayland) fn clear_inline_hits(&mut self) {
        self.inline.clear_hits();
    }

    pub(in crate::backend::wayland) fn set_inline_rendered(
        &mut self,
        hits: Vec<HitRegion>,
        rect: (f64, f64, f64, f64),
    ) {
        self.inline.set_rendered(hits, rect);
    }

    pub(in crate::backend::wayland) fn inline_focus_next(&mut self, reverse: bool) -> bool {
        self.inline.focus_next(reverse)
    }

    pub(in crate::backend::wayland) fn clear_inline_focus(&mut self) -> bool {
        self.inline.clear_focus()
    }

    pub(in crate::backend::wayland) fn inline_focused_event(&self) -> Option<ToolbarEvent> {
        self.inline.focused_event()
    }

    pub(in crate::backend::wayland) fn inline_focus_hover(&self) -> Option<(f64, f64)> {
        self.inline.focus_hover()
    }

    pub(in crate::backend::wayland) fn inline_cursor_hint(&self) -> Option<ToolbarCursorHint> {
        self.inline.cursor_hint()
    }

    pub(in crate::backend::wayland) fn inline_tooltip_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        self.inline.tooltip_timeout(now)
    }

    pub(in crate::backend::wayland) fn take_inline_tooltip_due(&mut self, now: Instant) -> bool {
        self.inline.take_tooltip_due(now)
    }

    pub(in crate::backend::wayland) fn fade(&self) -> &TopStripFade {
        &self.top_strip_fade
    }

    pub(in crate::backend::wayland) fn fade_mut(&mut self) -> &mut TopStripFade {
        &mut self.top_strip_fade
    }

    pub(in crate::backend::wayland) fn fade_inputs(
        &self,
        toolbar_pointer_present: bool,
        idle_for: Duration,
        menus_open: bool,
        reduced_chrome: bool,
        idle_fade_enabled: bool,
    ) -> TopStripFadeInputs {
        TopStripFadeInputs {
            idle_for,
            pointer_near: self.pointer_over_toolbar
                || toolbar_pointer_present
                || self.inline.hover.is_some()
                || self.gtk_top_hover,
            menus_open,
            reduced_chrome,
            idle_fade_enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::toolbar::events::HitKind;

    fn hit(id: Option<&str>, x: f64, tooltip: bool) -> HitRegion {
        HitRegion {
            focus_id: id.map(str::to_owned),
            rect: (x, 0.0, 20.0, 20.0),
            event: ToolbarEvent::Undo,
            kind: HitKind::Click,
            tooltip: tooltip.then(|| "Undo".to_owned()),
        }
    }

    #[test]
    fn configure_fallback_occurs_once_after_the_waiting_threshold() {
        let mut chrome = ToolbarChrome::new(false, (0.0, 0.0));
        assert_eq!(
            chrome.note_configure_result(false),
            ConfigureVerdict::StillWaiting
        );
        assert_eq!(chrome.note_configure_result(true), ConfigureVerdict::Ok);
        assert_eq!(chrome.configure_miss_count(), 0);

        for _ in 0..TOOLBAR_CONFIGURE_FAIL_THRESHOLD {
            assert_eq!(
                chrome.note_configure_result(false),
                ConfigureVerdict::StillWaiting
            );
        }
        assert_eq!(
            chrome.note_configure_result(false),
            ConfigureVerdict::FallBackToInline
        );
        assert!(chrome.inline_toolbars());
        assert_eq!(chrome.configure_miss_count(), 0);
        assert_eq!(chrome.note_configure_result(true), ConfigureVerdict::Ok);
    }

    #[test]
    fn applying_margins_reports_only_real_changes() {
        let mut chrome = ToolbarChrome::new(false, (0.0, 0.0));
        assert!(chrome.apply_margins((10, 20)));
        assert!(!chrome.apply_margins((10, 20)));
        assert!(chrome.apply_margins((11, 20)));
        assert!(chrome.apply_margins((11, 21)));
    }

    #[test]
    fn inline_bounds_include_edges_and_reject_points_beyond_them() {
        let mut chrome = ToolbarChrome::new(true, (0.0, 0.0));
        chrome.set_inline_rendered(Vec::new(), (10.0, 20.0, 100.0, 50.0));

        assert!(!chrome.inline_contains((9.9, 45.0)));
        assert!(!chrome.inline_contains((110.1, 45.0)));
        assert!(!chrome.inline_contains((60.0, 19.9)));
        assert!(!chrome.inline_contains((60.0, 70.1)));
        assert!(chrome.inline_contains((10.0, 20.0)));
        assert!(chrome.inline_contains((110.0, 70.0)));
    }

    #[test]
    fn hover_change_tracks_hit_identity_instead_of_pointer_pixels() {
        let mut chrome = ToolbarChrome::new(true, (0.0, 0.0));
        chrome.set_inline_rendered(
            vec![hit(Some("one"), 0.0, false), hit(Some("two"), 40.0, false)],
            (0.0, 0.0, 100.0, 20.0),
        );
        let now = Instant::now();
        assert!(
            chrome
                .set_inline_hover(Some((5.0, 5.0)), now)
                .target_changed
        );
        assert!(
            !chrome
                .set_inline_hover(Some((6.0, 5.0)), now)
                .target_changed
        );
        assert!(
            chrome
                .set_inline_hover(Some((45.0, 5.0)), now)
                .target_changed
        );
    }

    #[test]
    fn tooltip_is_pending_only_during_its_delay_window() {
        let mut chrome = ToolbarChrome::new(true, (0.0, 0.0));
        chrome.set_inline_rendered(vec![hit(Some("one"), 0.0, true)], (0.0, 0.0, 20.0, 20.0));
        let start = Instant::now();
        chrome.set_inline_hover(Some((5.0, 5.0)), start);
        assert_eq!(chrome.inline_tooltip_timeout(start), Some(TOOLTIP_DELAY));
        assert!(chrome.take_inline_tooltip_due(start + TOOLTIP_DELAY));
        assert_eq!(chrome.inline_tooltip_timeout(start + TOOLTIP_DELAY), None);
    }

    #[test]
    fn focus_cycling_wraps_and_skips_hits_without_an_id() {
        let mut chrome = ToolbarChrome::new(true, (0.0, 0.0));
        chrome.set_inline_rendered(
            vec![
                hit(Some("one"), 0.0, false),
                hit(None, 30.0, false),
                hit(Some("two"), 60.0, false),
            ],
            (0.0, 0.0, 100.0, 20.0),
        );

        assert!(chrome.inline_focus_next(false));
        assert_eq!(chrome.inline_focus_hover(), Some((10.0, 10.0)));
        assert!(chrome.inline_focus_next(false));
        assert_eq!(chrome.inline_focus_hover(), Some((70.0, 10.0)));
        assert!(chrome.inline_focus_next(false));
        assert_eq!(chrome.inline_focus_hover(), Some((10.0, 10.0)));
        assert!(chrome.inline_focus_next(true));
        assert_eq!(chrome.inline_focus_hover(), Some((70.0, 10.0)));
    }
}
