//! Presses taken by the inline strip, toolbar surfaces, and toolbar menus.
use super::*;

impl WaylandState {
    /// The inline strip, a toolbar surface, and open toolbar menus take a press
    /// before the canvas. Returns true when one of them did.
    pub(super) fn route_toolbar_pointer_press(
        &mut self,
        conn: &wayland_client::Connection,
        qh: &QueueHandle<Self>,
        event: &PointerEvent,
        button: u32,
        on_toolbar: bool,
        inline_active: bool,
    ) -> bool {
        if debug_toolbar_drag_logging_enabled() {
            debug!(
                "pointer press: button={}, on_toolbar={}, inline_active={}, drag_active={}",
                button,
                on_toolbar,
                inline_active,
                self.toolbar_drag.is_moving()
            );
        }
        if inline_active {
            self.inline_toolbar_motion(event.position);
        }
        if inline_active && self.handle_inline_pointer_press(conn, qh, event, button) {
            self.pointer
                .bind_contact(button, ContactOwner::InlineToolbar);
            return true;
        }
        if on_toolbar {
            self.handle_toolbar_pointer_press(conn, qh, event, button);
            return true;
        }
        if self.toolbar_chrome.pointer_over_toolbar() {
            self.finish_toolbar_item_drag(false);
            self.toolbar_drag.set_item_dragging(false);
            return true;
        }

        button == BTN_LEFT && self.dismiss_top_toolbar_menus()
    }

    fn handle_inline_pointer_press(
        &mut self,
        conn: &wayland_client::Connection,
        qh: &QueueHandle<Self>,
        event: &PointerEvent,
        button: u32,
    ) -> bool {
        if button == BTN_RIGHT
            && self.inline_toolbar_secondary_press(event.position, Some(conn), Some(qh))
        {
            self.refresh_keyboard_interactivity();
            return true;
        }
        if button == BTN_LEFT && self.inline_toolbar_press(event.position, Some(conn), Some(qh)) {
            drag_log(|| {
                format!(
                    "pointer press: inline handled, drag_active={}, pos=({:.3}, {:.3}), surface={}",
                    self.toolbar_drag.item_dragging(),
                    event.position.0,
                    event.position.1,
                    surface_id(&event.surface)
                )
            });
            if self.toolbar_drag.is_moving() {
                self.lock_pointer_for_drag(qh, &event.surface);
            }
            return true;
        }
        if !self.toolbar_chrome.pointer_over_toolbar() {
            return false;
        }
        if button == BTN_LEFT {
            self.dismiss_top_toolbar_menus();
        }
        true
    }

    fn handle_toolbar_pointer_press(
        &mut self,
        conn: &wayland_client::Connection,
        qh: &QueueHandle<Self>,
        event: &PointerEvent,
        button: u32,
    ) {
        if button == BTN_RIGHT
            && let Some(index) = self
                .toolbar
                .quick_color_slot_at(&event.surface, event.position)
        {
            self.handle_toolbar_event(ToolbarEvent::EditQuickColor { index }, Some(conn), Some(qh));
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            self.refresh_keyboard_interactivity();
            return;
        }
        let handled = if button == BTN_LEFT {
            self.handle_primary_toolbar_pointer_press(conn, qh, event)
        } else {
            false
        };
        if button == BTN_LEFT && !handled {
            self.dismiss_top_toolbar_menus();
        }
    }

    fn handle_primary_toolbar_pointer_press(
        &mut self,
        conn: &wayland_client::Connection,
        qh: &QueueHandle<Self>,
        event: &PointerEvent,
    ) -> bool {
        let Some((intent, drag)) = self.toolbar.pointer_press(&event.surface, event.position)
        else {
            return false;
        };
        let toolbar_event = intent_to_event(intent, self.toolbar.last_snapshot());
        if matches!(toolbar_event, ToolbarEvent::MoveTopToolbar { .. }) && drag {
            self.lock_pointer_for_drag(qh, &event.surface);
        }
        log::info!(
            "toolbar press: drag_start={}, surface={}, seat={:?}, inline_active={}",
            drag,
            surface_id(&event.surface),
            self.focus.current_seat_id(),
            self.toolbar_chrome.inline_toolbars()
        );
        self.toolbar_drag.set_item_dragging(drag);
        self.handle_toolbar_event(toolbar_event, Some(conn), Some(qh));
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
        self.refresh_keyboard_interactivity();
        true
    }

    /// Click-away dismissal for the top-strip menus/popovers. Defers to the
    /// canonical [`InputState::close_top_toolbar_menus`] so the click-away set
    /// stays in lockstep with the keyboard Escape route and the apply-action
    /// callers — the Canvas popover in particular must dismiss here exactly
    /// like the Session/Settings popovers, else a canvas click would leak
    /// through and start a stray stroke. Returns whether a menu was open so the
    /// press handler early-returns instead of drawing.
    ///
    /// Shared with the touch-down and tablet pen-down paths so every canvas
    /// down modality dismisses the Canvas (and Session/Settings) popover and
    /// swallows the interaction identically.
    pub(in crate::backend::wayland) fn dismiss_top_toolbar_menus(&mut self) -> bool {
        let changed = self.input_state.close_top_toolbar_menus();
        if changed {
            if self.toolbar_chrome.inline_toolbars() {
                self.mark_inline_toolbar_full_damage();
            } else {
                self.toolbar.mark_dirty();
            }
        }
        changed
    }
}
