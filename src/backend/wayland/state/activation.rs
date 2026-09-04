use log::info;

use crate::app_id::runtime_app_id;

use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn activate_xdg_window_with_startup_token_if_present(
        &mut self,
    ) -> bool {
        if !self.surface.is_xdg_window() {
            return false;
        }

        let Some(token) = self.focus.take_startup_activation_token() else {
            return false;
        };
        let Some(activation) = self.protocol.activation() else {
            return false;
        };
        let Some(wl_surface) = self.surface.wl_surface().cloned() else {
            return false;
        };

        info!("Applying startup activation token for xdg fallback window");
        activation.activate::<WaylandState>(&wl_surface, token);
        true
    }

    pub(in crate::backend::wayland) fn request_xdg_activation(&mut self, qh: &QueueHandle<Self>) {
        if !self.surface.is_xdg_window() {
            return;
        }

        let Some(activation) = self.protocol.activation() else {
            return;
        };

        let Some(wl_surface) = self.surface.wl_surface().cloned() else {
            return;
        };

        if let Some(seat_serial) = self
            .focus
            .current_seat()
            .zip(self.focus.last_activation_serial())
        {
            let app_id = runtime_app_id();
            activation.request_token::<Self>(
                qh,
                RequestData {
                    app_id: Some(app_id),
                    seat_and_serial: Some(seat_serial),
                    surface: Some(wl_surface),
                },
            );
        } else {
            // Defer until we have a keyboard enter serial.
            self.focus.defer_activation_until_serial();
        }
    }

    fn activate_xdg_window_if_possible(&mut self) {
        if !self.surface.is_xdg_window() {
            return;
        }

        let Some(token) = self.focus.activation_token_to_apply() else {
            return;
        };

        let Some(activation) = self.protocol.activation() else {
            return;
        };

        let Some(wl_surface) = self.surface.wl_surface().cloned() else {
            return;
        };

        activation.activate::<WaylandState>(&wl_surface, token);
        self.focus.clear_pending_activation_token();
    }

    pub(in crate::backend::wayland) fn maybe_retry_activation(&mut self, qh: &QueueHandle<Self>) {
        if self.focus.retry_activation_wanted() {
            // Drop the placeholder and re-request with the new serial.
            self.focus.clear_pending_activation_token();
            self.request_xdg_activation(qh);
        }
    }
}

impl ActivationHandler for WaylandState {
    type RequestData = RequestData;

    fn new_token(&mut self, token: String, _data: &Self::RequestData) {
        self.focus.note_activation_token(token);
        self.activate_xdg_window_if_possible();
    }
}
