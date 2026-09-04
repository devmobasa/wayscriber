use super::*;
use crate::backend::wayland::toolbar::ToolbarCursorHint;

mod drag;
mod focus;
mod input;
mod render;

impl WaylandState {
    /// Repaints just the inline top strip.
    ///
    /// Setting `needs_redraw` without any damage rect looks cheap but is the
    /// opposite: the render pass falls back to `EmptyDamageFallback` and
    /// repaints the whole surface. Damage the strip's own rect instead and
    /// leave the canvas alone.
    pub(in crate::backend::wayland) fn mark_inline_toolbar_rect_damage(&mut self) {
        if let Some((x, y, w, h)) = self.toolbar_chrome.inline_rect()
            && let Some(rect) = crate::util::Rect::new(
                x.floor() as i32 - 1,
                y.floor() as i32 - 1,
                w.ceil() as i32 + 2,
                h.ceil() as i32 + 2,
            )
        {
            self.input_state.dirty_tracker.mark_rect(rect);
        }
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
    }

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
        self.toolbar_chrome.inline_tooltip_timeout(now)
    }

    pub(in crate::backend::wayland) fn update_inline_toolbar_tooltip(&mut self, now: Instant) {
        if self.toolbar_chrome.take_inline_tooltip_due(now) {
            self.mark_inline_toolbar_full_damage();
        }
    }

    /// Get cursor hint for inline toolbar hover position.
    pub(in crate::backend::wayland) fn inline_toolbar_cursor_hint(
        &self,
    ) -> Option<ToolbarCursorHint> {
        self.toolbar_chrome.inline_cursor_hint()
    }
}
