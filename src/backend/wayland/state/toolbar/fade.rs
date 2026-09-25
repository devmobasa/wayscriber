//! Backend wiring for the top-strip idle hide/reveal.
//!
//! The renderer-neutral policy lives in `ui::toolbar::snapshot::fade`; this
//! module feeds it the backend-only signals once per event-loop pass: the
//! pointer over the toolbar surfaces or inside the reveal zone around the
//! strip, hover on the GTK strip, open menus, and keyboard tool/color changes
//! (a brief reveal). It also exposes the wakeup deadline the loop needs so a
//! pending hide or an in-flight transition keeps ticking, and stops ticking
//! once settled.

use std::time::{Duration, Instant};

use super::*;
use crate::draw::Color;
use crate::input::Tool;
use crate::ui::toolbar::snapshot::fade::TopStripFadeInputs;

/// What a keyboard shortcut can change that the strip displays: the
/// explicitly selected tool and its color. A modifier-held drag tool is not
/// part of it, so holding Ctrl for Ctrl+Z does not flash the strip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct StripRevealKey {
    tool: Option<Tool>,
    color: Color,
}

impl StripRevealKey {
    fn of(input: &crate::input::state::InputState) -> Self {
        let tool = input.tool_override();
        Self {
            tool,
            color: input.color_for_tool(tool.unwrap_or(Tool::Pen)),
        }
    }
}

impl WaylandState {
    /// Advance the fade engine one step. Called once per event-loop pass,
    /// before the snapshot consumers (`render_layer_toolbars_if_needed`,
    /// `push_gtk_toolbar_update`) read `top_fade`.
    pub(in crate::backend::wayland) fn update_top_strip_fade(&mut self, now: Instant) {
        if self
            .toolbar_chrome
            .note_reveal_key(StripRevealKey::of(&self.input_state))
        {
            self.toolbar_chrome.fade_mut().reveal_briefly(now);
        }

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
    /// the remaining time before the hide starts. `None` when settled.
    pub(in crate::backend::wayland) fn top_strip_fade_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        self.toolbar_chrome
            .fade()
            .wake_after(&self.top_strip_fade_inputs(now), now)
    }

    fn top_strip_fade_inputs(&self, now: Instant) -> TopStripFadeInputs {
        let input = &self.input_state;
        // Minimal chrome never fades: the restore tab and micro chip are
        // already the quiet form, and a hidden strip has nothing to fade.
        let reduced_chrome = !input.toolbar_top_visible()
            || input.toolbar_top_minimized()
            || input.toolbar_top_display_mode() == crate::config::TopDisplayMode::Micro;
        let menus_open = top_menus_open(input);
        let idle_fade_enabled = input.ui_visibility.idle_fade;
        let on_strip = self.toolbar_chrome.strip_engaged() || self.toolbar.top_pointer_present();
        // The reveal zone costs a layout pass, so only measure it when
        // nothing else already decides the outcome.
        let pointer_near = on_strip
            || (idle_fade_enabled
                && !menus_open
                && !reduced_chrome
                && self.pointer_in_top_strip_reveal_zone());

        TopStripFadeInputs {
            idle_for: now.saturating_duration_since(input.last_draw_activity()),
            pointer_near,
            menus_open,
            reduced_chrome,
            idle_fade_enabled,
        }
    }

    fn pointer_in_top_strip_reveal_zone(&self) -> bool {
        let Some(point) = self.canvas_hover_point() else {
            return false;
        };
        let Some(strip) = self.top_strip_screen_rect() else {
            return false;
        };
        geometry::point_in_top_strip_reveal_zone(point, strip)
    }

    /// Where the pointer or a hovering stylus sits on the canvas surface, if
    /// either does. The toolbar surfaces report their own hover.
    fn canvas_hover_point(&self) -> Option<(f64, f64)> {
        #[cfg(feature = "tablet-input")]
        if self.tablet.on_overlay
            && !self.tablet.on_toolbar
            && let Some(point) = self.tablet.last_pos
        {
            return Some(point);
        }
        if !self.focus.pointer_focused() || self.toolbar_chrome.pointer_over_toolbar() {
            return None;
        }
        let (x, y) = self.pointer.position();
        Some((x as f64, y as f64))
    }

    /// The top strip's bounds in canvas coordinates. Layer-shell and GTK
    /// strips sit at the pushed base plus the drag offset with the shared
    /// natural size; the inline strip reports the rect it last painted.
    fn top_strip_screen_rect(&self) -> Option<(f64, f64, f64, f64)> {
        if self.inline_toolbars_render_active() {
            return self.toolbar_chrome.inline_rect();
        }
        let snapshot = self.toolbar_snapshot();
        let (width, height) = top_size(self.render.ui_text(), &snapshot);
        let offset = self.toolbar_chrome.top_offset();
        Some((
            self.inline_top_base_x() + offset.0,
            self.inline_top_base_y() + offset.1,
            width as f64,
            height as f64,
        ))
    }
}

/// True while any top-strip-anchored menu or popover is open. Open menus
/// hold the idle fade: the strip (and the popover hosted on its surface)
/// must stay visible while one is up, even with the pointer away.
fn top_menus_open(input: &crate::input::state::InputState) -> bool {
    input.toolbar_top_menu().is_open() || input.is_color_picker_popup_open()
}

#[cfg(test)]
mod tests {
    use super::{StripRevealKey, top_menus_open};
    use crate::input::Tool;
    use crate::input::state::{TopMenuState, test_support::make_test_input_state};

    /// Every top-strip menu — including the Canvas popover and the
    /// Session/Settings popovers the overflow anchors — holds the idle fade
    /// while open, so the strip (and the popover hosted on its surface) never
    /// hides out from under an open menu.
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

    /// A tool shortcut or a color change alters the reveal key, so the
    /// strip flashes; a modifier held for a shortcut (Ctrl for Ctrl+Z) does
    /// not.
    #[test]
    fn reveal_key_tracks_the_selected_tool_and_color_but_not_modifiers() {
        let mut input = make_test_input_state();
        let initial = StripRevealKey::of(&input);

        input.modifiers.ctrl = true;
        assert_eq!(StripRevealKey::of(&input), initial, "held modifier");
        input.modifiers.ctrl = false;

        assert!(input.set_tool_override(Some(Tool::Marker)));
        let marker = StripRevealKey::of(&input);
        assert_ne!(marker, initial, "tool shortcut");

        let color = crate::draw::Color {
            r: 0.1,
            g: 0.8,
            b: 0.3,
            a: 1.0,
        };
        assert!(input.set_color(color));
        assert_ne!(StripRevealKey::of(&input), marker, "color shortcut");
    }
}
