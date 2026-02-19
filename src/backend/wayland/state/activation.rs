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

        let Some(token) = self.take_startup_activation_token() else {
            return false;
        };
        let Some(activation) = self.activation.as_ref() else {
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

        let Some(activation) = self.activation.as_ref() else {
            return;
        };

        let Some(wl_surface) = self.surface.wl_surface().cloned() else {
            return;
        };

        if let Some(seat_serial) = self
            .current_seat()
            .as_ref()
            .cloned()
            .zip(self.last_activation_serial())
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
            self.set_pending_activation_token(Some(String::new())); // marker
        }
    }

    fn activate_xdg_window_if_possible(&mut self) {
        if !self.surface.is_xdg_window() {
            return;
        }

        let Some(token) = self.pending_activation_token() else {
            return;
        };

        let Some(activation) = self.activation.as_ref() else {
            return;
        };

        let Some(wl_surface) = self.surface.wl_surface().cloned() else {
            return;
        };

        activation.activate::<WaylandState>(&wl_surface, token);
        self.set_pending_activation_token(None);
    }

    pub(in crate::backend::wayland) fn maybe_retry_activation(&mut self, qh: &QueueHandle<Self>) {
        if self.pending_activation_token().is_some() && self.last_activation_serial().is_some() {
            // Drop the placeholder and re-request with the new serial.
            self.set_pending_activation_token(None);
            self.request_xdg_activation(qh);
        }
    }
}

impl ActivationHandler for WaylandState {
    type RequestData = RequestData;

    fn new_token(&mut self, token: String, _data: &Self::RequestData) {
        self.set_pending_activation_token(Some(token));
        self.activate_xdg_window_if_possible();
    }
}
