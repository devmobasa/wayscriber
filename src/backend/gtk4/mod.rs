//! GTK4 backend implementation for GNOME-based compositors.
//!
//! This backend is guarded by the `gtk-backend` feature and provides a
//! transparent fullscreen overlay using GTK4/GDK on Wayland. It currently
//! focuses on core drawing functionality; advanced features (screen capture,
//! multi-monitor awareness) will be iterated on in follow-up changes.

#![cfg(feature = "gtk-backend")]

use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use gdk::prelude::ListModelExt;
use glib::prelude::Cast;
use gtk4::{
    gdk,
    glib::{self, ControlFlow, Propagation, SignalHandlerId, SourceId},
    prelude::*,
};
use log::{debug, error, info, warn};

use crate::{
    backend::{Backend, common},
    capture::{
        CaptureManager, CaptureOutcome,
        file::{FileSaveConfig, expand_tilde},
        types::{CaptureDestination, CaptureType},
    },
    config::{Action, Config, ConfigSource},
    input::{MouseButton, events::Key},
    notification, session,
};

/// GTK backend implementation shared across daemon/CLI modes.
pub struct Gtk4Backend {
    initial_mode: Option<String>,
    application: gtk4::Application,
    runtime: tokio::runtime::Runtime,
    state: Option<Rc<RefCell<GtkState>>>,
    activate_handler: Option<SignalHandlerId>,
    visible: bool,
}

impl Gtk4Backend {
    pub fn new(initial_mode: Option<String>) -> Result<Self> {
        if std::env::var("GSK_RENDERER").is_err() {
            log::debug!("GSK_RENDERER not set; forcing cairo renderer for GTK backend");
            unsafe {
                std::env::set_var("GSK_RENDERER", "cairo");
            }
        }

        let runtime = tokio::runtime::Runtime::new()
            .context("Failed to create Tokio runtime for GTK backend")?;

        let application = gtk4::Application::builder()
            .application_id("com.wayscriber.overlay")
            .build();

        Ok(Self {
            initial_mode,
            application,
            runtime,
            state: None,
            activate_handler: None,
            visible: false,
        })
    }

    fn ensure_state(&mut self) -> Result<Rc<RefCell<GtkState>>> {
        if let Some(state) = &self.state {
            return Ok(state.clone());
        }

        // Load configuration and initialise input state
        let (config, config_source) = common::load_config();
        if matches!(config_source, ConfigSource::Legacy(_)) {
            warn!(
                "Continuing with settings from legacy hyprmarker config. Run `wayscriber --migrate-config` when convenient."
            );
        }

        let mut input_state = common::build_input_state(&config, self.initial_mode.clone())?;

        if let Some(display) = gdk::Display::default() {
            let rgba = display.is_rgba();
            let composited = display.is_composited();
            debug!("GTK display rgba={} composited={}", rgba, composited);
            if !rgba {
                warn!("Display does not support RGBA surfaces; forcing whiteboard fallback");
                state_helpers::force_whiteboard(&mut input_state);
            }
        }

        let capture_manager = CaptureManager::new(self.runtime.handle());

        let config_dir = Config::config_directory_from_source(&config_source)?;
        let display_env = std::env::var("WAYLAND_DISPLAY").ok();

        let session_options = match session::options_from_config(
            &config.session,
            &config_dir,
            display_env.as_deref(),
        ) {
            Ok(opts) => Some(opts),
            Err(err) => {
                warn!("Session persistence disabled: {}", err);
                None
            }
        };

        let mut session_loaded = false;
        if let Some(ref options) = session_options {
            match session::load_snapshot(options) {
                Ok(Some(snapshot)) => {
                    debug!(
                        "Restoring session from {}",
                        options.session_file_path().display()
                    );
                    session::apply_snapshot(&mut input_state, snapshot, options);
                    input_state.needs_redraw = true;
                    session_loaded = true;
                }
                Ok(None) => {
                    debug!(
                        "No session snapshot found at {}",
                        options.session_file_path().display()
                    );
                    session_loaded = true;
                }
                Err(err) => {
                    warn!("Failed to load session state: {}", err);
                    session_loaded = true;
                }
            }
        }

        let state = Rc::new(RefCell::new(GtkState {
            config,
            input_state,
            capture_manager,
            session_options,
            session_loaded,
            capture_in_progress: false,
            desired_visible: false,
            capture_poll: None,
            window: glib::WeakRef::new(),
            tokio_handle: self.runtime.handle().clone(),
            current_mouse_x: 0,
            current_mouse_y: 0,
        }));

        self.state = Some(state.clone());
        Ok(state)
    }

