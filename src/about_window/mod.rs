//! The standalone About dialog (`wayscriber --about`).
//!
//! Runs its own small Wayland client rather than borrowing the annotation
//! overlay's backend: the dialog is a plain toplevel with its own chrome,
//! sized to its content and painted from the shared theme tokens.

use anyhow::{Context, Result};
use log::debug;
use smithay_client_toolkit::compositor::CompositorState;
use smithay_client_toolkit::output::OutputState;
use smithay_client_toolkit::registry::RegistryState;
use smithay_client_toolkit::seat::SeatState;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::xdg::window::{Window, WindowDecorations};
use smithay_client_toolkit::shm::{Shm, slot::SlotPool};
use wayland_client::Connection;
use wayland_client::globals::registry_queue_init;

use crate::app_id::runtime_app_id;
use crate::config::Config;

mod clipboard;
mod content;
mod diagnostics;
mod handlers;
mod icon;
mod interaction;
mod layout;
mod render;
mod state;

use content::{AboutContent, UpdateState};
use interaction::Element;
use layout::Plan;

pub fn run_about_window() -> Result<()> {
    // Chrome colors come from the same `[ui] theme` key as the overlay, so the
    // dialog matches the toolbars the user already sees.
    let theme = match Config::load() {
        Ok(loaded) => crate::ui::theme::Theme::resolve(loaded.config.ui.theme.to_theme_mode()),
        Err(err) => {
            debug!("About dialog falling back to the default theme: {err}");
            crate::ui::theme::Theme::resolve(Config::default().ui.theme.to_theme_mode())
        }
    };

    let conn = Connection::connect_to_env().context("Failed to connect to Wayland compositor")?;
    let (globals, mut event_queue) =
        registry_queue_init(&conn).context("Failed to initialize Wayland registry")?;
    let qh = event_queue.handle();

    let compositor_state =
        CompositorState::bind(&globals, &qh).context("wl_compositor not available")?;
    let shm = Shm::bind(&globals, &qh).context("wl_shm not available")?;
    let xdg_shell = XdgShell::bind(&globals, &qh).context("xdg-shell not available")?;
    let output_state = OutputState::new(&globals, &qh);
    let seat_state = SeatState::new(&globals, &qh);
    let registry_state = RegistryState::new(&globals);

    let content = AboutContent::build();
    let plan = layout::plan(&content);
    let (width, height) = surface_size(&plan);

    let wl_surface = compositor_state.create_surface(&qh);
    let window = xdg_shell.create_window(wl_surface, WindowDecorations::None, &qh);
    window.set_title("About Wayscriber");
    window.set_app_id(runtime_app_id());
    // The dialog is content-sized; letting the compositor resize it would only
    // clip rows or leave dead space.
    window.set_min_size(Some((width, height)));
    window.set_max_size(Some((width, height)));
    window.commit();

    let mut state = AboutWindowState::new(
        registry_state,
        compositor_state,
        shm,
        output_state,
        seat_state,
        xdg_shell,
        window,
        content,
        plan,
        theme,
    );

    // Join helpers on every return path so ProcessBrokerGuard teardown cannot
    // cancel an in-flight Report/open/copy that already showed a success notice.
    let result = run_about_event_loop(&conn, &mut event_queue, &mut state);
    state.join_helper_workers();
    result
}

fn run_about_event_loop(
    conn: &Connection,
    event_queue: &mut wayland_client::EventQueue<AboutWindowState>,
    state: &mut AboutWindowState,
) -> Result<()> {
    loop {
        event_queue.blocking_dispatch(state)?;
        if state.should_exit {
            break;
        }
        if state.needs_redraw {
            state.render()?;
        }
        if state.check_requested {
            state.check_requested = false;
            // Show the "Checking…" frame before blocking on the network, then
            // repaint with the verdict. A few seconds of a frozen dialog is
            // preferable to threading an event source into this tiny loop.
            conn.flush().context("Failed to flush Wayland connection")?;
            state.run_update_check();
            state.render()?;
        }
    }

    Ok(())
}

/// Logical surface size for a plan, rounded up so nothing is clipped by a
/// fractional pixel.
fn surface_size(plan: &Plan) -> (u32, u32) {
    (
        plan.width.ceil().max(1.0) as u32,
        plan.height.ceil().max(1.0) as u32,
    )
}

struct AboutWindowState {
    theme: crate::ui::theme::Theme,
    registry_state: RegistryState,
    compositor_state: CompositorState,
    shm: Shm,
    output_state: OutputState,
    seat_state: SeatState,
    #[allow(dead_code)]
    xdg_shell: XdgShell,
    window: Window,
    pool: Option<SlotPool>,
    width: u32,
    height: u32,
    scale: i32,
    configured: bool,
    should_exit: bool,
    needs_redraw: bool,
    /// Set when the update card is activated; serviced by the event loop so the
    /// blocking fetch never runs inside a protocol handler.
    check_requested: bool,
    /// Open/copy workers that must finish before the process broker shuts down.
    helper_workers: Vec<std::thread::JoinHandle<()>>,
    content: AboutContent,
    plan: Plan,
    update: UpdateState,
    /// Focus order for the current update state.
    elements: Vec<Element>,
    hover: Option<Element>,
    focus: Option<Element>,
    /// Live Shift state, so Shift-Tab works on layouts that send a plain `Tab`.
    shift_held: bool,
    notice: Option<String>,
    icon: Option<cairo::ImageSurface>,
    themed_pointer: Option<
        smithay_client_toolkit::seat::pointer::ThemedPointer<
            smithay_client_toolkit::seat::pointer::PointerData,
        >,
    >,
}
