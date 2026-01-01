use log::{info, warn};
use std::env;

use crate::config::Config;

pub(super) struct OutputPreferences {
    pub(super) preferred_output_identity: Option<String>,
    pub(super) xdg_fullscreen: bool,
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
    let force_fullscreen = env::var("WAYSCRIBER_XDG_FULLSCREEN_FORCE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if xdg_fullscreen && desktop_env.to_uppercase().contains("GNOME") && !force_fullscreen {
        warn!(
            "GNOME fullscreen xdg fallback is opaque; falling back to maximized. Set WAYSCRIBER_XDG_FULLSCREEN_FORCE=1 to force fullscreen anyway."
        );
        xdg_fullscreen = false;
    }

    OutputPreferences {
        preferred_output_identity,
        xdg_fullscreen,
    }
}