    fn setup_activate_handler(
        &mut self,
        state: Rc<RefCell<GtkState>>,
        ready: Option<mpsc::Sender<Result<()>>>,
        start_visible: bool,
    ) {
        // Disconnect previous handler if show() is called multiple times.
        if let Some(handler) = self.activate_handler.take() {
            self.application.disconnect(handler);
        }

        let state_for_activate = Rc::clone(&state);
        let ready_clone = ready.clone();
        let handler = self.application.connect_activate(move |app| {
            let state = Rc::clone(&state_for_activate);
            match build_overlay(app, state.clone()) {
                Ok(()) => {
                    {
                        let mut state_mut = state.borrow_mut();
                        if start_visible {
                            show_overlay_window(&mut state_mut);
                        } else {
                            hide_overlay_window(&mut state_mut);
                        }
                    }
                    if let Some(tx) = ready_clone.as_ref() {
                        let _ = tx.send(Ok(()));
                    }
                }
                Err(err) => {
                    error!("Failed to initialise GTK overlay: {err:?}");
                    if let Some(tx) = ready_clone.as_ref() {
                        let _ = tx.send(Err(err));
                    }
                    app.quit();
                }
            }
        });

        self.activate_handler = Some(handler);
    }

    fn save_session_snapshot(&self, state: &GtkState) {
        if let Some(options) = state.session_options.as_ref() {
            if let Some(snapshot) = session::snapshot_from_input(&state.input_state, options) {
                if let Err(err) = session::save_snapshot(&snapshot, options) {
                    warn!("Failed to save session state: {}", err);
                    notification::send_notification_async(
                        self.runtime.handle(),
                        "Failed to Save Session".to_string(),
                        format!("Your drawings may not persist: {}", err),
                        Some("dialog-error".to_string()),
                    );
                }
            }
        }
    }

    fn run_daemon_loop(
        &mut self,
        command_rx: mpsc::Receiver<GtkCommand>,
        ready_tx: mpsc::Sender<Result<()>>,
    ) -> Result<()> {
        let state = self.ensure_state()?;
        self.setup_activate_handler(state.clone(), Some(ready_tx), false);

        let app = self.application.clone();
        let state_for_idle = state.clone();
        let command_rx = Arc::new(Mutex::new(command_rx));
        let command_rx_idle = command_rx.clone();
        glib::idle_add_local(move || {
            let guard = command_rx_idle
                .lock()
                .expect("Failed to lock GTK command receiver");
            while let Ok(command) = guard.try_recv() {
                let mut state = state_for_idle.borrow_mut();
                match command {
                    GtkCommand::Show => show_overlay_window(&mut state),
                    GtkCommand::Hide => hide_overlay_window(&mut state),
                    GtkCommand::Quit => {
                        app.quit();
                        return ControlFlow::Break;
                    }
                }
            }
            ControlFlow::Continue
        });

        let status = self.application.run_with_args::<&str>(&[]);

        if let Some(state_rc) = self.state.take() {
            self.save_session_snapshot(&state_rc.borrow());
        } else {
            self.save_session_snapshot(&state.borrow());
        }

        if status.value() != 0 {
            return Err(anyhow!("GTK application exited with status {:?}", status));
        }

        Ok(())
    }
}

impl crate::backend::Backend for Gtk4Backend {
    fn init(&mut self) -> Result<()> {
        info!("Initializing GTK4 backend");
        self.visible = false;
        Ok(())
    }

