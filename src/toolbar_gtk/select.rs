//! Decides which frontend renders the toolbars.
//!
//! The GTK frontend only replaces the built-in bars where the built-in
//! bars would have used their own layer surfaces: gtk4-layer-shell needs
//! the same `zwlr_layer_shell_v1` protocol, and compositors that force the
//! inline fallback (overlay-layer canvas on niri/sway, forced inline)
//! would cover separate GTK surfaces just the same.

use std::sync::OnceLock;

use crate::config::{Config, ToolbarBackendKind};
use crate::env_vars::TOOLBAR_BACKEND_ENV;

/// Why the GTK toolbars cannot be used even when requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GtkUnavailableReason {
    FeatureDisabled,
    NoLayerShell,
    InlineToolbars,
    OverlayLayerMain,
}

impl GtkUnavailableReason {
    pub fn describe(self) -> &'static str {
        match self {
            Self::FeatureDisabled => "this build does not include the toolbar-gtk feature",
            Self::NoLayerShell => "the compositor does not support layer-shell",
            Self::InlineToolbars => "inline toolbars are forced (config or env)",
            Self::OverlayLayerMain => "the overlay canvas occupies the compositor overlay layer",
        }
    }
}

/// Compositor and build facts the decision depends on.
#[derive(Debug, Clone, Copy)]
pub struct GtkPreconditions {
    pub feature_compiled: bool,
    pub layer_shell: bool,
    pub force_inline: bool,
    pub main_surface_uses_overlay_layer: bool,
}

impl GtkPreconditions {
    fn blocker(self) -> Option<GtkUnavailableReason> {
        if !self.feature_compiled {
            Some(GtkUnavailableReason::FeatureDisabled)
        } else if !self.layer_shell {
            Some(GtkUnavailableReason::NoLayerShell)
        } else if self.force_inline {
            Some(GtkUnavailableReason::InlineToolbars)
        } else if self.main_surface_uses_overlay_layer {
            Some(GtkUnavailableReason::OverlayLayerMain)
        } else {
            None
        }
    }
}

/// Effective toolbar frontend after applying the request to the
/// preconditions. `Builtin` carries the blocker when GTK was requested
/// explicitly, so the caller can warn; `Auto` falls back silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarFrontend {
    Gtk,
    Builtin(Option<GtkUnavailableReason>),
}

pub fn resolve_frontend(request: ToolbarBackendKind, pre: GtkPreconditions) -> ToolbarFrontend {
    match request {
        ToolbarBackendKind::Builtin => ToolbarFrontend::Builtin(None),
        ToolbarBackendKind::Auto => match pre.blocker() {
            None => ToolbarFrontend::Gtk,
            Some(_) => ToolbarFrontend::Builtin(None),
        },
        ToolbarBackendKind::Gtk => match pre.blocker() {
            None => ToolbarFrontend::Gtk,
            Some(reason) => ToolbarFrontend::Builtin(Some(reason)),
        },
    }
}

fn parse_backend_env(raw: &str) -> Option<ToolbarBackendKind> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "auto" => Some(ToolbarBackendKind::Auto),
        "gtk" | "gtk4" => Some(ToolbarBackendKind::Gtk),
        "builtin" | "cairo" => Some(ToolbarBackendKind::Builtin),
        other => {
            log::warn!(
                "Ignoring unknown {TOOLBAR_BACKEND_ENV} value '{other}' (expected 'auto', 'gtk', or 'builtin')"
            );
            None
        }
    }
}

fn backend_env_override() -> Option<ToolbarBackendKind> {
    static OVERRIDE: OnceLock<Option<ToolbarBackendKind>> = OnceLock::new();
    *OVERRIDE.get_or_init(|| {
        std::env::var(TOOLBAR_BACKEND_ENV)
            .ok()
            .and_then(|raw| parse_backend_env(&raw))
    })
}

/// Backend request from config, with the env var taking precedence.
pub fn requested_backend(config: &Config) -> ToolbarBackendKind {
    backend_env_override().unwrap_or(config.ui.toolbar.backend)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_CLEAR: GtkPreconditions = GtkPreconditions {
        feature_compiled: true,
        layer_shell: true,
        force_inline: false,
        main_surface_uses_overlay_layer: false,
    };

    #[test]
    fn auto_uses_gtk_when_preconditions_hold() {
        assert_eq!(
            resolve_frontend(ToolbarBackendKind::Auto, ALL_CLEAR),
            ToolbarFrontend::Gtk
        );
    }

    #[test]
    fn auto_falls_back_silently_on_any_blocker() {
        for pre in [
            GtkPreconditions {
                feature_compiled: false,
                ..ALL_CLEAR
            },
            GtkPreconditions {
                layer_shell: false,
                ..ALL_CLEAR
            },
            GtkPreconditions {
                force_inline: true,
                ..ALL_CLEAR
            },
            GtkPreconditions {
                main_surface_uses_overlay_layer: true,
                ..ALL_CLEAR
            },
        ] {
            assert_eq!(
                resolve_frontend(ToolbarBackendKind::Auto, pre),
                ToolbarFrontend::Builtin(None)
            );
        }
    }

    #[test]
    fn explicit_gtk_reports_the_blocker() {
        assert_eq!(
            resolve_frontend(
                ToolbarBackendKind::Gtk,
                GtkPreconditions {
                    layer_shell: false,
                    ..ALL_CLEAR
                }
            ),
            ToolbarFrontend::Builtin(Some(GtkUnavailableReason::NoLayerShell))
        );
        assert_eq!(
            resolve_frontend(ToolbarBackendKind::Gtk, ALL_CLEAR),
            ToolbarFrontend::Gtk
        );
    }

    #[test]
    fn explicit_builtin_always_wins() {
        assert_eq!(
            resolve_frontend(ToolbarBackendKind::Builtin, ALL_CLEAR),
            ToolbarFrontend::Builtin(None)
        );
    }

    #[test]
    fn env_values_parse_case_insensitively() {
        assert_eq!(parse_backend_env(" GTK4 "), Some(ToolbarBackendKind::Gtk));
        assert_eq!(parse_backend_env("gtk"), Some(ToolbarBackendKind::Gtk));
        assert_eq!(parse_backend_env("Auto"), Some(ToolbarBackendKind::Auto));
        assert_eq!(
            parse_backend_env("builtin"),
            Some(ToolbarBackendKind::Builtin)
        );
        assert_eq!(
            parse_backend_env("cairo"),
            Some(ToolbarBackendKind::Builtin)
        );
        assert_eq!(parse_backend_env(""), None);
        assert_eq!(parse_backend_env("nonsense"), None);
    }
}
