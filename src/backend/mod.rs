use anyhow::Result;
#[cfg(not(feature = "gtk-backend"))]
use anyhow::anyhow;
use log::{debug, warn};
use std::env;

pub(crate) mod common;
#[cfg(feature = "gtk-backend")]
pub mod gtk4;
pub mod wayland;

/// Trait implemented by compositor backends (wlr-layer-shell, GTK4, …).
pub trait Backend {
    fn init(&mut self) -> Result<()>;
    fn show(&mut self) -> Result<()>;
    fn hide(&mut self) -> Result<()>;
    #[allow(dead_code)]
    fn is_visible(&self) -> bool;
}

/// Known backend implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    WlrLayerShell,
    Gtk4,
}

impl BackendKind {
    /// Normalised keyword used for CLI/env selection.
    pub fn keyword(self) -> &'static str {
        match self {
            BackendKind::WlrLayerShell => "wlr-layer-shell",
            BackendKind::Gtk4 => "gtk4",
        }
    }

    /// Parse a backend keyword into a [`BackendKind`].
    pub fn from_keyword(keyword: &str) -> Option<Self> {
        let lower = keyword.trim().to_ascii_lowercase();
        match lower.as_str() {
            "wlr" | "wlr-layer-shell" | "layer-shell" => Some(BackendKind::WlrLayerShell),
            "gtk4" | "gtk-4" => Some(BackendKind::Gtk4),
            _ => None,
        }
    }
}

/// Helper to drive a backend through its init/show/hide cycle.
pub fn run_backend<B: Backend + ?Sized>(backend: &mut B) -> Result<()> {
    backend.init()?;
    backend.show()?;
    backend.hide()?;
    Ok(())
}

/// Create a backend instance for the requested kind.
pub fn create_backend(kind: BackendKind, initial_mode: Option<String>) -> Result<Box<dyn Backend>> {
    match kind {
        BackendKind::WlrLayerShell => {
            let backend = wayland::WlrLayerShellBackend::new(initial_mode)?;
            Ok(Box::new(backend))
        }
        BackendKind::Gtk4 => {
            #[cfg(feature = "gtk-backend")]
            {
                let backend = gtk4::Gtk4Backend::new(initial_mode)?;
                Ok(Box::new(backend))
            }
            #[cfg(not(feature = "gtk-backend"))]
            {
                Err(anyhow!(
                    "This binary was built without the `gtk-backend` feature; rebuild with \
                     `--features gtk-backend` to enable the GTK4 backend."
                ))
            }
        }
    }
}

/// Detect the preferred backend for the current session.
///
/// Placeholder: will grow compositor heuristics in follow-up steps.
pub fn detect_backend() -> BackendKind {
    if is_gnome_session() {
        if cfg!(feature = "gtk-backend") {
            debug!("Detected GNOME desktop session; selecting GTK4 backend");
            BackendKind::Gtk4
        } else {
            warn!(
                "Detected GNOME desktop session but GTK backend was not enabled at build time; \
                 falling back to layer-shell backend"
            );
            BackendKind::WlrLayerShell
        }
    } else {
        BackendKind::WlrLayerShell
    }
}

#[allow(dead_code)]
pub fn run_wayland(initial_mode: Option<String>) -> Result<()> {
    let mut backend = wayland::WlrLayerShellBackend::new(initial_mode)?;
    run_backend(&mut backend)
}

#[cfg(test)]
mod tests {
    use super::{BackendKind, run_backend};

    #[test]
    #[ignore]
    fn wayland_backend_smoke_test() {
        if std::env::var("WAYLAND_DISPLAY").is_err() {
            eprintln!("WAYLAND_DISPLAY not set; skipping Wayland smoke test");
            return;
        }
        let mut backend = match super::create_backend(BackendKind::WlrLayerShell, None) {
            Ok(backend) => backend,
            Err(err) => panic!("Failed to create backend: {err}"),
        };
        run_backend(backend.as_mut()).expect("Wayland backend should start");
    }
}

fn is_gnome_session() -> bool {
    env_var_contains("XDG_CURRENT_DESKTOP", "gnome")
        || env_var_contains("DESKTOP_SESSION", "gnome")
        || env_var_contains("GDMSESSION", "gnome")
        || env::var("GNOME_DESKTOP_SESSION_ID").is_ok()
}

fn env_var_contains(var: &str, needle: &str) -> bool {
    match env::var(var) {
        Ok(value) => {
            let lower = value.to_ascii_lowercase();
            if lower.contains(needle) {
                return true;
            }
            value
                .split(|c| c == ':' || c == ';' || c == ',')
                .any(|part| part.trim().eq_ignore_ascii_case(needle))
        }
        Err(_) => false,
    }
}
