use anyhow::Result;
use log::info;

use super::WaylandBackend;
use super::event_loop::run_event_loop;
use super::setup::setup_wayland;
use super::state_init::init_state;
use super::surface::create_overlay_surface;

pub(super) fn run_backend(backend: &mut WaylandBackend) -> Result<()> {
    info!("Starting Wayland backend");

    let setup = setup_wayland()?;
    let mut runtime = init_state(backend, setup)?;

    create_overlay_surface(&mut runtime.state, &runtime.qh)?;
    runtime.state.refresh_active_output_label();

    let outcome = run_event_loop(
        &runtime.conn,
        &mut runtime.event_queue,
        &runtime.qh,
        &mut runtime.state,
    );

    match outcome.loop_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
