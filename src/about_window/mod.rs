use anyhow::{Context, Result};
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

mod clipboard;
mod handlers;
mod render;
mod state;

const ABOUT_WIDTH: u32 = 360;
const ABOUT_HEIGHT: u32 = 220;
const WEBSITE_URL: &str = "https://wayscriber.com";
const GITHUB_URL: &str = "https://github.com/devmobasa/wayscriber";

pub fn run_about_window() -> Result<()> {
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

    let wl_surface = compositor_state.create_surface(&qh);
    let window = xdg_shell.create_window(wl_surface, WindowDecorations::None, &qh);
    window.set_title("Wayscriber About");
    window.set_app_id("com.devmobasa.wayscriber");
    window.set_min_size(Some((ABOUT_WIDTH, ABOUT_HEIGHT)));
    window.set_max_size(Some((ABOUT_WIDTH, ABOUT_HEIGHT)));
    window.commit();

    let mut state = AboutWindowState::new(
        registry_state,
        compositor_state,
        shm,
        output_state,
        seat_state,
        xdg_shell,
        window,
    );

    loop {
        event_queue.blocking_dispatch(&mut state)?;
        if state.should_exit {
            break;
        }
        if state.needs_redraw {
            state.render()?;
        }
    }

    Ok(())
}

enum LinkAction {
    OpenUrl(String),
    CopyText(String),
    Close,
}

struct LinkRegion {
    rect: (f64, f64, f64, f64),
    action: LinkAction,
}

impl LinkRegion {
    fn contains(&self, pos: (f64, f64)) -> bool {
        let (x, y) = pos;
        x >= self.rect.0
            && x <= self.rect.0 + self.rect.2
            && y >= self.rect.1
            && y <= self.rect.1 + self.rect.3
    }
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
    link_regions: Vec<LinkRegion>,
    hover_index: Option<usize>,
    themed_pointer: Option<
        smithay_client_toolkit::seat::pointer::ThemedPointer<
            smithay_client_toolkit::seat::pointer::PointerData,
        >,
    >,
}
