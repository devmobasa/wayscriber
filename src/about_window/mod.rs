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
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::time::Duration;
use wayland_client::Connection;
use wayland_client::backend::{ReadEventsGuard, WaylandError};
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

pub fn run_about_window(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    config_store: &crate::config::ConfigStore,
    update_cache: crate::update_check::UpdateCacheStore,
) -> Result<()> {
    // Chrome colors come from the same `[ui] theme` key as the overlay, so the
    // dialog matches the toolbars the user already sees.
    let theme = match config_store.load() {
        Ok(loaded) => crate::ui::theme::Theme::from_mode(loaded.config.ui.theme.to_theme_mode()),
        Err(err) => {
            debug!("About dialog falling back to the default theme: {err}");
            crate::ui::theme::Theme::from_mode(Config::default().ui.theme.to_theme_mode())
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
    let clipboard_wake = crate::backend::wayland::RuntimeWakeSource::new()
        .context("Failed to create About clipboard completion source")?;
    let clipboard_wake_sender = clipboard_wake
        .try_sender()
        .context("Failed to duplicate About clipboard completion source")?;

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
        process_broker.clone(),
        update_cache,
        clipboard_wake_sender,
    );

    let event_loop_result = (|| -> Result<()> {
        loop {
            event_queue.dispatch_pending(&mut state)?;
            state.settle_finished_clipboard_jobs();
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
            event_queue
                .flush()
                .context("Failed to flush Wayland connection")?;
            let Some(prepared_read) = event_queue.prepare_read() else {
                continue;
            };
            let outcome = read_about_events_with_wake(prepared_read, &clipboard_wake, None)?;
            if outcome.completion_wake {
                state.settle_finished_clipboard_jobs();
            }
        }
        Ok(())
    })();

    // The process broker owner lives in the caller. Settle every accepted copy
    // before this root returns so its broker handle cannot be invalidated first.
    state.settle_clipboard_jobs();
    event_loop_result
}

trait PreparedAboutRead {
    fn connection_raw_fd(&self) -> RawFd;
    fn read(self) -> std::result::Result<usize, WaylandError>;
}

impl PreparedAboutRead for ReadEventsGuard {
    fn connection_raw_fd(&self) -> RawFd {
        self.connection_fd().as_raw_fd()
    }

    fn read(self) -> std::result::Result<usize, WaylandError> {
        ReadEventsGuard::read(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AboutReadOutcome {
    wayland_read: bool,
    completion_wake: bool,
}

fn read_about_events_with_wake(
    prepared_read: impl PreparedAboutRead,
    completion_wake: &crate::backend::wayland::RuntimeWakeSource,
    timeout: Option<Duration>,
) -> Result<AboutReadOutcome> {
    let mut pollfds = [
        libc::pollfd {
            fd: prepared_read.connection_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: completion_wake.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    let timeout_ms = timeout
        .map(|duration| {
            duration
                .as_nanos()
                .div_ceil(1_000_000)
                .min(i32::MAX as u128) as i32
        })
        .unwrap_or(-1);
    loop {
        // SAFETY: pollfds contains two initialized entries whose owners remain
        // live throughout this bounded or event-driven wait.
        let ready = unsafe {
            libc::poll(
                pollfds.as_mut_ptr(),
                pollfds.len() as libc::nfds_t,
                timeout_ms,
            )
        };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error).context("About readiness poll failed");
        }
        if ready == 0 {
            drop(prepared_read);
            return Ok(AboutReadOutcome {
                wayland_read: false,
                completion_wake: false,
            });
        }
        break;
    }

    let wayland_readable = validate_about_readiness(&pollfds[0], "Wayland", true)?;
    let completion_readable =
        validate_about_readiness(&pollfds[1], "About clipboard completion", false)?;
    if !wayland_readable && !completion_readable {
        drop(prepared_read);
        return Err(anyhow::anyhow!(
            "About poll returned without a readable descriptor"
        ));
    }

    if wayland_readable {
        match prepared_read.read() {
            Ok(_) => {}
            Err(WaylandError::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(anyhow::anyhow!(error.to_string())),
        }
    } else {
        // Cancels the prepared read before processing the non-Wayland source.
        drop(prepared_read);
    }
    if completion_readable {
        completion_wake
            .drain()
            .context("Failed to drain About clipboard completion source")?;
    }

    Ok(AboutReadOutcome {
        wayland_read: wayland_readable,
        completion_wake: completion_readable,
    })
}

fn validate_about_readiness(
    pollfd: &libc::pollfd,
    label: &str,
    read_buffered_terminal: bool,
) -> io::Result<bool> {
    let readable = pollfd.revents & libc::POLLIN != 0;
    if pollfd.revents & libc::POLLNVAL != 0 {
        return Err(io::Error::other(format!(
            "{label} poll descriptor is invalid ({:#x})",
            pollfd.revents
        )));
    }
    let terminal = pollfd.revents & (libc::POLLERR | libc::POLLHUP);
    if terminal != 0 && !(read_buffered_terminal && readable) {
        return Err(io::Error::other(format!(
            "{label} poll descriptor failed ({:#x})",
            pollfd.revents
        )));
    }
    if pollfd.revents != 0 && !readable {
        return Err(io::Error::other(format!(
            "{label} poll descriptor returned unexpected readiness ({:#x})",
            pollfd.revents
        )));
    }
    Ok(readable)
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
    content: AboutContent,
    plan: Plan,
    theme: crate::ui::theme::Theme,
    update: UpdateState,
    /// Focus order for the current update state.
    elements: Vec<Element>,
    hover: Option<Element>,
    focus: Option<Element>,
    /// Live Shift state, so Shift-Tab works on layouts that send a plain `Tab`.
    shift_held: bool,
    notice: Option<String>,
    process_broker: crate::process_broker::ProcessBrokerHandle,
    update_cache: crate::update_check::UpdateCacheStore,
    clipboard_jobs: clipboard::ClipboardCopyJobs,
    icon: Option<cairo::ImageSurface>,
    themed_pointer: Option<
        smithay_client_toolkit::seat::pointer::ThemedPointer<
            smithay_client_toolkit::seat::pointer::PointerData,
        >,
    >,
}
