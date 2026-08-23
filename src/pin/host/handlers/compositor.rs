use smithay_client_toolkit::compositor::CompositorHandler;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    protocol::{wl_callback, wl_output, wl_surface},
};

use super::super::PinHost;
use crate::pin::surface::ShellEventIdentity;

impl Dispatch<wl_callback::WlCallback, ShellEventIdentity> for PinHost {
    fn event(
        state: &mut Self,
        _proxy: &wl_callback::WlCallback,
        _event: wl_callback::Event,
        identity: &ShellEventIdentity,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some(pin) = state.pins.get_mut(&identity.pin_id) else {
            return;
        };
        if pin.shell.generation != identity.shell_generation
            || pin.shell.frame_callback != Some(identity.token)
        {
            return;
        }
        pin.shell.frame_callback = None;
        if let Some(origin) = pin.shell.pending_origin.take() {
            pin.shell.committed_origin = origin;
        }
        if pin.dirty {
            // The outer loop retries bounded-buffer rendering after dispatch.
            log::trace!(
                "Pin {} frame callback released render throttle",
                identity.pin_id
            );
        }
    }
}

impl CompositorHandler for PinHost {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        let Some(id) = self.by_wl_surface.get(&surface.id()).copied() else {
            return;
        };
        let scale = new_factor.max(1);
        if self
            .pins
            .get(&id)
            .is_some_and(|pin| pin.shell.scale != scale)
            && let Err(error) = self.change_scale(id, scale)
        {
            log::error!("Pin {id} scale change failed: {error:#}");
            self.close_pin(id);
        }
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        // Layer-shell owns output transform. Source pixels are never rotated twice.
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        let Some(id) = self.by_wl_surface.get(&surface.id()).copied() else {
            return;
        };
        let Some(output) = self.outputs.by_proxy(output) else {
            return;
        };
        if let Err(error) = self.change_scale(id, output.scale.max(1)) {
            log::error!("Pin {id} output-enter scale failed: {error:#}");
            self.close_pin(id);
        }
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}
