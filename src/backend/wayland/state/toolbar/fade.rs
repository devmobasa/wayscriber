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
        let before = self.toolbar_chrome.fade().value();
        let after = self.toolbar_chrome.fade_mut().update(&inputs, now);
        // Layer-shell (and GTK) toolbars repaint from the changed snapshot on
        // their own; inline toolbars live on the canvas surface, so a fade
        // step must damage their rect and request a canvas redraw itself.
        if before != after && self.inline_toolbars_render_active() {
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
            self.input_state.needs_redraw = true;
        }
    }

    /// Deadline for the event loop: the next fade tick while animating, or
    /// the remaining idle time before the dim starts. `None` when settled.
    pub(in crate::backend::wayland) fn top_strip_fade_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        self.toolbar_chrome
            .fade()
            .wake_after(&self.top_strip_fade_inputs(now))
    }

    fn top_strip_fade_inputs(&self, now: Instant) -> TopStripFadeInputs {
        let input = &self.input_state;
        // Minimal chrome never fades: the restore tab and micro chip are
        // already the quiet form, and a hidden strip has nothing to fade.
        let reduced_chrome = !input.toolbar_top_visible()
            || input.toolbar_top_minimized()
            || input.toolbar_top_display_mode() == crate::config::TopDisplayMode::Micro;
        self.toolbar_chrome.fade_inputs(
            self.toolbar.top_pointer_present(),
            now.saturating_duration_since(input.last_draw_activity()),
            top_menus_open(input),
            reduced_chrome,
            input.ui_visibility.idle_fade,
        )
    }
}

/// True while any top-strip-anchored menu or popover is open. Open menus
/// hold the idle fade: the strip (and the popover hosted on its surface)
/// must stay full-opacity while one is up, even with the pointer away.
fn top_menus_open(input: &crate::input::state::InputState) -> bool {
    input.toolbar_top_menu().is_open() || input.is_color_picker_popup_open()
}

#[cfg(test)]
mod tests {
    use super::top_menus_open;
    use crate::input::state::{TopMenuState, test_support::make_test_input_state};

    /// Every top-strip menu — including the Canvas popover and the
    /// Session/Settings popovers the overflow anchors — holds the idle fade
    /// while open, so the strip (and the popover hosted on its surface) never
    /// dims out from under an open menu.
    #[test]
    fn every_open_top_menu_holds_the_idle_fade() {
        let mut input = make_test_input_state();
        assert!(!top_menus_open(&input));

        for menu in [
            TopMenuState::ShapePicker,
            TopMenuState::TopOverflow,
            TopMenuState::CanvasPopover,
            TopMenuState::SessionPopover,
            TopMenuState::SettingsPopover,
        ] {
            input.test_set_toolbar_menu_state(menu, input.toolbar_top_popover_scroll());
            assert!(top_menus_open(&input), "{menu:?}");
        }

        input.test_set_toolbar_menu_state(TopMenuState::Closed, input.toolbar_top_popover_scroll());

        assert!(!top_menus_open(&input));
    }
}
