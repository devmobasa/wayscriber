use super::*;
use wayland_client::Proxy;

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
        drag_log(format!(
            "lock_pointer_for_drag: inline_active={}, locked={}, surface={}",
            self.inline_toolbars_active(),
            self.pointer_lock_active(),
            surface_id(surface)
        ));
        if !toolbar_pointer_lock_enabled() {
            log::info!("skip pointer lock: disabled via WAYSCRIBER_TOOLBAR_POINTER_LOCK");
            return;
        }
        if self.pointer_lock_active() {
            log::info!("skip pointer lock: already_locked");
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
                drag_log(format!(
                    "pointer lock requested: seat={:?}, surface={}, pointer_id={}",
                    self.current_seat_id(),
                    surface_id(surface),
                    pointer.id().protocol_id()
                ));
            }
            Err(err) => {
                warn!("Failed to lock pointer for toolbar drag: {}", err);
                return;
            }
        }

        // Hide the cursor while dragging with pointer lock to avoid visual jitter.
        if pointer
            .data::<PointerData>()
            .and_then(|data| data.latest_button_serial().or(data.latest_enter_serial()))
            .is_some()
        {
            self.hide_pointer_cursor();
        }

        match self
            .relative_pointer_state
            .get_relative_pointer(&pointer, qh)
        {
            Ok(rp) => {
                self.relative_pointer = Some(rp);
                drag_log("relative pointer bound for drag");
            }
            Err(err) => {
                warn!("Failed to obtain relative pointer for drag: {}", err);
                // Abort lock if relative pointer is unavailable; fall back to absolute path.
                self.unlock_pointer();
            }
        }
    }

    pub(in crate::backend::wayland) fn unlock_pointer(&mut self) {
        if self.locked_pointer.is_some() || self.relative_pointer.is_some() {
            drag_log(format!(
                "unlock pointer: locked={}, relative={}",
                self.locked_pointer.is_some(),
                self.relative_pointer.is_some()
            ));
        }
        if let Some(lp) = self.locked_pointer.take() {
            lp.destroy();
        }
        if let Some(rp) = self.relative_pointer.take() {
            rp.destroy();
        }
    }
}
