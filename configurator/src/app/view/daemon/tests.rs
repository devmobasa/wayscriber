use super::*;

#[test]
fn daemon_sections_keep_default_setup_order() {
    assert_eq!(
        daemon_sections(true, true, true, true),
        vec![
            DaemonSection::Install,
            DaemonSection::Shortcut,
            DaemonSection::LightControls,
            DaemonSection::Start,
            DaemonSection::TechnicalDetails,
        ],
    );
}
