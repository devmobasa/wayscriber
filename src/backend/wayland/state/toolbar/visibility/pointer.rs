use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn pointer_lock_active(&self) -> bool {
        self.locked_pointer.is_some()
    }

    #[allow(dead_code)] // Kept for potential future pointer lock support
    pub(in crate::backend::wayland) fn lock_pointer_for_drag(
        &mut self,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
    ) {
        if self.inline_toolbars_active() || self.pointer_lock_active() {
            log::info!(
                "skip pointer lock: inline_active={}, already_locked={}",
                self.inline_toolbars_active(),
                self.pointer_lock_active()
            );
            return;
        }
        if self.pointer_constraints_state.bound_global().is_err() {
            log::info!("pointer lock unavailable: constraints global missing");
            return;
        }
        let Some(pointer) = self.current_pointer() else {
            log::info!("pointer lock unavailable: no current pointer");
            return;
        };

        match self.pointer_constraints_state.lock_pointer(
            surface,
            &pointer,
            None,
            zwp_pointer_constraints_v1::Lifetime::Oneshot,
            qh,
        ) {
            Ok(lp) => {
                self.locked_pointer = Some(lp);
                log::info!(
                    "pointer lock requested: seat={:?}, surface={}, pointer_id={}",
                    self.current_seat_id(),
                    surface_id(surface),
                    pointer.id().protocol_id()
                );
            }
            Err(err) => {
                warn!("Failed to lock pointer for toolbar drag: {}", err);
                return;
            }
        }

        match self
            .relative_pointer_state
            .get_relative_pointer(&pointer, qh)
        {
            Ok(rp) => {
                self.relative_pointer = Some(rp);
                log::info!("relative pointer bound for drag");
            }
            Err(err) => {
                warn!("Failed to obtain relative pointer for drag: {}", err);
                // Abort lock if relative pointer is unavailable; fall back to absolute path.
                self.unlock_pointer();
            }
        }
    }

    pub(in crate::backend::wayland) fn unlock_pointer(&mut self) {
        if let Some(lp) = self.locked_pointer.take() {
            lp.destroy();
        }
        if let Some(rp) = self.relative_pointer.take() {
            rp.destroy();
        }
    }
}
