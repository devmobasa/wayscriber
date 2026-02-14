use iced::{Application, Settings, Size};

use super::state::ConfiguratorApp;

pub fn run() -> iced::Result {
    let mut settings = Settings::default();
    settings.window.size = Size::new(960.0, 640.0);
    settings.window.resizable = true;
    settings.window.decorations = true;
    if std::env::var_os("ICED_BACKEND").is_none() && should_force_tiny_skia() {
        // GNOME Wayland + wgpu can crash on dma-buf/present mode selection; tiny-skia avoids this.
        // SAFETY: setting a process-local env var before initializing iced is safe here.
        unsafe {
            std::env::set_var("ICED_BACKEND", "tiny-skia");
        }
        eprintln!(
            "wayscriber-configurator: GNOME Wayland detected; using tiny-skia renderer (set ICED_BACKEND=wgpu to override)."
        );
    }
    ConfiguratorApp::run(settings)
}

fn should_force_tiny_skia() -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return false;
    }
    let current = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session = std::env::var("XDG_SESSION_DESKTOP").unwrap_or_default();
    let combined = format!("{current};{session}");
    let combined = combined.to_ascii_lowercase();
    combined.contains("gnome") || combined.contains("ubuntu")
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    fn env_mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn should_force_tiny_skia_requires_wayland_and_gnome_like_desktop() {
        let _guard = env_mutex().lock().unwrap();
        let original_wayland = std::env::var_os("WAYLAND_DISPLAY");
        let original_current = std::env::var_os("XDG_CURRENT_DESKTOP");
        let original_session = std::env::var_os("XDG_SESSION_DESKTOP");

        // SAFETY: serialized by env mutex in this test module.
        unsafe {
            std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
            std::env::set_var("XDG_CURRENT_DESKTOP", "GNOME");
            std::env::set_var("XDG_SESSION_DESKTOP", "");
        }
        assert!(should_force_tiny_skia());

        // SAFETY: serialized by env mutex in this test module.
        unsafe {
            std::env::set_var("XDG_CURRENT_DESKTOP", "KDE");
            std::env::set_var("XDG_SESSION_DESKTOP", "plasma");
        }
        assert!(!should_force_tiny_skia());

        // SAFETY: serialized by env mutex in this test module.
        unsafe {
            std::env::remove_var("WAYLAND_DISPLAY");
        }
        assert!(!should_force_tiny_skia());

        match original_wayland {
            Some(value) => unsafe { std::env::set_var("WAYLAND_DISPLAY", value) },
            None => unsafe { std::env::remove_var("WAYLAND_DISPLAY") },
        }
        match original_current {
            Some(value) => unsafe { std::env::set_var("XDG_CURRENT_DESKTOP", value) },
            None => unsafe { std::env::remove_var("XDG_CURRENT_DESKTOP") },
        }
        match original_session {
            Some(value) => unsafe { std::env::set_var("XDG_SESSION_DESKTOP", value) },
            None => unsafe { std::env::remove_var("XDG_SESSION_DESKTOP") },
        }
    }
}
