use anyhow::{Context, Result};
use log::{debug, warn};
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::registry_handlers;
use smithay_client_toolkit::seat::keyboard::{
    KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers,
};
use smithay_client_toolkit::seat::pointer::{
    BTN_LEFT, CursorIcon, PointerEvent, PointerEventKind, PointerHandler,
};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState, pointer::ThemeSpec};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::xdg::window::{
    Window, WindowConfigure, WindowDecorations, WindowHandler,
};
use smithay_client_toolkit::shm::{Shm, ShmHandler, slot::SlotPool};
use smithay_client_toolkit::{
    delegate_compositor, delegate_keyboard, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm, delegate_xdg_shell, delegate_xdg_window,
};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{
    wl_buffer, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface,
};
use wayland_client::{Connection, Dispatch, QueueHandle};

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

impl AboutWindowState {
    fn new(
        registry_state: RegistryState,
        compositor_state: CompositorState,
        shm: Shm,
        output_state: OutputState,
        seat_state: SeatState,
        xdg_shell: XdgShell,
        window: Window,
    ) -> Self {
        Self {
            registry_state,
            compositor_state,
            shm,
            output_state,
            seat_state,
            xdg_shell,
            window,
            pool: None,
            width: ABOUT_WIDTH,
            height: ABOUT_HEIGHT,
            scale: 1,
            configured: false,
            should_exit: false,
            needs_redraw: true,
            link_regions: Vec::new(),
            hover_index: None,
            themed_pointer: None,
        }
    }

    fn link_index_at(&self, pos: (f64, f64)) -> Option<usize> {
        self.link_regions.iter().position(|link| link.contains(pos))
    }

    fn update_hover(&mut self, pos: (f64, f64)) {
        let next = self.link_index_at(pos);
        if next != self.hover_index {
            self.hover_index = next;
            self.needs_redraw = true;
        }
    }

    fn update_cursor(&self, conn: &Connection) {
        if let Some(pointer) = self.themed_pointer.as_ref() {
            let icon = if self.hover_index.is_some() {
                CursorIcon::Pointer
            } else {
                CursorIcon::Default
            };
            if let Err(err) = pointer.set_cursor(conn, icon) {
                debug!("Failed to set cursor icon: {}", err);
            }
        }
    }

    fn render(&mut self) -> Result<()> {
        if !self.configured {
            return Ok(());
        }

        let (phys_w, phys_h) = (
            self.width.saturating_mul(self.scale as u32),
            self.height.saturating_mul(self.scale as u32),
        );

        if self.pool.is_none() {
            let buffer_size = (phys_w * phys_h * 4) as usize;
            let pool = SlotPool::new(buffer_size, &self.shm)
                .context("Failed to create about window buffer pool")?;
            self.pool = Some(pool);
        }

        let pool = match self.pool.as_mut() {
            Some(pool) => pool,
            None => return Ok(()),
        };
        let (buffer, canvas) = pool
            .create_buffer(
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
                wayland_client::protocol::wl_shm::Format::Argb8888,
            )
            .context("Failed to create about window buffer")?;

        let surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                canvas.as_mut_ptr(),
                cairo::Format::ARgb32,
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
            )
        }
        .context("Failed to create Cairo surface")?;
        let ctx = cairo::Context::new(&surface).context("Failed to create Cairo context")?;

        ctx.set_operator(cairo::Operator::Clear);
        let _ = ctx.paint();
        ctx.set_operator(cairo::Operator::Over);
        if self.scale > 1 {
            ctx.scale(self.scale as f64, self.scale as f64);
        }

        self.link_regions.clear();
        draw_about(
            &ctx,
            self.width as f64,
            self.height as f64,
            &mut self.link_regions,
            self.hover_index,
        );

        surface.flush();

        let wl_surface = self.window.wl_surface();
        wl_surface.set_buffer_scale(self.scale);
        wl_surface.attach(Some(buffer.wl_buffer()), 0, 0);
        wl_surface.damage_buffer(0, 0, phys_w as i32, phys_h as i32);
        wl_surface.commit();

        self.needs_redraw = false;
        Ok(())
    }
}

