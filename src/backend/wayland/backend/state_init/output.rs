use log::{info, warn};
use std::env;

use crate::config::Config;

pub(super) struct OutputPreferences {
    pub(super) preferred_output_identity: Option<String>,
    pub(super) xdg_fullscreen: bool,
    pub(super) main_surface_uses_overlay_layer: bool,
}

pub(super) fn resolve(config: &Config) -> OutputPreferences {
    let preferred_output_identity = env::var("WAYSCRIBER_XDG_OUTPUT")
        .ok()
        .or_else(|| config.ui.preferred_output.clone());
    if let Some(ref output) = preferred_output_identity {
        info!(
            "Preferring xdg fullscreen on output '{}' (env or config override)",
            output
        );
    }

    let mut xdg_fullscreen = env::var("WAYSCRIBER_XDG_FULLSCREEN")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(config.ui.xdg_fullscreen);
    let desktop_env = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session_env = env::var("XDG_SESSION_DESKTOP").unwrap_or_default();
    let desktop_session = env::var("DESKTOP_SESSION").unwrap_or_default();
    let force_fullscreen = env::var("WAYSCRIBER_XDG_FULLSCREEN_FORCE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if xdg_fullscreen && desktop_env.to_uppercase().contains("GNOME") && !force_fullscreen {
        warn!(
            "GNOME fullscreen xdg fallback is opaque; falling back to maximized. Set WAYSCRIBER_XDG_FULLSCREEN_FORCE=1 to force fullscreen anyway."
        );
        xdg_fullscreen = false;
    }
    let main_surface_uses_overlay_layer =
        main_surface_uses_overlay_layer_with_env(&desktop_env, &session_env, &desktop_session);
    if main_surface_uses_overlay_layer {
        info!(
            "Niri detected; mapping the main overlay surface in the overlay layer so fullscreen windows cannot cover Wayscriber"
        );
    }

    OutputPreferences {
        preferred_output_identity,
        xdg_fullscreen,
        main_surface_uses_overlay_layer,
    }
}

fn main_surface_uses_overlay_layer_with_env(
    desktop_env: &str,
    session_env: &str,
    desktop_session: &str,
) -> bool {
    desktop_matches(desktop_env, "niri")
        || desktop_matches(session_env, "niri")
        || desktop_matches(desktop_session, "niri")
}

fn desktop_matches(value: &str, target: &str) -> bool {
    value
        .split(':')
        .map(str::trim)
        .any(|entry| entry.eq_ignore_ascii_case(target))
}

#[cfg(test)]
mod tests {
    use super::main_surface_uses_overlay_layer_with_env;

    #[test]
    fn main_surface_uses_overlay_layer_for_niri_desktop() {
        assert!(main_surface_uses_overlay_layer_with_env("niri", "", ""));
        assert!(main_surface_uses_overlay_layer_with_env(
            "Hyprland:Niri",
            "",
            ""
        ));
    }

    #[test]
    fn main_surface_uses_overlay_layer_for_niri_session() {
        assert!(main_surface_uses_overlay_layer_with_env("", "NIRI", ""));
    }

    #[test]
    fn main_surface_uses_overlay_layer_for_niri_desktop_session() {
        assert!(main_surface_uses_overlay_layer_with_env("", "", "niri"));
    }

    #[test]
    fn main_surface_stays_on_top_layer_for_other_desktops() {
        assert!(!main_surface_uses_overlay_layer_with_env(
            "Hyprland", "", ""
        ));
        assert!(!main_surface_uses_overlay_layer_with_env(
            "KDE", "plasma", ""
        ));
    }
}
