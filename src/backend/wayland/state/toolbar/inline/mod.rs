use super::*;
use crate::backend::wayland::toolbar::ToolbarCursorHint;

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
        inline_tooltip_timeout(
            self.data.inline_top_tooltip_pending,
            self.data.inline_top_hover_start,
            now,
        )
    }

    pub(in crate::backend::wayland) fn update_inline_toolbar_tooltip(&mut self, now: Instant) {
        let due = inline_tooltip_due(
            self.data.inline_top_tooltip_pending,
            self.data.inline_top_hover_start,
            now,
        );
        if due {
            self.data.inline_top_tooltip_pending = false;
            self.mark_inline_toolbar_full_damage();
        }
    }

    pub(super) fn clear_inline_toolbar_hits(&mut self) {
        self.data.inline_top_hits.clear();
        self.data.inline_top_rect = None;
    }

    pub(super) fn clear_inline_toolbar_hover(&mut self) {
        self.data.inline_top_hover = None;
        self.data.inline_top_tooltip_pending = false;
    }

    pub(super) fn clear_inline_toolbar_focus(&mut self) {
        self.data.inline_top_focus_index = None;
        self.data.inline_top_focus_id = None;
    }

    /// Get cursor hint for inline toolbar hover position.
    pub(in crate::backend::wayland) fn inline_toolbar_cursor_hint(
        &self,
    ) -> Option<ToolbarCursorHint> {
        let (hx, hy) = self.data.inline_top_hover?;
        for hit in &self.data.inline_top_hits {
            if hit.contains(hx, hy) {
                return Some(hit.kind.cursor_hint());
            }
        }
        Some(ToolbarCursorHint::Default)
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

#[cfg(test)]
mod tests {
    use super::{inline_tooltip_due, inline_tooltip_timeout};
    use std::time::Instant;

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
}