impl CompositorHandler for AboutWindowState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        let next = new_factor.max(1);
        if self.scale != next {
            self.scale = next;
            self.pool = None;
            self.needs_redraw = true;
        }
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
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
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
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

impl OutputHandler for AboutWindowState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl ShmHandler for AboutWindowState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl SeatHandler for AboutWindowState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            match self.seat_state.get_pointer_with_theme(
                qh,
                &seat,
                self.shm.wl_shm(),
                self.compositor_state.create_surface(qh),
                ThemeSpec::default(),
            ) {
                Ok(pointer) => {
                    self.themed_pointer = Some(pointer);
                }
                Err(err) => {
                    warn!("Pointer initialized without theme: {}", err);
                    let _ = self.seat_state.get_pointer(qh, &seat);
                }
            }
        }

        if capability == Capability::Keyboard {
            let _ = self.seat_state.get_keyboard(qh, &seat, None);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            self.themed_pointer = None;
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
    }
}

impl PointerHandler for AboutWindowState {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if &event.surface != self.window.wl_surface() {
                continue;
            }
            match event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.update_hover(event.position);
                    self.update_cursor(conn);
                }
                PointerEventKind::Leave { .. } => {
                    if self.hover_index.is_some() {
                        self.hover_index = None;
                        self.needs_redraw = true;
                    }
                    self.update_cursor(conn);
                }
                PointerEventKind::Press { button, .. } => {
                    if button == BTN_LEFT
                        && let Some(index) = self.link_index_at(event.position)
                        && let Some(link) = self.link_regions.get(index)
                    {
                        match &link.action {
                            LinkAction::OpenUrl(url) => open_url(url),
                            LinkAction::CopyText(text) => copy_text_to_clipboard(text),
                            LinkAction::Close => self.should_exit = true,
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

impl KeyboardHandler for AboutWindowState {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        if event.keysym == Keysym::Escape {
            self.should_exit = true;
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _layout: RawModifiers,
        _group: u32,
    ) {
    }

    fn repeat_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }
}

impl WindowHandler for AboutWindowState {
    fn request_close(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _window: &Window) {
        self.should_exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        let width = configure
            .new_size
            .0
            .map(|w| w.get())
            .unwrap_or(ABOUT_WIDTH)
            .max(1);
        let height = configure
            .new_size
            .1
            .map(|h| h.get())
            .unwrap_or(ABOUT_HEIGHT)
            .max(1);

        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.pool = None;
        }

        self.configured = true;
        self.needs_redraw = true;
    }
}

impl ProvidesRegistryState for AboutWindowState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}

impl Dispatch<wl_buffer::WlBuffer, ()> for AboutWindowState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        _event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

delegate_compositor!(AboutWindowState);
delegate_output!(AboutWindowState);
delegate_shm!(AboutWindowState);
delegate_seat!(AboutWindowState);
delegate_keyboard!(AboutWindowState);
delegate_pointer!(AboutWindowState);
delegate_registry!(AboutWindowState);
delegate_xdg_shell!(AboutWindowState);
delegate_xdg_window!(AboutWindowState);

