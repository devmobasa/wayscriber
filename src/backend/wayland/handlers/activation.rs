// Handles xdg-activation events so GNOME will focus the fallback xdg overlay window.
use log::debug;
use smithay_client_toolkit::{
    activation::{ActivationHandler, RequestData},
    globals::GlobalData,
};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::activation::v1::client::{xdg_activation_token_v1, xdg_activation_v1};

use super::super::state::WaylandState;

impl Dispatch<xdg_activation_v1::XdgActivationV1, GlobalData> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &xdg_activation_v1::XdgActivationV1,
        _event: <xdg_activation_v1::XdgActivationV1 as wayland_client::Proxy>::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // xdg_activation_v1 has no events.
        debug!("xdg_activation_v1 event received (none expected)");
    }
}

impl Dispatch<xdg_activation_token_v1::XdgActivationTokenV1, RequestData> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &xdg_activation_token_v1::XdgActivationTokenV1,
        event: <xdg_activation_token_v1::XdgActivationTokenV1 as wayland_client::Proxy>::Event,
        data: &RequestData,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_activation_token_v1::Event::Done { token } = event {
            ActivationHandler::new_token(state, token, data);
        }
    }
}
