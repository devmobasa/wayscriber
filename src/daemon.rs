/// Daemon mode implementation: background service with toggle activation
use anyhow::{Context, Result, anyhow};
use ksni::TrayMethods;
use log::{debug, error, info, warn};
use signal_hook::consts::signal::{SIGINT, SIGTERM, SIGUSR1};
use signal_hook::iterator::Signals;
use std::env;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[cfg(unix)]
use libc;

use crate::backend;
use crate::legacy;

/// Overlay state for daemon mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Hidden,  // Daemon running, overlay not visible
    Visible, // Overlay active, capturing input
}

/// Daemon state manager
type BackendRunner = dyn Fn(backend::BackendKind, Option<String>) -> Result<()> + Send + Sync;

const TRAY_START_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Daemon {
    overlay_state: OverlayState,
    should_quit: Arc<AtomicBool>,
    toggle_requested: Arc<AtomicBool>,
    initial_mode: Option<String>,
    backend_kind: backend::BackendKind,
    backend_runner: Option<Arc<BackendRunner>>,
    tray_thread: Option<JoinHandle<()>>,
    overlay_child: Option<Child>,
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
    pub fn new(initial_mode: Option<String>, backend_kind: backend::BackendKind) -> Self {
        Self {
            overlay_state: OverlayState::Hidden,
            should_quit: Arc::new(AtomicBool::new(false)),
            toggle_requested: Arc::new(AtomicBool::new(false)),
            initial_mode,
            backend_kind,
            backend_runner: None,
            tray_thread: None,
            overlay_child: None,
        }
    }

    #[cfg(test)]
    fn with_backend_runner_internal(
        initial_mode: Option<String>,
        backend_kind: backend::BackendKind,
        backend_runner: Arc<BackendRunner>,
    ) -> Self {
        Self {
            overlay_state: OverlayState::Hidden,
            should_quit: Arc::new(AtomicBool::new(false)),
            toggle_requested: Arc::new(AtomicBool::new(false)),
            initial_mode,
            backend_kind,
            backend_runner: Some(backend_runner),
            tray_thread: None,
            overlay_child: None,
        }
    }

    #[cfg(test)]
    pub fn with_backend_runner(
        initial_mode: Option<String>,
        backend_kind: backend::BackendKind,
        backend_runner: Arc<BackendRunner>,
    ) -> Self {
        Self::with_backend_runner_internal(initial_mode, backend_kind, backend_runner)
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
                if quit_flag.load(Ordering::Acquire) {
                    info!("Signal handler thread exiting");
                    break;
                }
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
        let tray_handle =
            start_system_tray(tray_toggle, tray_quit).context("Failed to start system tray")?;
        self.tray_thread = Some(tray_handle);

        info!("Daemon ready - waiting for toggle signal");

        // Main daemon loop
        loop {
            self.update_overlay_process_state()?;

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
        // Ensure overlay is stopped before exit
        if let Err(err) = self.hide_overlay() {
            warn!("Failed to hide overlay during shutdown: {}", err);
        }
        self.should_quit.store(true, Ordering::Release);
        if let Some(handle) = self.tray_thread.take() {
            match handle.join() {
                Ok(()) => info!("System tray thread joined"),
                Err(err) => warn!("System tray thread panicked: {:?}", err),
            }
        }
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

        if let Some(runner) = &self.backend_runner {
            self.overlay_state = OverlayState::Visible;
            info!("Overlay state set to Visible");
            let result = runner(self.backend_kind, self.initial_mode.clone());
            self.overlay_state = OverlayState::Hidden;
            info!("Overlay closed, back to daemon mode");
            return result;
        }

        self.spawn_overlay_process()?;
        Ok(())
    }

    /// Hide overlay (destroy layer surface, return to hidden state)
    fn hide_overlay(&mut self) -> Result<()> {
        if self.overlay_state == OverlayState::Hidden {
            debug!("Overlay already hidden");
            return Ok(());
        }

        if self.backend_runner.is_some() {
            // Internal runner does not keep additional state to tear down
            debug!("Internal backend runner hidden");
            self.overlay_state = OverlayState::Hidden;
            return Ok(());
        }

        self.terminate_overlay_process()?;
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
fn start_system_tray(
    toggle_flag: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
) -> Result<JoinHandle<()>> {
    let configurator_binary =
        legacy::configurator_override().unwrap_or_else(|| "wayscriber-configurator".to_string());

    let tray_quit_flag = quit_flag.clone();
    let tray = WayscriberTray::new(toggle_flag, tray_quit_flag.clone(), configurator_binary);
    let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();

    info!("Creating tray service...");
    info!("Spawning system tray runtime thread...");

    let ready_thread_tx = ready_tx.clone();
    let tray_thread = thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(e) => {
                warn!("Failed to create Tokio runtime for system tray: {}", e);
                report_tray_readiness(
                    &ready_thread_tx,
                    Err(anyhow!(
                        "Failed to create Tokio runtime for system tray: {e}"
                    )),
                );
                return;
            }
        };

        rt.block_on(async {
            match tray.spawn().await {
                Ok(handle) => {
                    info!("System tray spawned successfully");
                    report_tray_readiness(&ready_thread_tx, Ok(()));

                    // Monitor quit flag and shutdown gracefully
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        if tray_quit_flag.load(Ordering::Acquire) {
                            info!("Quit signal received - shutting down system tray");
                            let _ = handle.shutdown().await;
                            break;
                        }
                    }
                }
                Err(e) => {
                    warn!("System tray error: {}", e);
                    report_tray_readiness(&ready_thread_tx, Err(anyhow!("System tray error: {e}")));
                }
            }
        });
    });

    drop(ready_tx);

    info!("Waiting for system tray readiness signal...");
    match ready_rx.recv_timeout(TRAY_START_TIMEOUT) {
        Ok(result) => {
            result?;
            info!("System tray thread started");
            Ok(tray_thread)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            warn!("Timed out waiting for system tray to start");
            quit_flag.store(true, Ordering::Release);
            let _ = tray_thread.join();
            Err(anyhow!("Timed out waiting for system tray to start"))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = tray_thread.join();
            Err(anyhow!(
                "System tray thread exited before signaling readiness"
            ))
        }
    }
}

