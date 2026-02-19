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
        let combined = format!("{current};{session}").to_lowercase();
        if combined.contains("gnome") {
            return Self::Gnome;
        }
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
    pub fn from_environment(
        desktop: DesktopEnvironment,
        gsettings_available: bool,
        systemctl_available: bool,
    ) -> Self {
        if desktop == DesktopEnvironment::Gnome && gsettings_available {
            return Self::GnomeCustomShortcut;
        }
        if systemctl_available {
            return Self::PortalServiceDropIn;
        }
        Self::Manual
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::GnomeCustomShortcut => "GNOME Settings custom shortcut",
            Self::PortalServiceDropIn => "Portal shortcut via systemd service drop-in",
            Self::Manual => "Manual keybind required",
        }
    }

    pub fn guidance(self) -> &'static str {
        match self {
            Self::GnomeCustomShortcut => {
                "Configurator will write a GNOME custom shortcut that runs `pkill -SIGUSR1 wayscriber`."
            }
            Self::PortalServiceDropIn => {
                "Configurator will set WAYSCRIBER_PORTAL_SHORTCUT in systemd user service drop-ins."
            }
            Self::Manual => {
                "Automatic setup unavailable in this environment; add a keybind manually to run `pkill -SIGUSR1 wayscriber`."
            }
        }
    }

    pub fn friendly_label(self) -> &'static str {
        match self {
            Self::GnomeCustomShortcut => "Shortcut will be configured via GNOME Settings",
            Self::PortalServiceDropIn => "Shortcut will be configured via your desktop portal",
            Self::Manual => "Automatic shortcut setup is not available — you'll need to add a keybind manually",
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
    fn shortcut_backend_selection_prefers_gnome_when_available() {
        assert_eq!(
            ShortcutBackend::from_environment(DesktopEnvironment::Gnome, true, true),
            ShortcutBackend::GnomeCustomShortcut
        );
        assert_eq!(
            ShortcutBackend::from_environment(DesktopEnvironment::Kde, false, true),
            ShortcutBackend::PortalServiceDropIn
        );
        assert_eq!(
            ShortcutBackend::from_environment(DesktopEnvironment::Unknown, false, false),
            ShortcutBackend::Manual
        );
    }
}