fn draw_about(
    ctx: &cairo::Context,
    width: f64,
    height: f64,
    links: &mut Vec<LinkRegion>,
    hover_index: Option<usize>,
) {
    let margin = 22.0;
    let mut y = margin + 20.0;

    ctx.set_source_rgb(0.96, 0.95, 0.93);
    ctx.rectangle(0.0, 0.0, width, height);
    let _ = ctx.fill();

    ctx.set_source_rgb(0.18, 0.18, 0.18);
    ctx.set_line_width(1.0);
    ctx.rectangle(0.5, 0.5, width - 1.0, height - 1.0);
    let _ = ctx.stroke();

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(18.0);
    let title = format!("Wayscriber version {}", version());
    draw_text(ctx, margin, y, &title);

    y += 28.0;
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(13.0);

    let mut link_index = 0usize;
    let close_size = 16.0;
    let close_padding = 10.0;
    let close_x = width - close_padding - close_size;
    let close_y = close_padding;
    let close_rect = draw_close_button(
        ctx,
        close_x,
        close_y,
        close_size,
        hover_index == Some(link_index),
    );
    links.push(LinkRegion {
        rect: close_rect,
        action: LinkAction::Close,
    });
    link_index += 1;

    y = add_link_line(
        ctx,
        margin,
        y,
        &format!("Website: {}", WEBSITE_URL.trim_start_matches("https://")),
        LinkAction::OpenUrl(WEBSITE_URL.to_string()),
        link_index,
        &mut LinkRenderState { hover_index, links },
    );
    link_index += 1;

    y = add_link_line(
        ctx,
        margin,
        y,
        "GitHub: github.com/devmobasa/wayscriber",
        LinkAction::OpenUrl(GITHUB_URL.to_string()),
        link_index,
        &mut LinkRenderState { hover_index, links },
    );
    link_index += 1;

    let commit = commit_hash();
    let commit_line = format!("Commit: {}", commit);
    ctx.set_source_rgb(0.25, 0.25, 0.25);
    let commit_rect = draw_text(ctx, margin, y, &commit_line);
    if commit != "unknown" {
        let button_size = 14.0;
        let text_right = commit_rect.0 + commit_rect.2;
        let button_x = text_right + 8.0;
        let button_y = commit_rect.1 + (commit_rect.3 - button_size) / 2.0;
        let rect = draw_copy_button(
            ctx,
            button_x,
            button_y,
            button_size,
            hover_index == Some(link_index),
        );
        links.push(LinkRegion {
            rect,
            action: LinkAction::CopyText(commit.to_string()),
        });
    }

    ctx.set_source_rgb(0.4, 0.4, 0.4);
    ctx.set_font_size(11.0);
    draw_text(ctx, margin, height - 16.0, "Press Esc or click X to close");
}

struct LinkRenderState<'a> {
    hover_index: Option<usize>,
    links: &'a mut Vec<LinkRegion>,
}

fn add_link_line(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    text: &str,
    action: LinkAction,
    index: usize,
    state: &mut LinkRenderState<'_>,
) -> f64 {
    let is_hover = state.hover_index == Some(index);
    if is_hover {
        ctx.set_source_rgb(0.08, 0.38, 0.75);
    } else {
        ctx.set_source_rgb(0.12, 0.45, 0.84);
    }
    let rect = draw_text(ctx, x, y, text);
    ctx.set_line_width(1.0);
    ctx.move_to(rect.0, rect.1 + rect.3 + 2.0);
    ctx.line_to(rect.0 + rect.2, rect.1 + rect.3 + 2.0);
    let _ = ctx.stroke();
    state.links.push(LinkRegion { rect, action });
    y + 22.0
}

fn draw_text(ctx: &cairo::Context, x: f64, y: f64, text: &str) -> (f64, f64, f64, f64) {
    ctx.move_to(x, y);
    let _ = ctx.show_text(text);
    let extents = match ctx.text_extents(text) {
        Ok(extents) => extents,
        Err(_) => fallback_text_extents(ctx, text),
    };
    (
        x + extents.x_bearing(),
        y + extents.y_bearing(),
        extents.width(),
        extents.height(),
    )
}

fn fallback_text_extents(ctx: &cairo::Context, text: &str) -> cairo::TextExtents {
    let height = ctx
        .font_extents()
        .map(|extents| extents.height())
        .unwrap_or(14.0);
    let width = text.len() as f64 * height * 0.5;
    cairo::TextExtents::new(0.0, -height, width, height, width, 0.0)
}