impl Daemon {
    fn spawn_overlay_process(&mut self) -> Result<()> {
        let exe = env::current_exe().context("Failed to determine current executable path")?;
        let mut command = Command::new(exe);
        command.arg("--active");
        command.arg("--backend").arg(self.backend_kind.keyword());
        if let Some(mode) = &self.initial_mode {
            command.arg("--mode").arg(mode);
        }
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());

        info!("Spawning overlay process...");
        let child = command.spawn().context("Failed to spawn overlay process")?;
        let pid = child.id();
        self.overlay_child = Some(child);
        self.overlay_state = OverlayState::Visible;
        info!("Overlay process started (pid {pid})");
        Ok(())
    }

    fn terminate_overlay_process(&mut self) -> Result<()> {
        if let Some(mut child) = self.overlay_child.take() {
            info!("Stopping overlay process (pid {})", child.id());
            #[cfg(unix)]
            {
                if unsafe { libc::kill(child.id() as i32, libc::SIGTERM) } != 0 {
                    warn!(
                        "Failed to signal overlay process: {}",
                        std::io::Error::last_os_error()
                    );
                }
            }
            #[cfg(not(unix))]
            {
                if let Err(err) = child.kill() {
                    warn!("Failed to signal overlay process: {}", err);
                }
            }

            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        info!("Overlay process exited with status {:?}", status);
                        break;
                    }
                    Ok(None) => {
                        if Instant::now() >= deadline {
                            warn!("Overlay process did not exit, sending SIGKILL");
                            let _ = child.kill();
                            let _ = child.wait();
                            break;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(err) => {
                        warn!("Failed to query overlay process status: {}", err);
                        let _ = child.kill();
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    fn update_overlay_process_state(&mut self) -> Result<()> {
        if self.backend_runner.is_some() {
            return Ok(());
        }

        if let Some(child) = self.overlay_child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    info!("Overlay process exited with status {:?}", status);
                    self.overlay_child = None;
                    self.overlay_state = OverlayState::Hidden;
                }
                Ok(None) => {}
                Err(err) => {
                    warn!("Failed to poll overlay process status: {}", err);
                }
            }
        }
        Ok(())
    }
}

fn report_tray_readiness(tx: &mpsc::Sender<Result<()>>, result: Result<()>) {
    if let Err(err) = tx.send(result) {
        debug!(
            "System tray readiness receiver dropped before signal could be delivered: {}",
            err
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksni::{Tray, menu::MenuItem};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    fn runner_counter(count: Arc<AtomicUsize>) -> Arc<BackendRunner> {
        Arc::new(
            move |backend_kind: backend::BackendKind, mode: Option<String>| -> Result<()> {
                assert_eq!(backend_kind, backend::BackendKind::WlrLayerShell);
                assert_eq!(mode.as_deref(), Some("whiteboard"));
                count.fetch_add(1, AtomicOrdering::SeqCst);
                Ok(())
            },
        )
    }

    #[test]
    fn toggle_overlay_invokes_backend_when_hidden() {
        let counter = Arc::new(AtomicUsize::new(0));
        let runner = runner_counter(counter.clone());
        let mut daemon = Daemon::with_backend_runner(
            Some("whiteboard".into()),
            backend::BackendKind::WlrLayerShell,
            runner,
        );

        daemon.toggle_overlay().unwrap();
        assert_eq!(counter.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(daemon.test_state(), OverlayState::Hidden);
    }

    #[test]
    fn hide_overlay_is_idempotent() {
        let runner = Arc::new(|_, _| Ok(())) as Arc<BackendRunner>;
        let mut daemon =
            Daemon::with_backend_runner(None, backend::BackendKind::WlrLayerShell, runner);
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
