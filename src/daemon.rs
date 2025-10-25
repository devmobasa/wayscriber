/// Daemon mode implementation: background service with toggle activation
use anyhow::{Context, Result};
use ksni::TrayMethods;
use log::{debug, error, info, warn};
use signal_hook::consts::signal::{SIGINT, SIGTERM, SIGUSR1};
use signal_hook::iterator::Signals;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::backend;
use crate::legacy;

/// Overlay state for daemon mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Hidden,  // Daemon running, overlay not visible
    Visible, // Overlay active, capturing input
}

/// Daemon state manager
type BackendRunner = dyn Fn(Option<String>) -> Result<()> + Send + Sync;

pub struct Daemon {
    overlay_state: OverlayState,
    should_quit: Arc<AtomicBool>,
    toggle_requested: Arc<AtomicBool>,
    initial_mode: Option<String>,
    backend_runner: Arc<BackendRunner>,
}

pub(crate) struct WayscriberTray {
    toggle_flag: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
    configurator_binary: String,
}

impl WayscriberTray {
    fn new(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        configurator_binary: String,
    ) -> Self {
        Self {
            toggle_flag,
            quit_flag,
            configurator_binary,
        }
    }

    #[cfg(test)]
    fn new_for_tests(toggle_flag: Arc<AtomicBool>, quit_flag: Arc<AtomicBool>) -> Self {
        Self::new(toggle_flag, quit_flag, "true".into())
    }
}

impl WayscriberTray {
    fn launch_configurator(&self) {
        let mut command = Command::new(&self.configurator_binary);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(child) => {
                info!(
                    "Launched wayscriber-configurator (binary: {}, pid: {})",
                    self.configurator_binary,
                    child.id()
                );
            }
            Err(err) => {
                error!(
                    "Failed to launch wayscriber-configurator using '{}': {}",
                    self.configurator_binary, err
                );
                error!(
                    "Set WAYSCRIBER_CONFIGURATOR (or legacy HYPRMARKER_CONFIGURATOR) to override the executable path if needed."
                );
            }
        }
    }
}

impl ksni::Tray for WayscriberTray {
    fn id(&self) -> String {
        "wayscriber".into()
    }

    fn title(&self) -> String {
        "Wayscriber Screen Annotation".into()
    }

