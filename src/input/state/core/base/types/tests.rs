use super::CompositorCapabilities;

#[test]
fn compositor_capabilities_limitations_summary_returns_none_when_fully_available() {
    assert_eq!(
        CompositorCapabilities {
            layer_shell: true,
            screencopy: true,
            freeze_capture: true,
            pointer_constraints: true,
            desktop_environment: Default::default(),
            shell_mode: Default::default(),
        }
        .limitations_summary(),
        None
    );
}

#[test]
fn compositor_capabilities_limitations_summary_lists_missing_features_in_order() {
    assert_eq!(
        CompositorCapabilities {
            layer_shell: false,
            screencopy: true,
            freeze_capture: true,
            pointer_constraints: false,
            desktop_environment: Default::default(),
            shell_mode: Default::default(),
        }
        .limitations_summary(),
        Some(
            "Toolbars limited, light passthrough unavailable, Pointer lock unavailable".to_string()
        )
    );
}

#[test]
fn compositor_capabilities_reports_portal_freeze_without_hiding_limitations() {
    let caps = CompositorCapabilities {
        layer_shell: true,
        screencopy: false,
        freeze_capture: true,
        pointer_constraints: true,
        desktop_environment: Default::default(),
        shell_mode: Default::default(),
    };

    assert!(!caps.all_available());
    assert_eq!(
        caps.limitations_summary(),
        Some("Freeze uses portal capture".to_string())
    );
}
