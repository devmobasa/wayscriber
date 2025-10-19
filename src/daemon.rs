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

/// Overlay state for daemon mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Hidden,  // Daemon running, overlay not visible
    Visible, // Overlay active, capturing input
}

/// Daemon state manager
pub struct Daemon {
    overlay_state: OverlayState,
    should_quit: Arc<AtomicBool>,
    toggle_requested: Arc<AtomicBool>,
    initial_mode: Option<String>,
}

impl Daemon {
    pub fn new(initial_mode: Option<String>) -> Self {
        Self {
            overlay_state: OverlayState::Hidden,
            should_quit: Arc::new(AtomicBool::new(false)),
            toggle_requested: Arc::new(AtomicBool::new(false)),
            initial_mode,
        }
    }

    /// Run daemon with signal handling
    pub fn run(&mut self) -> Result<()> {
        info!("Starting hyprmarker daemon");
        info!("Send SIGUSR1 to toggle overlay (e.g., pkill -SIGUSR1 hyprmarker)");
        info!("Configure Hyprland: bind = SUPER, D, exec, pkill -SIGUSR1 hyprmarker");

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
        let result = backend::run_wayland(self.initial_mode.clone());

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

/// System tray implementation
fn run_system_tray(toggle_flag: Arc<AtomicBool>, quit_flag: Arc<AtomicBool>) -> Result<()> {
    use ksni;

    struct HyprmarkerTray {
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        configurator_binary: String,
    }

    impl HyprmarkerTray {
        fn launch_configurator(&self) {
            let mut command = Command::new(&self.configurator_binary);
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());

            match command.spawn() {
                Ok(child) => {
                    info!(
                        "Launched hyprmarker-configurator (binary: {}, pid: {})",
                        self.configurator_binary,
                        child.id()
                    );
                }
                Err(err) => {
                    error!(
                        "Failed to launch hyprmarker-configurator using '{}': {}",
                        self.configurator_binary, err
                    );
                    error!(
                        "Set HYPRMARKER_CONFIGURATOR to override the executable path if needed."
                    );
                }
            }
        }
    }

    impl ksni::Tray for HyprmarkerTray {
        fn id(&self) -> String {
            "hyprmarker".into()
        }

        fn title(&self) -> String {
            "Hyprmarker Screen Annotation".into()
        }

        fn icon_name(&self) -> String {
            // Try common icon names - some systems might have these
            "applications-graphics".into()
        }

        fn tool_tip(&self) -> ksni::ToolTip {
            ksni::ToolTip {
                icon_name: "applications-graphics".into(),
                icon_pixmap: vec![],
                title: format!("Hyprmarker {}", env!("CARGO_PKG_VERSION")),
                description: "Super+D toggles overlay • F11 opens configurator".into(),
            }
        }

        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            // Create a simple, highly visible pencil icon
            // 22x22 pixels in ARGB32 format (4 bytes per pixel)
            let size = 22;
            let mut data = Vec::with_capacity(size * size * 4);

            for y in 0..size {
                for x in 0..size {
                    // Create a diagonal pencil shape pointing down-right
                    let (a, r, g, b) = if (2..=4).contains(&x) && (2..=4).contains(&y) {
                        // Pencil tip - dark gray/graphite
                        (255, 60, 60, 60)
                    } else if (3..=5).contains(&x) && (5..=7).contains(&y) {
                        // Wood section - brown
                        (255, 180, 120, 60)
                    } else if (4..=8).contains(&x) && (6..=14).contains(&y) {
                        // Main body - yellow pencil
                        (255, 255, 220, 0)
                    } else if (7..=9).contains(&x) && (13..=17).contains(&y) {
                        // Metal ferrule - silver
                        (255, 180, 180, 180)
                    } else if (8..=11).contains(&x) && (16..=19).contains(&y) {
                        // Eraser - pink
                        (255, 255, 150, 180)
                    } else {
                        // Transparent background
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
                    label: format!("Toggle Overlay (Super+D)"),
                    icon_name: "tool-pointer".into(),
                    activate: Box::new(|this: &mut Self| {
                        // Use Release ordering to ensure memory operations are visible
                        this.toggle_flag.store(true, Ordering::Release);
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: format!("Open Configurator"),
                    icon_name: "preferences-desktop".into(),
                    activate: Box::new(|this: &mut Self| {
                        this.launch_configurator();
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: format!("Quit"),
                    icon_name: "window-close".into(),
                    activate: Box::new(|this: &mut Self| {
                        // Use Release ordering to ensure memory operations are visible
                        this.quit_flag.store(true, Ordering::Release);
                    }),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }

    let configurator_binary = std::env::var("HYPRMARKER_CONFIGURATOR")
        .unwrap_or_else(|_| "hyprmarker-configurator".to_string());

    let tray = HyprmarkerTray {
        toggle_flag,
        quit_flag: quit_flag.clone(),
        configurator_binary,
    };

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