    fn icon_name(&self) -> String {
        "applications-graphics".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: "applications-graphics".into(),
            icon_pixmap: vec![],
            title: format!("Wayscriber {}", env!("CARGO_PKG_VERSION")),
            description: "Super+D toggles overlay • F11 opens configurator".into(),
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let size = 22;
        let mut data = Vec::with_capacity(size * size * 4);

        for y in 0..size {
            for x in 0..size {
                let (a, r, g, b) = if (2..=4).contains(&x) && (2..=4).contains(&y) {
                    (255, 60, 60, 60)
                } else if (3..=5).contains(&x) && (5..=7).contains(&y) {
                    (255, 180, 120, 60)
                } else if (4..=8).contains(&x) && (6..=14).contains(&y) {
                    (255, 255, 220, 0)
                } else if (7..=9).contains(&x) && (13..=17).contains(&y) {
                    (255, 180, 180, 180)
                } else if (8..=11).contains(&x) && (16..=19).contains(&y) {
                    (255, 255, 150, 180)
                } else {
                    (0, 0, 0, 0)
                };

                data.push(a);
                data.push(r);
                data.push(g);
                data.push(b);
            }
        }

        vec![ksni::Icon {
            width: size as i32,
            height: size as i32,
            data,
        }]
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        vec![
            StandardItem {
                label: "Toggle Overlay (Super+D)".to_string(),
                icon_name: "tool-pointer".into(),
                activate: Box::new(|this: &mut Self| {
                    this.toggle_flag.store(true, Ordering::Release);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Configurator".to_string(),
                icon_name: "preferences-desktop".into(),
                activate: Box::new(|this: &mut Self| {
                    this.launch_configurator();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                icon_name: "window-close".into(),
                activate: Box::new(|this: &mut Self| {
                    this.quit_flag.store(true, Ordering::Release);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

impl Daemon {
    pub fn new(initial_mode: Option<String>) -> Self {
        Self::with_backend_runner_internal(
            initial_mode,
            Arc::new(|mode| backend::run_wayland(mode)),
        )
    }

    fn with_backend_runner_internal(
        initial_mode: Option<String>,
        backend_runner: Arc<BackendRunner>,
    ) -> Self {
        Self {
            overlay_state: OverlayState::Hidden,
            should_quit: Arc::new(AtomicBool::new(false)),
            toggle_requested: Arc::new(AtomicBool::new(false)),
            initial_mode,
            backend_runner,
        }
    }

    #[cfg(test)]
    pub fn with_backend_runner(
        initial_mode: Option<String>,
        backend_runner: Arc<BackendRunner>,
    ) -> Self {
        Self::with_backend_runner_internal(initial_mode, backend_runner)
    }

    /// Run daemon with signal handling
    pub fn run(&mut self) -> Result<()> {
        info!("Starting wayscriber daemon");
        info!("Send SIGUSR1 to toggle overlay (e.g., pkill -SIGUSR1 wayscriber)");
        info!("Configure Hyprland: bind = SUPER, D, exec, pkill -SIGUSR1 wayscriber");

        // Set up signal handling
        let mut signals = Signals::new([SIGUSR1, SIGTERM, SIGINT])
            .context("Failed to register signal handler")?;

        let toggle_flag = self.toggle_requested.clone();
        let quit_flag = self.should_quit.clone();

        // Spawn signal handler thread
        // Note: This thread will run until process termination. The signal_hook iterator
        // doesn't provide a clean shutdown mechanism with forever(), but this is acceptable
        // for a daemon process as the thread has no resources requiring explicit cleanup.
        // The thread will be terminated by the OS when the process exits.
        thread::spawn(move || {
            for sig in signals.forever() {
                match sig {
                    SIGUSR1 => {
                        info!("Received SIGUSR1 - toggling overlay");
                        // Use Release ordering to ensure all prior memory operations
                        // are visible to the thread that reads this flag
                        toggle_flag.store(true, Ordering::Release);
                    }
                    SIGTERM | SIGINT => {
                        info!(
                            "Received {} - initiating graceful shutdown",
                            if sig == SIGTERM { "SIGTERM" } else { "SIGINT" }
                        );
                        // Use Release ordering to ensure all prior memory operations
                        // are visible to the thread that reads this flag
                        quit_flag.store(true, Ordering::Release);
                    }
                    _ => {
                        warn!("Received unexpected signal: {}", sig);
                    }
                }
            }
        });

        // Start system tray
        let tray_toggle = self.toggle_requested.clone();
        let tray_quit = self.should_quit.clone();
        thread::spawn(move || {
            if let Err(e) = run_system_tray(tray_toggle, tray_quit) {
                warn!("System tray failed: {}", e);
            }
        });

        info!("Daemon ready - waiting for toggle signal");

        // Main daemon loop
        loop {
            // Check for quit signal
            // Use Acquire ordering to ensure we see all memory operations
            // that happened before the flag was set
            if self.should_quit.load(Ordering::Acquire) {
                info!("Quit signal received - exiting daemon");
                break;
            }

            // Check for toggle request
            // Use Acquire ordering to ensure we see all memory operations
            // that happened before the flag was set
            if self.toggle_requested.swap(false, Ordering::Acquire) {
                self.toggle_overlay()?;
            }

            // Small sleep to avoid busy-waiting
            thread::sleep(Duration::from_millis(100));
        }

        info!("Daemon shutting down");
        Ok(())
    }

    /// Toggle overlay visibility
    fn toggle_overlay(&mut self) -> Result<()> {
        match self.overlay_state {
            OverlayState::Hidden => {
                info!("Showing overlay");
                self.show_overlay()?;
            }
            OverlayState::Visible => {
                info!("Hiding overlay");
                self.hide_overlay()?;
            }
        }
        Ok(())
    }

    /// Show overlay (create layer surface and enter drawing mode)
    fn show_overlay(&mut self) -> Result<()> {
        if self.overlay_state == OverlayState::Visible {
            debug!("Overlay already visible");
            return Ok(());
        }

        // Set state to visible before running
        self.overlay_state = OverlayState::Visible;
        info!("Overlay state set to Visible");

        // Run the Wayland backend (this will block until overlay is closed)
        let result = (self.backend_runner)(self.initial_mode.clone());

        // When run_wayland returns, the overlay was closed
        self.overlay_state = OverlayState::Hidden;
        info!("Overlay closed, back to daemon mode");

        result
    }

    /// Hide overlay (destroy layer surface, return to hidden state)
    fn hide_overlay(&mut self) -> Result<()> {
        if self.overlay_state == OverlayState::Hidden {
            debug!("Overlay already hidden");
            return Ok(());
        }

        // NOTE: The overlay will be closed when user presses Escape
        // or when the backend exits naturally
        self.overlay_state = OverlayState::Hidden;
        Ok(())
    }
}

#[cfg(test)]
impl Daemon {
    pub fn test_state(&self) -> OverlayState {
        self.overlay_state
    }
}

/// System tray implementation
fn run_system_tray(toggle_flag: Arc<AtomicBool>, quit_flag: Arc<AtomicBool>) -> Result<()> {
    let configurator_binary =
        legacy::configurator_override().unwrap_or_else(|| "wayscriber-configurator".to_string());

    let tray = WayscriberTray::new(toggle_flag, quit_flag.clone(), configurator_binary);

    info!("Creating tray service...");

    // ksni 0.3+ uses async API - spawn service in background tokio runtime
    // Note: This thread will be terminated when the main daemon loop exits.
    // The tray library handles its own cleanup when dropped.
    let _tray_thread = std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(e) => {
                warn!("Failed to create Tokio runtime for system tray: {}", e);
                return;
            }
        };

        rt.block_on(async {
            match tray.spawn().await {
                Ok(handle) => {
                    info!("System tray spawned successfully");

                    // Monitor quit flag and shutdown gracefully
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        if quit_flag.load(Ordering::Acquire) {
                            info!("Quit signal received - shutting down system tray");
                            // Shutdown awaiter will clean up the tray service
                            let _ = handle.shutdown().await;
                            break;
                        }
                    }
                }
                Err(e) => {
                    warn!("System tray error: {}", e);
                }
            }
        });
    });

    info!("System tray thread started");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksni::{Tray, menu::MenuItem};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    fn runner_counter(count: Arc<AtomicUsize>) -> Arc<BackendRunner> {
        Arc::new(move |mode: Option<String>| -> Result<()> {
            assert_eq!(mode.as_deref(), Some("whiteboard"));
            count.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        })
    }

    #[test]
    fn toggle_overlay_invokes_backend_when_hidden() {
        let counter = Arc::new(AtomicUsize::new(0));
        let runner = runner_counter(counter.clone());
        let mut daemon = Daemon::with_backend_runner(Some("whiteboard".into()), runner);

        daemon.toggle_overlay().unwrap();
        assert_eq!(counter.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(daemon.test_state(), OverlayState::Hidden);
    }

    #[test]
    fn hide_overlay_is_idempotent() {
        let runner = Arc::new(|_: Option<String>| Ok(())) as Arc<BackendRunner>;
        let mut daemon = Daemon::with_backend_runner(None, runner);
        daemon.hide_overlay().unwrap();
        assert_eq!(daemon.test_state(), OverlayState::Hidden);

        daemon.overlay_state = OverlayState::Visible;
        daemon.toggle_overlay().unwrap();
        assert_eq!(daemon.test_state(), OverlayState::Hidden);
    }

    fn activate_menu_item(tray: &mut WayscriberTray, label: &str) {
        for item in tray.menu() {
            if let MenuItem::Standard(standard) = item {
                if standard.label.contains(label) {
                    let activate = standard.activate;
                    activate(tray);
                    return;
                }
            }
        }
        panic!("Menu item '{label}' not found");
    }

    #[test]
    fn tray_toggle_action_sets_flag() {
        let toggle = Arc::new(AtomicBool::new(false));
        let quit = Arc::new(AtomicBool::new(false));
        let mut tray = WayscriberTray::new_for_tests(toggle.clone(), quit);

        activate_menu_item(&mut tray, "Toggle Overlay");
        assert!(toggle.load(Ordering::SeqCst));
    }

    #[test]
    fn tray_quit_action_sets_quit_flag() {
        let toggle = Arc::new(AtomicBool::new(false));
        let quit = Arc::new(AtomicBool::new(false));
        let mut tray = WayscriberTray::new_for_tests(toggle, quit.clone());

        activate_menu_item(&mut tray, "Quit");
        assert!(quit.load(Ordering::SeqCst));
    }
}
