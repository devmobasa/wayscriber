use super::super::*;
use smithay_client_toolkit::shell::WaylandSurface;
use std::time::Instant;

impl WaylandState {
    pub(in crate::backend::wayland) fn has_cursor_focus(&self) -> bool {
        self.focus.pointer_focused() || self.stylus_hover_cursor_visible()
    }

    pub(in crate::backend::wayland) fn cursor_blocked_by_toolbar(&self) -> bool {
        self.stylus_hover_cursor_position().is_none() && self.toolbar_chrome.pointer_over_toolbar()
    }

    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn stylus_hover_cursor_visible(&self) -> bool {
        self.tablet.hover_cursor_position().is_some()
    }

    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn stylus_hover_cursor_position(&self) -> Option<(f64, f64)> {
        self.tablet.hover_cursor_position()
    }

    /// Retire in-flight stylus input without dispatching a canvas release.
    ///
    /// A screen-region modal cancels the gesture the pen was making, and then
    /// consumes the tip-up the compositor still owes us — so nothing else
    /// retires the contact. Left set, `tablet.tip_down` hides the hover cursor
    /// and defers session saving until some unrelated release or proximity-out
    /// happens to clear it. The stroke was cancelled rather than finished, so
    /// its peak pressure is dropped instead of being committed to the tool.
    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn retire_stylus_contact(&mut self) {
        // Tablet events are coalesced until their frame commits, so input that
        // physically happened before the modal opened can still be sitting in
        // the buffer. Replaying it afterwards would start a region from a press
        // that predates the selector, or apply that batch's pressure and barrel
        // actions behind it. Dropping the batch is unconditional: an uncommitted
        // tip-down is exactly the case where no contact is logically held yet.
        // Clearing our own flags does not lift the pen. The compositor keeps
        // reporting this contact — pressure included — until the tip rises.
        let transition = self.tablet.retire_contact();
        self.mark_stylus_hover_cursor_dirty(transition.previous, transition.next);
    }

    #[cfg(not(feature = "tablet-input"))]
    pub(in crate::backend::wayland) fn stylus_hover_cursor_visible(&self) -> bool {
        false
    }

    #[cfg(not(feature = "tablet-input"))]
    pub(in crate::backend::wayland) fn stylus_hover_cursor_position(&self) -> Option<(f64, f64)> {
        None
    }

    /// Consume the disowned-contact latch, reporting whether the tip that just
    /// lifted belonged to a contact a screen modal had taken over.
    ///
    /// A disowned tip-up dispatches no canvas release and commits no thickness;
    /// the next press after it is an ordinary fresh contact.
    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn take_retired_stylus_contact(&mut self) -> bool {
        self.tablet.take_retired_contact()
    }

    #[cfg(not(feature = "tablet-input"))]
    pub(in crate::backend::wayland) fn retire_stylus_contact(&mut self) {}

    pub(in crate::backend::wayland) fn begin_xdg_frozen_fullscreen(&mut self) -> bool {
        let Some(window) = self.surface.xdg_window().cloned() else {
            return false;
        };
        self.surface
            .placement_mut()
            .xdg_frozen_mut()
            .request(Instant::now());
        if let Some(output) = self.preferred_fullscreen_output() {
            window.set_fullscreen(Some(&output));
        } else {
            window.set_fullscreen(None);
        }
        window.commit();
        true
    }

    pub(in crate::backend::wayland) fn restore_xdg_after_frozen(&mut self) {
        if !self.surface.placement().xdg_frozen().requested() {
            return;
        }
        if let Some(window) = self.surface.xdg_window().cloned() {
            if self.surface.placement().xdg_fullscreen() {
                if let Some(output) = self.preferred_fullscreen_output() {
                    window.set_fullscreen(Some(&output));
                } else {
                    window.set_fullscreen(None);
                }
            } else {
                window.unset_fullscreen();
                window.set_maximized();
            }
            window.commit();
        }
        self.surface.placement_mut().xdg_frozen_mut().finish();
    }

    pub(in crate::backend::wayland) fn activate_pending_frozen_image_for_current_surface(
        &mut self,
    ) {
        let was_xdg_frozen_fullscreen = self.surface.placement().xdg_frozen().requested();
        let (phys_width, phys_height) = self.surface.physical_dimensions();
        let live_output_count = self.live_output_count();
        match self.frozen.activate_pending_image_with_live_outputs(
            phys_width,
            phys_height,
            &mut self.input_state,
            live_output_count,
        ) {
            Ok(true) => {
                if was_xdg_frozen_fullscreen {
                    self.surface.placement_mut().xdg_frozen_mut().activate();
                }
            }
            Ok(false) => {}
            Err(err) => {
                log::warn!("Frozen pending image activation failed: {}", err);
                self.restore_xdg_after_frozen();
            }
        }
    }

    pub(in crate::backend::wayland) fn xdg_focus_loss_exits_overlay(&self) -> bool {
        matches!(
            self.config.ui.xdg_focus_loss_behavior,
            crate::config::XdgFocusLossBehavior::Exit
        )
    }

    pub(in crate::backend::wayland) fn session_options(&self) -> Option<&SessionOptions> {
        self.session.options()
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn session_options_mut(
        &mut self,
    ) -> Option<&mut SessionOptions> {
        self.session.options_mut()
    }
}
