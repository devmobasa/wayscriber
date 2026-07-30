use super::*;
use crate::backend::wayland::toolbar::{ToolbarCursorHint, ToolbarFocusTarget};

mod drag;
mod focus;
mod input;
mod render;

impl WaylandState {
    pub(in crate::backend::wayland) fn mark_inline_toolbar_full_damage(&mut self) {
        self.input_state
            .dirty_tracker
            .mark_full_for(crate::draw::DirtyFullReason::InlineToolbar);
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland) fn inline_toolbar_tooltip_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        min_inline_tooltip_timeout(
            inline_tooltip_timeout(
                self.data.inline_top_tooltip_pending,
                self.data.inline_top_hover_start,
                now,
            ),
            inline_tooltip_timeout(
                self.data.inline_side_tooltip_pending,
                self.data.inline_side_hover_start,
                now,
            ),
        )
    }

    pub(in crate::backend::wayland) fn update_inline_toolbar_tooltip(&mut self, now: Instant) {
        let top_due = inline_tooltip_due(
            self.data.inline_top_tooltip_pending,
            self.data.inline_top_hover_start,
            now,
        );
        let side_due = inline_tooltip_due(
            self.data.inline_side_tooltip_pending,
            self.data.inline_side_hover_start,
            now,
        );
        if top_due {
            self.data.inline_top_tooltip_pending = false;
        }
        if side_due {
            self.data.inline_side_tooltip_pending = false;
        }
        if top_due || side_due {
            self.mark_inline_toolbar_full_damage();
        }
    }

    pub(super) fn clear_inline_toolbar_hits(&mut self) {
        self.data.inline_top_hits.clear();
        self.data.inline_side_hits.clear();
        self.data.inline_top_rect = None;
        self.data.inline_side_rect = None;
    }

    pub(super) fn clear_inline_toolbar_hover(&mut self) {
        self.data.inline_top_hover = None;
        self.data.inline_side_hover = None;
        self.data.inline_top_tooltip_pending = false;
        self.data.inline_side_tooltip_pending = false;
    }

    pub(super) fn clear_inline_toolbar_focus(&mut self) {
        self.data.inline_top_focus_index = None;
        self.data.inline_side_focus_index = None;
        self.data.inline_top_focus_id = None;
        self.data.inline_side_focus_id = None;
    }

    fn inline_focus_index(&self, target: ToolbarFocusTarget) -> Option<usize> {
        match target {
            ToolbarFocusTarget::Top => self.data.inline_top_focus_index,
            ToolbarFocusTarget::Side => self.data.inline_side_focus_index,
        }
    }

    fn inline_focus_index_mut(&mut self, target: ToolbarFocusTarget) -> &mut Option<usize> {
        match target {
            ToolbarFocusTarget::Top => &mut self.data.inline_top_focus_index,
            ToolbarFocusTarget::Side => &mut self.data.inline_side_focus_index,
        }
    }

    fn inline_focus_id(&self, target: ToolbarFocusTarget) -> Option<&str> {
        match target {
            ToolbarFocusTarget::Top => self.data.inline_top_focus_id.as_deref(),
            ToolbarFocusTarget::Side => self.data.inline_side_focus_id.as_deref(),
        }
    }

    fn set_inline_focus_id(&mut self, target: ToolbarFocusTarget, id: Option<String>) {
        match target {
            ToolbarFocusTarget::Top => self.data.inline_top_focus_id = id,
            ToolbarFocusTarget::Side => self.data.inline_side_focus_id = id,
        }
    }

    /// Get cursor hint for inline toolbar hover position.
    pub(in crate::backend::wayland) fn inline_toolbar_cursor_hint(
        &self,
    ) -> Option<ToolbarCursorHint> {
        // Check top toolbar hover
        if let Some((hx, hy)) = self.data.inline_top_hover {
            for hit in &self.data.inline_top_hits {
                if hit.contains(hx, hy) {
                    return Some(hit.kind.cursor_hint());
                }
            }
            return Some(ToolbarCursorHint::Default);
        }
        // Check side toolbar hover
        if let Some((hx, hy)) = self.data.inline_side_hover {
            for hit in &self.data.inline_side_hits {
                if hit.contains(hx, hy) {
                    return Some(hit.kind.cursor_hint());
                }
            }
            return Some(ToolbarCursorHint::Default);
        }
        None
    }
}

fn inline_tooltip_timeout(
    pending: bool,
    hover_start: Option<Instant>,
    now: Instant,
) -> Option<Duration> {
    if !pending {
        return None;
    }
    hover_start.map(|start| {
        start
            .checked_add(crate::backend::wayland::toolbar::render::TOOLTIP_DELAY)
            .unwrap_or(start)
            .saturating_duration_since(now)
    })
}

fn inline_tooltip_due(pending: bool, hover_start: Option<Instant>, now: Instant) -> bool {
    inline_tooltip_timeout(pending, hover_start, now) == Some(Duration::ZERO)
}

fn min_inline_tooltip_timeout(top: Option<Duration>, side: Option<Duration>) -> Option<Duration> {
    match (top, side) {
        (Some(top), Some(side)) => Some(top.min(side)),
        (Some(timeout), None) | (None, Some(timeout)) => Some(timeout),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{inline_tooltip_due, inline_tooltip_timeout, min_inline_tooltip_timeout};
    use std::time::{Duration, Instant};

    #[test]
    fn inline_tooltip_timeout_reaches_zero_at_the_deadline() {
        let start = Instant::now();
        assert_eq!(
            inline_tooltip_timeout(true, Some(start), start),
            Some(crate::backend::wayland::toolbar::render::TOOLTIP_DELAY)
        );
        let due = start + crate::backend::wayland::toolbar::render::TOOLTIP_DELAY;
        assert!(inline_tooltip_due(true, Some(start), due));
        assert_eq!(inline_tooltip_timeout(false, Some(start), due), None);
        assert_eq!(inline_tooltip_timeout(true, None, due), None);
    }

    #[test]
    fn inline_tooltip_timeout_uses_the_earliest_pending_toolbar() {
        assert_eq!(
            min_inline_tooltip_timeout(
                Some(Duration::from_millis(200)),
                Some(Duration::from_millis(50)),
            ),
            Some(Duration::from_millis(50))
        );
    }
}
