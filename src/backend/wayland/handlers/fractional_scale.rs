use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{
        wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        wp_fractional_scale_v1::{self, WpFractionalScaleV1},
    },
    viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
};

use crate::backend::wayland::state::{FullDamageReason, WaylandState};

impl Dispatch<WpFractionalScaleManagerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WpFractionalScaleManagerV1,
        _: <WpFractionalScaleManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        unreachable!("fractional scale manager has no events")
    }
}

impl Dispatch<WpViewporter, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WpViewporter,
        _: <WpViewporter as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        unreachable!("viewporter has no events")
    }
}

impl Dispatch<WpViewport, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WpViewport,
        _: <WpViewport as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        unreachable!("viewport has no events")
    }
}

impl Dispatch<WpFractionalScaleV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        source: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let wp_fractional_scale_v1::Event::PreferredScale { scale: preferred } = event else {
            return;
        };
        match state.toolbar.set_preferred_scale(source, preferred) {
            Ok(true) => {
                state.input_state.needs_redraw = true;
                return;
            }
            Ok(false) => {}
            Err(err) => {
                log::warn!("Rejected invalid toolbar scale: {err:#}");
                return;
            }
        }
        if !state.surface.is_fractional_scale(source) {
            return;
        }
        match state
            .surface
            .set_preferred_scale(preferred, state.config.performance.buffer_count as usize)
        {
            Ok(true) => {}
            Ok(false) => return,
            Err(err) => {
                log::warn!("Rejected invalid preferred surface scale: {err:#}");
                return;
            }
        }

        state
            .buffer_damage
            .mark_all_full(FullDamageReason::ScaleChanged);
        state.render.canvas_layer_cache_mut().clear();
        state.refresh_freeze_zoom_geometry();
        let (width, height) = state.surface.physical_dimensions();
        state
            .frozen
            .handle_resize(width, height, &mut state.input_state);
        state
            .zoom
            .handle_resize(width, height, &mut state.input_state);
        state.cancel_screen_modals_if_source_changed();
        state.input_state.needs_redraw = true;
    }
}
