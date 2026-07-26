//! The "Copy diagnostics" payload.
//!
//! Everything here is already visible to the user; the button exists so a bug
//! report carries the build, desktop, and feature information in one paste
//! instead of three follow-up questions.

use crate::env_vars::{
    DESKTOP_SESSION_ENV, WAYLAND_DISPLAY_ENV, XDG_CURRENT_DESKTOP_ENV, XDG_SESSION_DESKTOP_ENV,
};

/// Environment facts worth reporting, captured so the formatter stays pure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct DiagnosticsEnv {
    pub(super) version: String,
    pub(super) commit: String,
    pub(super) build_date: Option<String>,
    pub(super) install_source: Option<String>,
    pub(super) current_desktop: Option<String>,
    pub(super) session_desktop: Option<String>,
    pub(super) desktop_session: Option<String>,
    pub(super) wayland_display: Option<String>,
}

impl DiagnosticsEnv {
    fn capture() -> Self {
        Self {
            version: crate::build_info::version().to_string(),
            commit: crate::build_info::commit_hash().to_string(),
            build_date: crate::build_info::build_date().map(str::to_string),
            install_source: crate::build_info::install_source().map(str::to_string),
            current_desktop: env_value(XDG_CURRENT_DESKTOP_ENV),
            session_desktop: env_value(XDG_SESSION_DESKTOP_ENV),
            desktop_session: env_value(DESKTOP_SESSION_ENV),
            wayland_display: env_value(WAYLAND_DISPLAY_ENV),
        }
    }
}

fn env_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Diagnostics for this process.
pub(super) fn report() -> String {
    format_report(&DiagnosticsEnv::capture(), compiled_features())
}

/// Cargo features this binary was built with, which decide whether the tray,
/// portal capture, GTK toolbars, and tablet input exist at all.
fn compiled_features() -> Vec<&'static str> {
    let mut features = Vec::new();
    if cfg!(feature = "dbus") {
        features.push("dbus");
    }
    if cfg!(feature = "portal") {
        features.push("portal");
    }
    if cfg!(feature = "tray") {
        features.push("tray");
    }
    if cfg!(feature = "toolbar-gtk") {
        features.push("toolbar-gtk");
    }
    if cfg!(feature = "tablet-input") {
        features.push("tablet-input");
    }
    features
}

fn format_report(env: &DiagnosticsEnv, features: Vec<&'static str>) -> String {
    let mut lines = Vec::new();

    let mut build = format!("Wayscriber {}", env.version);
    let mut build_details = Vec::new();
    if env.commit != "unknown" && !env.commit.is_empty() {
        build_details.push(format!("commit {}", env.commit));
    }
    if let Some(date) = env.build_date.as_deref() {
        build_details.push(format!("built {date}"));
    }
    if !build_details.is_empty() {
        build.push_str(&format!(" ({})", build_details.join(", ")));
    }
    lines.push(build);

    lines.push(format!(
        "Install source: {}",
        env.install_source.as_deref().unwrap_or("unknown")
    ));
    lines.push(format!(
        "Desktop: {}",
        env.current_desktop
            .as_deref()
            .or(env.session_desktop.as_deref())
            .or(env.desktop_session.as_deref())
            .unwrap_or("unknown")
    ));
    lines.push(format!(
        "Wayland display: {}",
        env.wayland_display.as_deref().unwrap_or("none")
    ));
    lines.push(format!(
        "Features: {}",
        if features.is_empty() {
            "none".to_string()
        } else {
            features.join(", ")
        }
    ));

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_env() -> DiagnosticsEnv {
        DiagnosticsEnv {
            version: "0.9.22".to_string(),
            commit: "51113dd1".to_string(),
            build_date: Some("2026-07-20".to_string()),
            install_source: Some("apt".to_string()),
            current_desktop: Some("Hyprland".to_string()),
            session_desktop: Some("hyprland".to_string()),
            desktop_session: None,
            wayland_display: Some("wayland-1".to_string()),
        }
    }

    #[test]
    fn reports_every_field_when_available() {
        let report = format_report(&sample_env(), vec!["tray", "portal"]);

        assert_eq!(
            report,
            "Wayscriber 0.9.22 (commit 51113dd1, built 2026-07-20)\n\
             Install source: apt\n\
             Desktop: Hyprland\n\
             Wayland display: wayland-1\n\
             Features: tray, portal"
        );
    }

    #[test]
    fn degrades_to_unknown_instead_of_omitting_lines() {
        let env = DiagnosticsEnv {
            version: "1.0.0".to_string(),
            commit: "unknown".to_string(),
            ..DiagnosticsEnv::default()
        };

        let report = format_report(&env, Vec::new());

        assert_eq!(
            report,
            "Wayscriber 1.0.0\n\
             Install source: unknown\n\
             Desktop: unknown\n\
             Wayland display: none\n\
             Features: none"
        );
    }

    #[test]
    fn falls_back_through_the_desktop_variables() {
        let env = DiagnosticsEnv {
            current_desktop: None,
            session_desktop: None,
            desktop_session: Some("sway".to_string()),
            ..sample_env()
        };

        assert!(format_report(&env, Vec::new()).contains("Desktop: sway"));
    }

    #[test]
    fn captured_report_is_never_empty() {
        assert!(report().starts_with("Wayscriber "));
    }
}