    fn show(&mut self) -> Result<()> {
        info!("Showing GTK4 overlay");
        let state = self.ensure_state()?;
        self.setup_activate_handler(state.clone(), None, true);
        self.visible = true;

        let status = self.application.run_with_args::<&str>(&[]);
        self.visible = false;
        if let Some(state_rc) = self.state.take() {
            self.save_session_snapshot(&state_rc.borrow());
        } else {
            self.save_session_snapshot(&state.borrow());
        }
        self.state = None;

        if status.value() != 0 {
            return Err(anyhow!("GTK application exited with status {:?}", status));
        }

        Ok(())
    }

    fn hide(&mut self) -> Result<()> {
        if self.visible {
            info!("Requesting GTK overlay shutdown");
            self.application.quit();
            self.visible = false;
        }
        Ok(())
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

/// Runtime state owned by the GTK overlay thread.
struct GtkState {
    config: Config,
    input_state: crate::input::InputState,
    #[allow(dead_code)]
    capture_manager: CaptureManager,
    session_options: Option<session::SessionOptions>,
    #[allow(dead_code)]
    session_loaded: bool,
    capture_in_progress: bool,
    desired_visible: bool,
    capture_poll: Option<SourceId>,
    window: glib::WeakRef<gtk4::ApplicationWindow>,
    tokio_handle: tokio::runtime::Handle,
    current_mouse_x: i32,
    current_mouse_y: i32,
}

fn build_overlay(app: &gtk4::Application, state: Rc<RefCell<GtkState>>) -> Result<()> {
    let display = gdk::Display::default().ok_or_else(|| anyhow!("No default display available"))?;
    let (monitor, (width, height)) = detect_monitor(&display)?;

    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("Wayscriber")
        .default_width(width as i32)
        .default_height(height as i32)
        .decorated(false)
        .resizable(false)
        .build();

    window.fullscreen();
    window.fullscreen_on_monitor(&monitor);
    window.set_default_size(width as i32, height as i32);
    window.set_deletable(true);
    if let Some(surface) = window.surface() {
        surface.set_opaque_region(None);
    }

    state.borrow_mut().window = window.downgrade();

    apply_transparent_css(&window);

    let drawing_area = gtk4::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .build();

    let resize_state = state.clone();
    drawing_area.connect_resize(move |_, width, height| {
        debug!(
            "GTK resize allocation: {}x{} (mode={:?})",
            width,
            height,
            resize_state.borrow().input_state.board_mode()
        );
    });

    let draw_state = state.clone();
    drawing_area.set_draw_func(move |area, ctx, width, height| {
        if let Err(err) = render_canvas(&draw_state, ctx, width, height) {
            log::error!("GTK render error: {err:?}");
        } else {
            // Queue another frame if the input state still needs redraw.
            if draw_state.borrow().input_state.needs_redraw {
                area.queue_draw();
            }
        }
    });

    register_input_handlers(&window, &drawing_area, state.clone());

    let monitor_for_map = monitor.clone();
    window.connect_map(move |win| {
        debug!("GTK window mapped; enforcing fullscreen on monitor");
        win.fullscreen_on_monitor(&monitor_for_map);
        win.present();
    });

    let app_weak = app.downgrade();
    let tick_state = state.clone();
    drawing_area.add_tick_callback(move |area, _clock| {
        {
            let mut state_mut = tick_state.borrow_mut();

            if state_mut.input_state.should_exit {
                info!("GTK backend detected exit request; closing window");
                state_mut.input_state.should_exit = false;
                if let Some(app) = app_weak.upgrade() {
                    drop(state_mut);
                    app.quit();
                    return ControlFlow::Break;
                }
            }

            if let Some(action) = state_mut.input_state.take_pending_capture_action() {
                drop(state_mut);
                handle_capture_action(&tick_state, action);
                return ControlFlow::Continue;
            }

            if state_mut.input_state.needs_redraw {
                debug!("GTK backend scheduling redraw");
                area.queue_draw();
                state_mut.input_state.needs_redraw = false;
            }
        }

        ControlFlow::Continue
    });

    window.set_child(Some(&drawing_area));
    window.present();
    drawing_area.queue_draw();

    Ok(())
}

fn detect_monitor(display: &gdk::Display) -> Result<(gdk::Monitor, (u32, u32))> {
    let monitors = display.monitors();
    let monitor = monitors
        .item(0)
        .and_then(|obj| obj.downcast::<gdk::Monitor>().ok())
        .ok_or_else(|| anyhow!("No monitor found"))?;
    let geometry = monitor.geometry();
    let size = (geometry.width() as u32, geometry.height() as u32);
    debug!("Detected monitor geometry {:?}", size);
    Ok((monitor, size))
}

fn apply_transparent_css(window: &gtk4::ApplicationWindow) {
    let css_provider = gtk4::CssProvider::new();
    css_provider.load_from_string(
        r#"
        window {
            background-color: transparent;
        }
        drawingarea {
            background-color: transparent;
        }
    "#,
    );

    let display = gtk4::prelude::WidgetExt::display(window);
    gtk4::style_context_add_provider_for_display(
        &display,
        &css_provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn hide_overlay_window(state: &mut GtkState) {
    if let Some(window) = state.window.upgrade() {
        window.set_visible(false);
    }
    state.desired_visible = false;
    debug!("GTK overlay hidden (desired_visible=false)");
    state.input_state.needs_redraw = false;
}

fn show_overlay_window(state: &mut GtkState) {
    if let Some(window) = state.window.upgrade() {
        window.set_visible(true);
        window.present();
    }
    state.desired_visible = true;
    debug!("GTK overlay shown (desired_visible=true)");
    state.input_state.needs_redraw = true;
}

fn hide_overlay_for_capture(state: &mut GtkState) {
    if let Some(window) = state.window.upgrade() {
        window.set_visible(false);
    }
    debug!("GTK overlay hidden for capture");
    state.input_state.needs_redraw = false;
}

fn handle_capture_action(state: &Rc<RefCell<GtkState>>, action: Action) {
    {
        let mut state_mut = state.borrow_mut();

        if !state_mut.config.capture.enabled {
            warn!("Capture action triggered but capture is disabled in config");
            return;
        }

        if state_mut.capture_in_progress {
            warn!(
                "Capture action {:?} requested while another capture is running; ignoring",
                action
            );
            return;
        }

        let default_destination = if state_mut.config.capture.copy_to_clipboard {
            CaptureDestination::ClipboardAndFile
        } else {
            CaptureDestination::FileOnly
        };

        let (capture_type, destination) = match action {
            Action::CaptureFullScreen => (CaptureType::FullScreen, default_destination),
            Action::CaptureActiveWindow => (CaptureType::ActiveWindow, default_destination),
            Action::CaptureSelection => (
                CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                default_destination,
            ),
            Action::CaptureClipboardFull => {
                (CaptureType::FullScreen, CaptureDestination::ClipboardOnly)
            }
            Action::CaptureFileFull => (CaptureType::FullScreen, CaptureDestination::FileOnly),
            Action::CaptureClipboardSelection | Action::CaptureClipboardRegion => (
                CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                CaptureDestination::ClipboardOnly,
            ),
            Action::CaptureFileSelection | Action::CaptureFileRegion => (
                CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                CaptureDestination::FileOnly,
            ),
            other => {
                error!(
                    "Non-capture action passed to GTK capture handler: {:?}",
                    other
                );
                return;
            }
        };

        let save_config = if matches!(destination, CaptureDestination::ClipboardOnly) {
            None
        } else {
            Some(FileSaveConfig {
                save_directory: expand_tilde(&state_mut.config.capture.save_directory),
                filename_template: state_mut.config.capture.filename_template.clone(),
                format: state_mut.config.capture.format.clone(),
            })
        };

        hide_overlay_for_capture(&mut state_mut);
        state_mut.capture_in_progress = true;

        info!("Requesting {:?} capture", capture_type);
        if let Err(err) =
            state_mut
                .capture_manager
                .request_capture(capture_type, destination, save_config)
        {
            error!("Failed to request capture: {}", err);
            state_mut.capture_in_progress = false;
            show_overlay_window(&mut state_mut);
            return;
        }
    }

    start_capture_poll(state);
}

fn friendly_capture_error(error: &str) -> String {
    let lower = error.to_lowercase();

    if lower.contains("requestcancelled") || lower.contains("cancelled") {
        "Screen capture cancelled by user".to_string()
    } else if lower.contains("permission") {
        "Permission denied. Enable screen sharing in system settings.".to_string()
    } else if lower.contains("busy") {
        "Screen capture in progress. Try again in a moment.".to_string()
    } else {
        "Screen capture failed. Please try again.".to_string()
    }
}

fn start_capture_poll(state: &Rc<RefCell<GtkState>>) {
    let mut state_mut = state.borrow_mut();
    if state_mut.capture_poll.is_some() {
        return;
    }

    let state_weak = Rc::downgrade(state);
    let source_id = glib::timeout_add_local(Duration::from_millis(50), move || {
        if let Some(state_rc) = state_weak.upgrade() {
            let mut state = state_rc.borrow_mut();
            if let Some(outcome) = state.capture_manager.try_take_result() {
                state.capture_in_progress = false;
                state.capture_poll = None;
                debug!(
                    "GTK capture completed; desired_visible={}, overlay_state will refresh",
                    state.desired_visible
                );
                handle_capture_outcome(&mut state, outcome);
                return ControlFlow::Break;
            }
            ControlFlow::Continue
        } else {
            ControlFlow::Break
        }
    });

    state_mut.capture_poll = Some(source_id);
}

fn handle_capture_outcome(state: &mut GtkState, outcome: CaptureOutcome) {
    if state.desired_visible {
        show_overlay_window(state);
    }

    match outcome {
        CaptureOutcome::Success(result) => {
            let mut parts = Vec::new();
            if let Some(ref path) = result.saved_path {
                info!("Screenshot saved to: {}", path.display());
                if let Some(filename) = path.file_name() {
                    parts.push(format!("Saved as {}", filename.to_string_lossy()));
                }
            }
            if result.copied_to_clipboard {
                info!("Screenshot copied to clipboard");
                parts.push("Copied to clipboard".to_string());
            }

            let body = if parts.is_empty() {
                "Screenshot captured".to_string()
            } else {
                parts.join(" • ")
            };

            notification::send_notification_async(
                &state.tokio_handle,
                "Screenshot Captured".to_string(),
                body,
                Some("camera-photo".to_string()),
            );
        }
        CaptureOutcome::Failed(err) => {
            let friendly = friendly_capture_error(&err);
            warn!("Screenshot capture failed: {}", err);
            notification::send_notification_async(
                &state.tokio_handle,
                "Screenshot Failed".to_string(),
                friendly,
                Some("dialog-error".to_string()),
            );
        }
        CaptureOutcome::Cancelled(reason) => {
            info!("Capture cancelled: {}", reason);
        }
    }
}

mod state_helpers {
    use crate::input::{BoardMode, InputState};

    pub fn force_whiteboard(state: &mut InputState) {
        if state.board_mode() != BoardMode::Whiteboard {
            state.canvas_set.switch_mode(BoardMode::Whiteboard);
            state.needs_redraw = true;
        }
    }
}

enum GtkCommand {
    Show,
    Hide,
    Quit,
}

pub struct GtkDaemonController {
    sender: mpsc::Sender<GtkCommand>,
    handle: Option<thread::JoinHandle<Result<()>>>,
}

impl GtkDaemonController {
    pub fn start(initial_mode: Option<String>) -> Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();

        let handle = thread::Builder::new()
            .name("wayscriber-gtk-daemon".into())
            .spawn(move || -> Result<()> {
                let mut backend = Gtk4Backend::new(initial_mode)?;
                backend.init()?;
                backend.run_daemon_loop(cmd_rx, ready_tx)?;
                Ok(())
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                let _ = handle.join();
                return Err(err);
            }
            Err(_) => {
                let _ = handle.join();
                return Err(anyhow!("GTK daemon backend failed to start"));
            }
        }

        Ok(Self {
            sender: cmd_tx,
            handle: Some(handle),
        })
    }

    pub fn show(&self) -> Result<()> {
        self.sender
            .send(GtkCommand::Show)
            .map_err(|_| anyhow!("GTK backend command channel closed"))
    }

    pub fn hide(&self) -> Result<()> {
        self.sender
            .send(GtkCommand::Hide)
            .map_err(|_| anyhow!("GTK backend command channel closed"))
    }

    pub fn shutdown(mut self) -> Result<()> {
        let _ = self.sender.send(GtkCommand::Quit);
        if let Some(handle) = self.handle.take() {
            match handle.join() {
                Ok(result) => result,
                Err(err) => Err(anyhow!("GTK controller thread panicked: {:?}", err)),
            }
        } else {
            Ok(())
        }
    }
}

fn register_input_handlers(
    window: &gtk4::ApplicationWindow,
    drawing_area: &gtk4::DrawingArea,
    state: Rc<RefCell<GtkState>>,
) {
    let motion_state = state.clone();
    let motion = gtk4::EventControllerMotion::new();
    motion.connect_motion(move |_, x, y| {
        let mut state = motion_state.borrow_mut();
        let mouse_x = x.round() as i32;
        let mouse_y = y.round() as i32;
        state.current_mouse_x = mouse_x;
        state.current_mouse_y = mouse_y;
        state.input_state.on_mouse_motion(mouse_x, mouse_y);
        state.input_state.needs_redraw = true;
    });
    drawing_area.add_controller(motion);

    let gesture = gtk4::GestureClick::new();
    gesture.set_button(0); // listen to all buttons
    let press_state = state.clone();
    gesture.connect_pressed(move |gesture, _n_press, x, y| {
        let button = gesture.current_button() as i32;
        if let Some(mapped) = map_mouse_button(button) {
            let mut state = press_state.borrow_mut();
            let mouse_x = x.round() as i32;
            let mouse_y = y.round() as i32;
            state.current_mouse_x = mouse_x;
            state.current_mouse_y = mouse_y;
            state.input_state.on_mouse_press(mapped, mouse_x, mouse_y);
            state.input_state.needs_redraw = true;
        }
    });
    let release_state = state.clone();
    gesture.connect_released(move |gesture, _n_press, x, y| {
        let button = gesture.current_button() as i32;
        if let Some(mapped) = map_mouse_button(button) {
            let mut state = release_state.borrow_mut();
            let mouse_x = x.round() as i32;
            let mouse_y = y.round() as i32;
            state.current_mouse_x = mouse_x;
            state.current_mouse_y = mouse_y;
            state.input_state.on_mouse_release(mapped, mouse_x, mouse_y);
            state.input_state.needs_redraw = true;
        }
    });
    drawing_area.add_controller(gesture);

    let scroll_state = state.clone();
    let scroll = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
    scroll.connect_scroll(move |_, _dx, dy| {
        let mut state = scroll_state.borrow_mut();
        let vertical = if dy.abs() > 0.1 { dy } else { 0.0 };
        if state.input_state.modifiers.shift {
            if vertical > 0.0 {
                state.input_state.adjust_font_size(-2.0);
            } else if vertical < 0.0 {
                state.input_state.adjust_font_size(2.0);
            }
        } else if vertical > 0.0 {
            state.input_state.current_thickness =
                (state.input_state.current_thickness - 1.0).max(1.0);
        } else if vertical < 0.0 {
            state.input_state.current_thickness =
                (state.input_state.current_thickness + 1.0).min(20.0);
        }
        state.input_state.needs_redraw = true;
        Propagation::Stop
    });
    drawing_area.add_controller(scroll);

    let key_controller = gtk4::EventControllerKey::new();
    let key_state = state.clone();
    key_controller.connect_key_pressed(move |_, key, _code, _mods| {
        let mapped = map_key(&key);
        let mut state = key_state.borrow_mut();
        state.input_state.on_key_press(mapped);
        state.input_state.needs_redraw = true;
        Propagation::Stop
    });
    let key_release_state = state;
    key_controller.connect_key_released(move |_, key, _code, _mods| {
        let mapped = map_key(&key);
        let mut state = key_release_state.borrow_mut();
        state.input_state.on_key_release(mapped);
    });
    window.add_controller(key_controller);
}

fn render_canvas(
    state: &Rc<RefCell<GtkState>>,
    ctx: &gtk4::cairo::Context,
    width: i32,
    height: i32,
) -> Result<()> {
    let mut state = state.borrow_mut();

    let width_u32 = width.max(1) as u32;
    let height_u32 = height.max(1) as u32;

    let raw_ptr = ctx.to_raw_none() as *mut cairo::ffi::cairo_t;
    let cairo_ctx = unsafe { cairo::Context::from_raw_none(raw_ptr) };

    cairo_ctx.set_operator(cairo::Operator::Clear);
    cairo_ctx.paint().context("Failed to clear GTK surface")?;
    cairo_ctx.set_operator(cairo::Operator::Over);

    debug!(
        "GTK render: mode={:?} size={}x{} desired_visible={} capture_in_progress={}",
        state.input_state.board_mode(),
        width_u32,
        height_u32,
        state.desired_visible,
        state.capture_in_progress
    );

    let now = Instant::now();
    let highlight_active = state.input_state.advance_click_highlights(now);
    if highlight_active {
        state.input_state.needs_redraw = true;
    }

    crate::draw::render_board_background(
        &cairo_ctx,
        state.input_state.board_mode(),
        &state.input_state.board_config,
    );

    crate::draw::render_shapes(
        &cairo_ctx,
        &state.input_state.canvas_set.active_frame().shapes,
    );

    state.input_state.render_provisional_shape(
        &cairo_ctx,
        state.current_mouse_x,
        state.current_mouse_y,
    );

    if let crate::input::state::DrawingState::TextInput { x, y, buffer } = &state.input_state.state
    {
        let preview_text = if buffer.is_empty() {
            "_".to_string()
        } else {
            format!("{}_", buffer)
        };
        crate::draw::render_text(
            &cairo_ctx,
            *x,
            *y,
            &preview_text,
            state.input_state.current_color,
            state.input_state.current_font_size,
            &state.input_state.font_descriptor,
            state.input_state.text_background_enabled,
        );
    }

    state.input_state.render_click_highlights(&cairo_ctx, now);

    if state.input_state.show_status_bar {
        crate::ui::render_status_bar(
            &cairo_ctx,
            &state.input_state,
            state.config.ui.status_bar_position,
            &state.config.ui.status_bar_style,
            width_u32,
            height_u32,
        );
    }

    if state.input_state.show_help {
        crate::ui::render_help_overlay(
            &cairo_ctx,
            &state.config.ui.help_overlay_style,
            width_u32,
            height_u32,
        );
    }

    state.input_state.needs_redraw = false;

    Ok(())
}

fn map_mouse_button(button: i32) -> Option<MouseButton> {
    match button {
        1 => Some(MouseButton::Left),
        2 => Some(MouseButton::Middle),
        3 => Some(MouseButton::Right),
        _ => None,
    }
}

fn map_key(key: &gdk::Key) -> Key {
    if let Some(name) = key.name() {
        match name.as_str() {
            "Escape" => return Key::Escape,
            "Return" => return Key::Return,
            "BackSpace" => return Key::Backspace,
            "Tab" => return Key::Tab,
            "space" => return Key::Space,
            "Shift_L" | "Shift_R" => return Key::Shift,
            "Control_L" | "Control_R" => return Key::Ctrl,
            "Alt_L" | "Alt_R" | "Meta_L" | "Meta_R" => return Key::Alt,
            "F10" => return Key::F10,
            "F11" => return Key::F11,
            "F12" => return Key::F12,
            "plus" => return Key::Char('+'),
            "minus" => return Key::Char('-'),
            "equal" => return Key::Char('='),
            "underscore" => return Key::Char('_'),
            _ => {}
        }
    }

    key.to_unicode()
        .filter(|c| !c.is_control())
        .map(Key::Char)
        .unwrap_or(Key::Unknown)
}
