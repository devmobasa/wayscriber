use super::*;
use crate::models::{DesktopEnvironment, ShortcutBackend};

fn shown_all() -> ShownAreas {
    ShownAreas {
        status: true,
        service: true,
        shortcut: true,
        light: true,
    }
}

fn visible_sections(shown: ShownAreas) -> Vec<DaemonSection> {
    DaemonSection::ALL
        .into_iter()
        .filter(|section| daemon_section_visible(*section, shown))
        .collect()
}

fn test_status() -> DaemonRuntimeStatus {
    DaemonRuntimeStatus {
        desktop: DesktopEnvironment::Hyprland,
        shortcut_backend: ShortcutBackend::Manual,
        shortcut_apply_capability: ShortcutApplyCapability::Manual,
        light_shortcut_apply_capability: LightShortcutApplyCapability::HyprlandNative,
        systemctl_available: true,
        gsettings_available: true,
        service_installed: false,
        service_enabled: false,
        service_active: false,
        service_unit_path: None,
        configured_shortcut: None,
        light_controls_configured: false,
        light_controls_config_path: None,
    }
}

#[test]
fn daemon_sections_keep_default_setup_order() {
    assert_eq!(
        visible_sections(shown_all()),
        vec![
            DaemonSection::Install,
            DaemonSection::Shortcut,
            DaemonSection::LightControls,
            DaemonSection::Start,
            DaemonSection::TechnicalDetails,
        ],
    );
}

#[test]
fn service_area_keeps_install_start_and_details() {
    assert_eq!(
        visible_sections(ShownAreas {
            status: false,
            service: true,
            shortcut: false,
            light: false,
        }),
        vec![
            DaemonSection::Install,
            DaemonSection::Start,
            DaemonSection::TechnicalDetails,
        ],
    );
}

#[test]
fn status_area_alone_keeps_only_details() {
    assert_eq!(
        visible_sections(ShownAreas {
            status: true,
            service: false,
            shortcut: false,
            light: false,
        }),
        vec![DaemonSection::TechnicalDetails],
    );
}

#[test]
fn shortcut_area_alone_keeps_only_the_shortcut_step() {
    assert_eq!(
        visible_sections(ShownAreas {
            status: false,
            service: false,
            shortcut: true,
            light: false,
        }),
        vec![DaemonSection::Shortcut],
    );
}

#[test]
fn overall_status_follows_service_state() {
    assert_eq!(overall_status(None).0, "Status: Detecting...");

    let mut status = test_status();
    assert_eq!(overall_status(Some(&status)).0, "Status: Not installed");

    status.service_installed = true;
    assert_eq!(
        overall_status(Some(&status)),
        ("Status: Installed, not running", Tone::Caution)
    );

    status.service_active = true;
    assert_eq!(
        overall_status(Some(&status)),
        ("Status: Running", Tone::Positive)
    );
}

#[test]
fn feedback_tone_flags_failures() {
    assert_eq!(
        feedback_tone("Background setup action failed"),
        Tone::Negative
    );
    assert_eq!(feedback_tone("gsettings ERROR: nope"), Tone::Negative);
    assert_eq!(
        feedback_tone("Background mode status loaded."),
        Tone::Positive
    );
}

#[test]
fn service_status_separates_running_from_enabled() {
    assert_eq!(service_status(true, true).0, "Running \u{2713}");
    assert_eq!(service_status(true, false).0, "Running (not enabled)");
    assert_eq!(service_status(false, true).0, "Enabled, not running");
    assert_eq!(service_status(false, false).0, "Stopped and disabled");
}

#[test]
fn shortcut_placeholder_follows_apply_capability() {
    assert_eq!(
        shortcut_placeholder(Some(ShortcutApplyCapability::GnomeCustomShortcut)),
        "e.g. Super+G or <Super>g"
    );
    assert_eq!(
        shortcut_placeholder(Some(ShortcutApplyCapability::PortalServiceDropIn)),
        "e.g. Ctrl+Shift+G or <Ctrl><Shift>g"
    );
    assert_eq!(
        shortcut_placeholder(Some(ShortcutApplyCapability::Manual)),
        "e.g. Ctrl+Shift+G"
    );
    assert_eq!(shortcut_placeholder(None), "e.g. Ctrl+Shift+G");
}

#[test]
fn missing_tools_only_reports_absent_ones() {
    let mut status = test_status();
    assert_eq!(missing_tools(&status), None);

    status.gsettings_available = false;
    assert_eq!(
        missing_tools(&status).as_deref(),
        Some("Missing tools: gsettings")
    );

    status.systemctl_available = false;
    assert_eq!(
        missing_tools(&status).as_deref(),
        Some("Missing tools: systemctl, gsettings")
    );
}

#[test]
fn light_controls_details_reflect_configured_state() {
    let mut status = test_status();
    assert_eq!(light_controls_details(&status), None);

    status.light_controls_config_path = Some("/tmp/wayscriber.conf".to_string());
    assert_eq!(
        light_controls_details(&status).as_deref(),
        Some("Light controls include: /tmp/wayscriber.conf")
    );

    status.light_controls_configured = true;
    assert_eq!(
        light_controls_details(&status).as_deref(),
        Some("Light controls: configured at /tmp/wayscriber.conf")
    );
}
