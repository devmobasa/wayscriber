use super::core::Daemon;
#[cfg(feature = "tray")]
use super::tray::WayscriberTray;
use super::types::{BackendRunner, OverlayState};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

#[cfg(feature = "tray")]
use ksni::{Tray, menu::MenuItem};
#[cfg(feature = "tray")]
use std::sync::atomic::AtomicBool;

#[test]
fn daemon_session_resume_override_reflects_constructor_value() {
    let daemon_true = Daemon::new(None, false, Some(true), None);
    let daemon_false = Daemon::new(None, false, Some(false), None);
    let daemon_none = Daemon::new(None, false, None, None);

    assert_eq!(daemon_true.session_resume_override(), Some(true));
    assert_eq!(daemon_false.session_resume_override(), Some(false));
    assert_eq!(daemon_none.session_resume_override(), None);
}

#[test]
fn toggle_overlay_with_backend_runner_works_without_external_process() {
    let called = Arc::new(AtomicUsize::new(0));
    let called_clone = Arc::clone(&called);
    let runner: Arc<BackendRunner> = Arc::new(move |_| {
        called_clone.fetch_add(1, AtomicOrdering::SeqCst);
        Ok(())
    });
    let mut daemon = Daemon::with_backend_runner(None, runner);

    daemon.toggle_overlay().unwrap();
    assert_eq!(called.load(AtomicOrdering::SeqCst), 1);
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[cfg(feature = "tray")]
fn runner_counter(count: Arc<AtomicUsize>) -> Arc<BackendRunner> {
    Arc::new(move |mode: Option<String>| -> anyhow::Result<()> {
        assert_eq!(mode.as_deref(), Some("whiteboard"));
        count.fetch_add(1, AtomicOrdering::SeqCst);
        Ok(())
    })
}

#[cfg(feature = "tray")]
#[test]
fn toggle_overlay_invokes_backend_when_hidden() {
    let counter = Arc::new(AtomicUsize::new(0));
    let runner = runner_counter(counter.clone());
    let mut daemon = Daemon::with_backend_runner(Some("whiteboard".into()), runner);

    daemon.toggle_overlay().unwrap();
    assert_eq!(counter.load(AtomicOrdering::SeqCst), 1);
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[cfg(feature = "tray")]
#[test]
fn hide_overlay_is_idempotent() {
    let runner = Arc::new(|_: Option<String>| Ok(())) as Arc<BackendRunner>;
    let mut daemon = Daemon::with_backend_runner(None, runner);
    daemon.hide_overlay().unwrap();
    assert_eq!(daemon.test_state(), OverlayState::Hidden);

    daemon.overlay_state = OverlayState::Visible;
    daemon.toggle_overlay().unwrap();
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[cfg(feature = "tray")]
fn activate_menu_item(tray: &mut WayscriberTray, label: &str) {
    fn activate_in(
        tray: &mut WayscriberTray,
        items: Vec<MenuItem<WayscriberTray>>,
        label: &str,
    ) -> bool {
        for item in items {
            let activated = match item {
                MenuItem::Standard(standard) if standard.label.contains(label) => {
                    (standard.activate)(tray);
                    true
                }
                MenuItem::Checkmark(check) if check.label.contains(label) => {
                    (check.activate)(tray);
                    true
                }
                MenuItem::SubMenu(submenu) => activate_in(tray, submenu.submenu, label),
                _ => false,
            };
            if activated {
                return true;
            }
        }
        false
    }

    let items = tray.menu();
    if activate_in(tray, items, label) {
        return;
    }
    panic!("Menu item '{label}' not found");
}

#[cfg(feature = "tray")]
fn collect_menu_labels(items: Vec<MenuItem<WayscriberTray>>, labels: &mut Vec<String>) {
    for item in items {
        match item {
            MenuItem::Standard(standard) => labels.push(standard.label),
            MenuItem::Checkmark(check) => labels.push(check.label),
            MenuItem::SubMenu(submenu) => {
                labels.push(submenu.label);
                collect_menu_labels(submenu.submenu, labels);
            }
            _ => {}
        }
    }
}

#[cfg(feature = "tray")]
fn menu_labels(tray: &WayscriberTray) -> Vec<String> {
    let mut labels = Vec::new();
    collect_menu_labels(tray.menu(), &mut labels);
    labels
}

#[cfg(feature = "tray")]
#[test]
fn tray_toggle_action_sets_flag() {
    let toggle = Arc::new(AtomicBool::new(false));
    let quit = Arc::new(AtomicBool::new(false));
    let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
    let mut tray = WayscriberTray::new_for_tests_with_wake(toggle.clone(), quit, wake.handle());

    activate_menu_item(&mut tray, "Toggle Overlay");
    assert!(toggle.load(AtomicOrdering::SeqCst));
    assert!(wake.wait_readable(Some(std::time::Duration::ZERO)).unwrap());
}

#[cfg(feature = "tray")]
#[test]
fn tray_menu_exposes_minimal_light_actions() {
    let toggle = Arc::new(AtomicBool::new(false));
    let quit = Arc::new(AtomicBool::new(false));
    let tray = WayscriberTray::new_for_tests(toggle, quit);

    let labels = menu_labels(&tray);
    assert!(labels.iter().any(|label| label.contains("Light Mode")));
    assert!(labels.iter().any(|label| label.contains("Light Drawing")));
    assert!(!labels.iter().any(|label| label.contains("Light Draw On")));
    assert!(!labels.iter().any(|label| label.contains("Light Draw Off")));
}

#[cfg(feature = "tray")]
#[test]
fn tray_menu_groups_actions_to_fit_short_displays() {
    let toggle = Arc::new(AtomicBool::new(false));
    let quit = Arc::new(AtomicBool::new(false));
    let tray = WayscriberTray::new_for_tests(toggle, quit);
    let menu = tray.menu();

    assert!(menu.len() <= 12, "top-level tray menu grew too tall");

    let submenu_labels: Vec<_> = menu
        .into_iter()
        .filter_map(|item| match item {
            MenuItem::SubMenu(submenu) => Some(submenu.label),
            _ => None,
        })
        .collect();
    assert_eq!(
        submenu_labels,
        ["Drawing Modes", "Capture", "Settings & Data"]
    );
}

#[cfg(feature = "tray")]
#[test]
fn tray_quit_action_sets_quit_flag() {
    let toggle = Arc::new(AtomicBool::new(false));
    let quit = Arc::new(AtomicBool::new(false));
    let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
    let mut tray = WayscriberTray::new_for_tests_with_wake(toggle, quit.clone(), wake.handle());

    activate_menu_item(&mut tray, "Quit");
    assert!(quit.load(AtomicOrdering::SeqCst));
    assert!(wake.wait_readable(Some(std::time::Duration::ZERO)).unwrap());
}

/// The tray used to answer a session-resume click by rewriting `[session]`. It
/// now points at the screen that owns those flags, so the menu offers a way in
/// rather than a switch the daemon would have to persist.
#[cfg(feature = "tray")]
#[test]
fn tray_menu_offers_session_settings_instead_of_a_session_toggle() {
    let toggle = Arc::new(AtomicBool::new(false));
    let quit = Arc::new(AtomicBool::new(false));
    let tray = WayscriberTray::new_for_tests(toggle, quit);

    let labels = menu_labels(&tray);
    assert!(
        labels
            .iter()
            .any(|label| label == "Session persistence settings…"),
        "the settings submenu should offer the session screen: {labels:?}"
    );
    assert!(
        !labels
            .iter()
            .any(|label| label.starts_with("Session resume")),
        "the session-resume toggle should be gone: {labels:?}"
    );
}

/// Where the item sends the user is the whole of what it does, so the test
/// watches the launch itself: a stand-in configurator writes down the arguments
/// the tray handed it.
#[cfg(feature = "tray")]
#[test]
fn tray_session_settings_item_opens_the_configurator_at_the_session_screen() {
    use std::os::unix::fs::PermissionsExt;

    let _environment = crate::test_env::lock();
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let recorded = temp.path().join("arguments");
    let recorder = temp.path().join("recording-configurator");
    std::fs::write(
        &recorder,
        // Written under a scratch name and renamed so a read either sees the
        // whole argument list or no file at all.
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{0}.part'\nmv '{0}.part' '{0}'\n",
            recorded.display()
        ),
    )
    .expect("the recording configurator should be written");
    let mut permissions = std::fs::metadata(&recorder)
        .expect("the recording configurator should exist")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&recorder, permissions)
        .expect("the recording configurator should be executable");

    let previous_configurator = std::env::var_os(crate::env_vars::CONFIGURATOR_ENV);
    let previous_config_home = std::env::var_os(crate::env_vars::XDG_CONFIG_HOME_ENV);
    // SAFETY: access to the process environment is serialized by test_env. The
    // configurator override makes the broker accept the stand-in; the config
    // home keeps the launch failure path away from the developer's own file.
    unsafe {
        std::env::set_var(crate::env_vars::CONFIGURATOR_ENV, &recorder);
        std::env::set_var(crate::env_vars::XDG_CONFIG_HOME_ENV, temp.path());
    }

    let toggle = Arc::new(AtomicBool::new(false));
    let quit = Arc::new(AtomicBool::new(false));
    let mut tray = WayscriberTray::new_for_tests_with_configurator(
        toggle,
        quit,
        recorder.to_string_lossy().into_owned(),
    );
    let launched = record_session_settings_launch(&mut tray, &recorded);

    // SAFETY: as above; the previous values are restored before the assertion
    // so a failure cannot leak this test's environment into the next one.
    unsafe {
        match previous_configurator {
            Some(value) => std::env::set_var(crate::env_vars::CONFIGURATOR_ENV, value),
            None => std::env::remove_var(crate::env_vars::CONFIGURATOR_ENV),
        }
        match previous_config_home {
            Some(value) => std::env::set_var(crate::env_vars::XDG_CONFIG_HOME_ENV, value),
            None => std::env::remove_var(crate::env_vars::XDG_CONFIG_HOME_ENV),
        }
    }

    assert_eq!(
        launched.as_deref(),
        Some("--open\nsession\n"),
        "the session item must open the configurator at its Session screen"
    );
}

/// Click the item and wait for the stand-in configurator to record its
/// arguments.
///
/// The tray spawns through the process broker's active-instance slot, which a
/// broker test running in parallel can replace and then clear. Re-establishing
/// the broker and clicking again keeps that race out of the assertion.
#[cfg(feature = "tray")]
fn record_session_settings_launch(
    tray: &mut WayscriberTray,
    recorded: &std::path::Path,
) -> Option<String> {
    for _ in 0..3 {
        let _broker = crate::process_broker::start_for_runtime().ok()?;
        activate_menu_item(tray, "Session persistence settings");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if let Ok(arguments) = std::fs::read_to_string(recorded) {
                return Some(arguments);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    None
}

/// `config.toml` is an authored input, and the daemon lost the last runtime
/// authority over it: the tray reads the file to draw itself and sends the user
/// to the configurator to change it. The capability is one import away, so the
/// absence of it is checked rather than remembered.
#[test]
fn no_daemon_source_can_write_the_config() {
    /// The names a daemon source would have to mention to write the file: the
    /// document that owns the save operations, its explicit-save entry point,
    /// the two primitives underneath it, and the runtime backup that existed
    /// only to make such a save recoverable.
    const WRITE_CAPABILITIES: [&str; 5] = [
        "ConfigDocument",
        "RuntimeConfigBackup",
        "save_with_backup",
        "write_config_text_atomic",
        "create_config_backup",
    ];

    fn production_sources(directory: &Path, sources: &mut Vec<(PathBuf, String)>) {
        for entry in fs::read_dir(directory).expect("read a daemon directory") {
            let path = entry.expect("read a daemon entry").path();
            if path.is_dir() {
                production_sources(&path, sources);
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs")
                || path.file_name().and_then(|name| name.to_str()) == Some("tests.rs")
            {
                continue;
            }
            let source = fs::read_to_string(&path).expect("read a daemon source");
            sources.push((path, source));
        }
    }

    let daemon_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/daemon");
    let mut sources = Vec::new();
    production_sources(&daemon_dir, &mut sources);

    for (path, source) in &sources {
        for capability in WRITE_CAPABILITIES {
            assert!(
                !source.contains(capability),
                "{} reaches a config write capability through {capability}",
                path.display()
            );
        }
    }

    let scanned = sources
        .iter()
        .map(|(path, _)| {
            path.strip_prefix(&daemon_dir)
                .unwrap_or(path.as_path())
                .to_path_buf()
        })
        .collect::<Vec<_>>();
    for expected in ["tray/ksni.rs", "tray/mod.rs", "tray/runtime.rs"] {
        assert!(
            scanned.iter().any(|path| path == Path::new(expected)),
            "the scan missed {expected}, so it proves nothing: {scanned:?}"
        );
    }
}
