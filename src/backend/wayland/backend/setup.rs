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
use wayland_protocols::ext::{
    image_capture_source::v1::client::ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
    image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1,
};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::env_vars::{XDG_CURRENT_DESKTOP_ENV, XDG_SESSION_DESKTOP_ENV};

use super::super::{
    frozen::ExtImageCopyManagers,
    state::{ProtocolGlobals, ProtocolGlobalsSeed, WaylandState},
};

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
    pub(super) state_globals: ProtocolGlobals,
    pub(super) screencopy_manager: Option<ZwlrScreencopyManagerV1>,
    pub(super) ext_image_copy_managers: Option<ExtImageCopyManagers>,
    pub(super) text_input_manager: Option<ZwpTextInputManagerV3>,
    pub(super) layer_shell_available: bool,
}

type ShellGlobals = (
    Option<LayerShell>,
    Option<XdgShell>,
    Option<ActivationState>,
);

fn bind_shell_globals(
    globals: &wayland_client::globals::GlobalList,
    qh: &wayland_client::QueueHandle<WaylandState>,
) -> Result<ShellGlobals> {
    let layer_shell = match LayerShell::bind(globals, qh) {
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
    let xdg_shell = match XdgShell::bind(globals, qh) {
        Ok(shell) => {
            debug!("Bound xdg-shell");
            Some(shell)
        }
        Err(err) => {
            warn!("xdg-shell not available: {}", err);
            None
        }
    };
    let activation = match ActivationState::bind(globals, qh) {
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
    Ok((layer_shell, xdg_shell, activation))
}

fn bind_capture_globals(
    globals: &wayland_client::globals::GlobalList,
    qh: &wayland_client::QueueHandle<WaylandState>,
) -> (
    Option<ZwlrScreencopyManagerV1>,
    Option<ExtImageCopyManagers>,
) {
    let screencopy_manager = match globals.bind::<ZwlrScreencopyManagerV1, _, _>(
        qh,
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
    let ext_image_copy_manager = globals
        .bind::<ExtImageCopyCaptureManagerV1, _, _>(qh, 1..=1, ())
        .ok();
    let ext_output_source_manager = globals
        .bind::<ExtOutputImageCaptureSourceManagerV1, _, _>(qh, 1..=1, ())
        .ok();
    let ext_image_copy_managers = match (ext_image_copy_manager, ext_output_source_manager) {
        (Some(capture), Some(output_source)) => {
            debug!("Bound ext-image-copy-capture output backend");
            Some(ExtImageCopyManagers::new(capture, output_source))
        }
        (capture, output_source) => {
            debug!(
                "ext-image-copy-capture output backend unavailable: capture_manager={}, output_source_manager={}",
                capture.is_some(),
                output_source.is_some()
            );
            None
        }
    };
    (screencopy_manager, ext_image_copy_managers)
}

fn bind_text_input_manager(
    globals: &wayland_client::globals::GlobalList,
    qh: &wayland_client::QueueHandle<WaylandState>,
) -> Option<ZwpTextInputManagerV3> {
    match globals.bind::<ZwpTextInputManagerV3, _, _>(qh, 1..=1, ()) {
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
    }
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

    let (layer_shell, xdg_shell, activation) = bind_shell_globals(&globals, &qh)?;

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

    let (screencopy_manager, ext_image_copy_managers) = bind_capture_globals(&globals, &qh);

    // IME / text-input-v3 for the text and sticky-note tools. Optional: when
    // the compositor lacks it, editing falls back to the raw keysym path
    // (single-key characters only).
    let text_input_manager = bind_text_input_manager(&globals, &qh);

    let layer_shell_available = layer_shell.is_some();

    let state_globals = ProtocolGlobals::from_seed(ProtocolGlobalsSeed {
        registry: registry_state,
        compositor: compositor_state,
        layer_shell,
        xdg_shell,
        activation,
        shm,
        pointer_constraints: pointer_constraints_state,
        relative_pointer: relative_pointer_state,
        output: output_state,
        seat: seat_state,
    });

    Ok(WaylandSetup {
        conn,
        #[cfg(feature = "tablet-input")]
        globals,
        event_queue,
        qh,
        state_globals,
        screencopy_manager,
        ext_image_copy_managers,
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
