use anyhow::{Context, Result};
use log::{debug, warn};
use smithay_client_toolkit::globals::ProvidesBoundGlobal;
use smithay_client_toolkit::{
    activation::ActivationState,
    compositor::CompositorState,
    output::OutputState,
    registry::RegistryState,
    seat::{
        SeatState, pointer_constraints::PointerConstraintsState,
        relative_pointer::RelativePointerState,
    },
    shell::{wlr_layer::LayerShell, xdg::XdgShell},
    shm::Shm,
};
use wayland_client::{Connection, EventQueue, globals::registry_queue_init};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::env_vars::{XDG_CURRENT_DESKTOP_ENV, XDG_SESSION_DESKTOP_ENV};

use super::super::state::{WaylandGlobals, WaylandState};

// Freeze/zoom capture currently consumes wl_shm buffer events and ignores linux-dmabuf.
// Version 3 can negotiate linux-dmabuf-only frames on newer wlroots/NVIDIA stacks, so
// bind through v2 until dmabuf capture is implemented.
const MAX_SHM_SCREENCOPY_VERSION: u32 = 2;

pub(super) struct WaylandSetup {
    pub(super) conn: Connection,
    #[cfg(feature = "tablet-input")]
    pub(super) globals: wayland_client::globals::GlobalList,
    pub(super) event_queue: EventQueue<WaylandState>,
    pub(super) qh: wayland_client::QueueHandle<WaylandState>,
    pub(super) state_globals: WaylandGlobals,
    pub(super) screencopy_manager: Option<ZwlrScreencopyManagerV1>,
    pub(super) text_input_manager: Option<ZwpTextInputManagerV3>,
    pub(super) layer_shell_available: bool,
}

pub(super) fn setup_wayland() -> Result<WaylandSetup> {
    // Connect to Wayland compositor
    let conn = Connection::connect_to_env().context("Failed to connect to Wayland compositor")?;
    debug!("Connected to Wayland display");

    // Initialize registry and event queue
    let (globals, event_queue) =
        registry_queue_init(&conn).context("Failed to initialize Wayland registry")?;
    let qh = event_queue.handle();

    // Bind global interfaces
    let compositor_state =
        CompositorState::bind(&globals, &qh).context("wl_compositor not available")?;
    debug!("Bound compositor");

    let layer_shell = match LayerShell::bind(&globals, &qh) {
        Ok(shell) => {
            debug!("Bound layer shell");
            Some(shell)
        }
        Err(err) => {
            let desktop_env =
                std::env::var(XDG_CURRENT_DESKTOP_ENV).unwrap_or_else(|_| "unknown".into());
            let session_env =
                std::env::var(XDG_SESSION_DESKTOP_ENV).unwrap_or_else(|_| "unknown".into());
            warn!(
                "Layer shell not available: {} (desktop='{}', session='{}'); toolbars will be disabled and xdg fallback may not cover docks/panels.",
                err, desktop_env, session_env
            );
            None
        }
    };

    let xdg_shell = match XdgShell::bind(&globals, &qh) {
        Ok(shell) => {
            debug!("Bound xdg-shell");
            Some(shell)
        }
        Err(err) => {
            warn!("xdg-shell not available: {}", err);
            None
        }
    };

    let activation = match ActivationState::bind(&globals, &qh) {
        Ok(state) => {
            debug!("Bound xdg-activation");
            Some(state)
        }
        Err(err) => {
            debug!("xdg-activation not available: {}", err);
            None
        }
    };

    if layer_shell.is_none() && xdg_shell.is_none() {
        return Err(anyhow::anyhow!(
            "Wayland compositor does not expose layer-shell or xdg-shell protocols"
        ));
    }

    let shm = Shm::bind(&globals, &qh).context("wl_shm not available")?;
    debug!("Bound shared memory");

    let output_state = OutputState::new(&globals, &qh);
    debug!("Initialized output state");

    let seat_state = SeatState::new(&globals, &qh);
    debug!("Initialized seat state");

    let registry_state = RegistryState::new(&globals);
    let pointer_constraints_state = PointerConstraintsState::bind(&globals, &qh);
    let relative_pointer_state = RelativePointerState::bind(&globals, &qh);
    if pointer_constraints_state.bound_global().is_ok() {
        debug!("Pointer constraints global available");
    } else {
        debug!("Pointer constraints global not available");
    }

    let screencopy_manager = match globals.bind::<ZwlrScreencopyManagerV1, _, _>(
        &qh,
        1..=MAX_SHM_SCREENCOPY_VERSION,
        (),
    ) {
        Ok(manager) => {
            debug!("Bound zwlr_screencopy_manager_v1");
            Some(manager)
        }
        Err(err) => {
            warn!(
                "zwlr_screencopy_manager_v1 not available; frozen mode may use portal fallback: {}",
                err
            );
            None
        }
    };

    // IME / text-input-v3 for the text and sticky-note tools. Optional: when
    // the compositor lacks it, editing falls back to the raw keysym path
    // (single-key characters only).
    let text_input_manager = match globals.bind::<ZwpTextInputManagerV3, _, _>(&qh, 1..=1, ()) {
        Ok(manager) => {
            debug!("Bound zwp_text_input_manager_v3");
            Some(manager)
        }
        Err(err) => {
            debug!(
                "zwp_text_input_manager_v3 not available; IME disabled: {}",
                err
            );
            None
        }
    };

    let layer_shell_available = layer_shell.is_some();

    let state_globals = WaylandGlobals {
        registry_state,
        compositor_state,
        layer_shell,
        xdg_shell,
        activation,
        shm,
        pointer_constraints_state,
        relative_pointer_state,
        output_state,
        seat_state,
    };

    Ok(WaylandSetup {
        conn,
        #[cfg(feature = "tablet-input")]
        globals,
        event_queue,
        qh,
        state_globals,
        screencopy_manager,
        text_input_manager,
        layer_shell_available,
    })
}

#[cfg(test)]
mod tests {
    use super::MAX_SHM_SCREENCOPY_VERSION;

    #[test]
    fn screencopy_binding_stays_on_wl_shm_compatible_version() {
        assert_eq!(MAX_SHM_SCREENCOPY_VERSION, 2);
    }
}