fn draw_copy_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
) -> (f64, f64, f64, f64) {
    let radius = 3.0;
    let (bg_r, bg_g, bg_b) = if hover {
        (0.85, 0.9, 0.98)
    } else {
        (0.92, 0.94, 0.96)
    };
    ctx.set_source_rgb(bg_r, bg_g, bg_b);
    draw_rounded_rect(ctx, x, y, size, size, radius);
    let _ = ctx.fill();

    ctx.set_source_rgb(0.55, 0.6, 0.68);
    ctx.set_line_width(1.0);
    draw_rounded_rect(ctx, x, y, size, size, radius);
    let _ = ctx.stroke();

    let pad = 3.0;
    let icon_size = size - pad * 2.0;
    let back = (x + pad + 2.0, y + pad - 1.0);
    let front = (x + pad - 1.0, y + pad + 2.0);
    ctx.set_source_rgb(0.35, 0.4, 0.48);
    draw_rounded_rect(ctx, back.0, back.1, icon_size, icon_size, 2.0);
    let _ = ctx.stroke();
    draw_rounded_rect(ctx, front.0, front.1, icon_size, icon_size, 2.0);
    let _ = ctx.stroke();

    (x, y, size, size)
}

fn draw_close_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
) -> (f64, f64, f64, f64) {
    let radius = 3.0;
    let (bg_r, bg_g, bg_b) = if hover {
        (0.98, 0.88, 0.88)
    } else {
        (0.96, 0.92, 0.92)
    };
    ctx.set_source_rgb(bg_r, bg_g, bg_b);
    draw_rounded_rect(ctx, x, y, size, size, radius);
    let _ = ctx.fill();

    ctx.set_source_rgb(0.7, 0.55, 0.55);
    ctx.set_line_width(1.0);
    draw_rounded_rect(ctx, x, y, size, size, radius);
    let _ = ctx.stroke();

    ctx.set_source_rgb(0.4, 0.25, 0.25);
    ctx.set_line_width(1.6);
    let inset = 4.0;
    ctx.move_to(x + inset, y + inset);
    ctx.line_to(x + size - inset, y + size - inset);
    let _ = ctx.stroke();
    ctx.move_to(x + size - inset, y + inset);
    ctx.line_to(x + inset, y + size - inset);
    let _ = ctx.stroke();

    (x, y, size, size)
}

fn draw_rounded_rect(ctx: &cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let r = radius.min(width / 2.0).min(height / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + width - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    ctx.arc(
        x + width - r,
        y + height - r,
        r,
        0.0,
        std::f64::consts::FRAC_PI_2,
    );
    ctx.arc(
        x + r,
        y + height - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    ctx.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    ctx.close_path();
}

fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn commit_hash() -> &'static str {
    env!("WAYSCRIBER_GIT_HASH")
}

fn open_url(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "xdg-open"
    };

    let mut cmd = std::process::Command::new(opener);
    if cfg!(target_os = "windows") {
        cmd.args(["/C", "start", ""]).arg(url);
    } else {
        cmd.arg(url);
    }
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    if let Err(err) = cmd.spawn() {
        warn!("Failed to open URL {}: {}", url, err);
    }
}

fn copy_text_to_clipboard(text: &str) {
    if text.is_empty() {
        return;
    }
    let text = text.to_string();
    std::thread::spawn(move || {
        if copy_text_via_command(&text).is_ok() {
            return;
        }
        if let Err(err) = copy_text_via_library(&text) {
            warn!("Failed to copy commit id to clipboard: {}", err);
        }
    });
}

fn copy_text_via_library(text: &str) -> Result<()> {
    use wl_clipboard_rs::copy::{MimeType, Options, ServeRequests, Source};

    let mut opts = Options::new();
    opts.serve_requests(ServeRequests::Only(1));
    opts.copy(Source::Bytes(text.as_bytes().into()), MimeType::Text)
        .context("wl-clipboard-rs text copy failed")?;
    Ok(())
}

fn copy_text_via_command(text: &str) -> Result<()> {
    use std::io::Write;

    let mut child = std::process::Command::new("wl-copy")
        .arg("--type")
        .arg("text/plain")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn wl-copy")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .context("Failed to write to wl-copy stdin")?;
    }

    let output = child
        .wait_with_output()
        .context("Failed to wait for wl-copy")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("wl-copy failed: {}", stderr.trim()));
    }

    Ok(())
}
