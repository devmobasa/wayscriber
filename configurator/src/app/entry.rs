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
