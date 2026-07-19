//! Backend wiring for the top-strip idle fade.
//!
//! The renderer-neutral policy lives in `ui::toolbar::snapshot::fade`; this
//! module feeds it the backend-only signals (pointer over the toolbar
//! surfaces, hover on the top strip, open menus) once per event-loop pass
//! and exposes the wakeup deadline the loop needs so a pending dim or an
//! in-flight transition keeps ticking — and stops ticking once settled.

use std::time::{Duration, Instant};

use super::*;
use crate::ui::toolbar::snapshot::fade::TopStripFadeInputs;

impl WaylandState {
    /// Advance the fade engine one step. Called once per event-loop pass,
    /// before the snapshot consumers (`render_layer_toolbars_if_needed`,
    /// `push_gtk_toolbar_update`) read `top_fade`.
    pub(in crate::backend::wayland) fn update_top_strip_fade(&mut self, now: Instant) {
        let inputs = self.top_strip_fade_inputs(now);
        let before = self.data.top_strip_fade.value();
        let after = self.data.top_strip_fade.update(&inputs, now);
        // Layer-shell (and GTK) toolbars repaint from the changed snapshot on
        // their own; inline toolbars live on the canvas surface, so a fade
        // step must damage their rect and request a canvas redraw itself.
        if before != after && self.inline_toolbars_render_active() {
            if let Some((x, y, w, h)) = self.data.inline_top_rect
                && let Some(rect) = crate::util::Rect::new(
                    x.floor() as i32 - 1,
                    y.floor() as i32 - 1,
                    w.ceil() as i32 + 2,
                    h.ceil() as i32 + 2,
                )
            {
                self.input_state.dirty_tracker.mark_rect(rect);
            }
            self.input_state.needs_redraw = true;
        }
    }

    /// Deadline for the event loop: the next fade tick while animating, or
    /// the remaining idle time before the dim starts. `None` when settled.
    pub(in crate::backend::wayland) fn top_strip_fade_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        self.data
            .top_strip_fade
            .wake_after(&self.top_strip_fade_inputs(now))
    }

    fn top_strip_fade_inputs(&self, now: Instant) -> TopStripFadeInputs {
        let input = &self.input_state;
        // Minimal chrome never fades: the restore tab and micro chip are
        // already the quiet form, and a hidden strip has nothing to fade.
        let reduced_chrome = !input.toolbar_top_visible()
            || input.toolbar_top_minimized
            || input.toolbar_top_display_mode == crate::config::TopDisplayMode::Micro;
        let pointer_near = self.pointer_over_toolbar()
            || self.toolbar.top_pointer_present()
            || self.data.inline_top_hover.is_some()
            || self.data.gtk_top_hover;
        let menus_open = input.toolbar_shapes_expanded
            || input.toolbar_top_overflow_open
            || input.is_color_picker_popup_open();
        TopStripFadeInputs {
            idle_for: now.saturating_duration_since(input.last_draw_activity()),
            pointer_near,
            menus_open,
            reduced_chrome,
        }
    }
}
