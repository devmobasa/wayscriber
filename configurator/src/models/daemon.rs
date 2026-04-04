use wayscriber::shortcut_hint::{
    PortalShortcutDropInState, ShortcutRuntimeBackend, ShortcutRuntimeInputs, is_gnome_desktop,
    portal_runtime_supported, resolve_shortcut_runtime_backend,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEnvironment {
    Gnome,
    Kde,
    Unknown,
}

impl DesktopEnvironment {
    pub fn detect_current() -> Self {
        let current = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let session = std::env::var("XDG_SESSION_DESKTOP").unwrap_or_default();
        Self::from_desktop_strings(&current, &session)
    }

    pub fn from_desktop_strings(current: &str, session: &str) -> Self {
        if is_gnome_desktop(current, session) {
            return Self::Gnome;
        }
        let combined = format!("{current};{session}").to_lowercase();
        if combined.contains("kde") || combined.contains("plasma") {
            return Self::Kde;
        }
        Self::Unknown
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Gnome => "GNOME",
            Self::Kde => "KDE Plasma",
            Self::Unknown => "Unknown/Other",
        }
    }

    pub fn default_shortcut_input(self) -> &'static str {
        match self {
            Self::Gnome => "Super+G",
            Self::Kde | Self::Unknown => "Ctrl+Shift+G",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutBackend {
    GnomeCustomShortcut,
    PortalServiceDropIn,
    Manual,
}

impl ShortcutBackend {
    pub fn from_runtime_inputs(
        desktop: DesktopEnvironment,
        portal_dropin_state: PortalShortcutDropInState,
    ) -> Self {
        match resolve_shortcut_runtime_backend(ShortcutRuntimeInputs {
            gnome_desktop: desktop == DesktopEnvironment::Gnome,
            portal_runtime_supported: portal_runtime_supported(),
            portal_dropin_state,
        }) {
            ShortcutRuntimeBackend::GnomeCustomShortcut => Self::GnomeCustomShortcut,
            ShortcutRuntimeBackend::PortalGlobalShortcuts => Self::PortalServiceDropIn,
            ShortcutRuntimeBackend::Manual => Self::Manual,
        }
    }

    pub fn friendly_label(self) -> &'static str {
        match self {
            Self::GnomeCustomShortcut => "Active shortcut backend: GNOME custom shortcut",
            Self::PortalServiceDropIn => "Active shortcut backend: desktop portal",
            Self::Manual => "Active shortcut backend: manual/none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutApplyCapability {
    GnomeCustomShortcut,
    PortalServiceDropIn,
    Manual,
}

impl ShortcutApplyCapability {
    pub fn from_environment(
        desktop: DesktopEnvironment,
        gsettings_available: bool,
        systemctl_available: bool,
    ) -> Self {
        if desktop == DesktopEnvironment::Gnome {
            if gsettings_available {
                return Self::GnomeCustomShortcut;
            }
            return Self::Manual;
        }
        if systemctl_available && portal_runtime_supported() {
            return Self::PortalServiceDropIn;
        }
        Self::Manual
    }

    pub fn friendly_label(self) -> &'static str {
        match self {
            Self::GnomeCustomShortcut => "Shortcut setup available via GNOME Settings",
            Self::PortalServiceDropIn => "Shortcut setup available via desktop portal drop-in",
            Self::Manual => "Automatic shortcut setup unavailable in this session",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonAction {
    RefreshStatus,
    InstallOrUpdateService,
    EnableAndStartService,
    RestartService,
    StopAndDisableService,
    ApplyShortcut,
}

#[derive(Debug, Clone)]
pub struct DaemonRuntimeStatus {
    pub desktop: DesktopEnvironment,
    pub shortcut_backend: ShortcutBackend,
    pub shortcut_apply_capability: ShortcutApplyCapability,
    pub systemctl_available: bool,
    pub gsettings_available: bool,
    pub service_installed: bool,
    pub service_enabled: bool,
    pub service_active: bool,
    pub service_unit_path: Option<String>,
    pub configured_shortcut: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DaemonActionResult {
    pub status: DaemonRuntimeStatus,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_desktop_prefers_explicit_gnome_marker() {
        assert_eq!(
            DesktopEnvironment::from_desktop_strings("GNOME", ""),
            DesktopEnvironment::Gnome
        );
        assert_eq!(
            DesktopEnvironment::from_desktop_strings("ubuntu:GNOME", "ubuntu"),
            DesktopEnvironment::Gnome
        );
    }

    #[test]
    fn detect_desktop_avoids_assuming_bare_ubuntu_is_gnome() {
        assert_eq!(
            DesktopEnvironment::from_desktop_strings("", "ubuntu"),
            DesktopEnvironment::Unknown
        );
    }

    #[test]
    fn detect_desktop_kde_variants() {
        assert_eq!(
            DesktopEnvironment::from_desktop_strings("KDE", ""),
            DesktopEnvironment::Kde
        );
        assert_eq!(
            DesktopEnvironment::from_desktop_strings("plasma", ""),
            DesktopEnvironment::Kde
        );
    }

    #[test]
    fn shortcut_backend_selection_prefers_gnome_without_explicit_opt_in() {
        assert_eq!(
            ShortcutBackend::from_runtime_inputs(
                DesktopEnvironment::Gnome,
                PortalShortcutDropInState {
                    portal_shortcut_present: true,
                    portal_app_id_present: true,
                    explicit_portal_opt_in_present: false,
                }
            ),
            ShortcutBackend::GnomeCustomShortcut
        );
        assert_eq!(
            ShortcutBackend::from_runtime_inputs(
                DesktopEnvironment::Gnome,
                PortalShortcutDropInState {
                    portal_shortcut_present: true,
                    portal_app_id_present: true,
                    explicit_portal_opt_in_present: true,
                }
            ),
            if portal_runtime_supported() {
                ShortcutBackend::PortalServiceDropIn
            } else {
                ShortcutBackend::GnomeCustomShortcut
            }
        );
        assert_eq!(
            ShortcutBackend::from_runtime_inputs(
                DesktopEnvironment::Kde,
                PortalShortcutDropInState::default(),
            ),
            if portal_runtime_supported() {
                ShortcutBackend::PortalServiceDropIn
            } else {
                ShortcutBackend::Manual
            }
        );
    }

    #[test]
    fn shortcut_apply_capability_does_not_fallback_to_portal_on_gnome() {
        assert_eq!(
            ShortcutApplyCapability::from_environment(DesktopEnvironment::Gnome, true, true),
            ShortcutApplyCapability::GnomeCustomShortcut
        );
        assert_eq!(
            ShortcutApplyCapability::from_environment(DesktopEnvironment::Gnome, false, true),
            ShortcutApplyCapability::Manual
        );
        assert_eq!(
            ShortcutApplyCapability::from_environment(DesktopEnvironment::Unknown, false, true),
            if portal_runtime_supported() {
                ShortcutApplyCapability::PortalServiceDropIn
            } else {
                ShortcutApplyCapability::Manual
            }
        );
    }

    #[test]
    fn configurator_build_keeps_portal_runtime_support_enabled() {
        assert!(
            portal_runtime_supported(),
            "wayscriber-configurator must enable the wayscriber `portal` feature so status and shortcut setup stay aligned with portal-capable daemon builds"
        );
    }
}
