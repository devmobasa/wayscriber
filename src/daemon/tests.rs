use super::core::Daemon;
#[cfg(feature = "tray")]
use super::tray::WayscriberTray;
#[cfg(feature = "tray")]
use super::types::DaemonControlMessage;
use super::types::{BackendRunner, OverlayState};
use std::sync::mpsc::{self, Receiver};

#[cfg(feature = "tray")]
use ksni::{Tray, menu::MenuItem};

type RunnerInvocation = (Option<String>, Option<bool>);

fn runner_probe() -> (Box<BackendRunner>, Receiver<RunnerInvocation>) {
    let (sender, receiver) = mpsc::channel();
    let runner = Box::new(move |mode, session_resume_override| {
        sender
            .send((mode, session_resume_override))
            .map_err(|_| anyhow::anyhow!("runner probe receiver disconnected"))
    });
    (runner, receiver)
}

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
    let (runner, calls) = runner_probe();
    let mut daemon = Daemon::with_backend_runner(None, runner);

    daemon
        .toggle_overlay()
        .expect("fixture toggles through its inline backend runner");
    assert_eq!(calls.try_iter().count(), 1);
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[cfg(feature = "tray")]
#[test]
fn toggle_overlay_invokes_backend_when_hidden() {
    let (runner, modes) = runner_probe();
    let mut daemon = Daemon::with_backend_runner(Some("whiteboard".into()), runner);

    daemon
        .toggle_overlay()
        .expect("fixture invokes its hidden-overlay backend runner");
    assert_eq!(
        modes.try_iter().collect::<Vec<_>>(),
        [(Some("whiteboard".into()), None)]
    );
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[cfg(feature = "tray")]
#[test]
fn hide_overlay_is_idempotent() {
    let runner = Box::new(|_: Option<String>, _: Option<bool>| Ok(()));
    let mut daemon = Daemon::with_backend_runner(None, runner);
    daemon
        .hide_overlay()
        .expect("fixture hides an already-hidden overlay");
    assert_eq!(daemon.test_state(), OverlayState::Hidden);

    daemon.overlay_state = OverlayState::Visible;
    daemon
        .toggle_overlay()
        .expect("fixture hides its synthetic visible overlay");
    assert_eq!(daemon.test_state(), OverlayState::Hidden);
}

#[cfg(feature = "tray")]
fn activate_menu_item(tray: &mut WayscriberTray, label: &str) -> bool {
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
    activate_in(tray, items, label)
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
    let (mut tray, inbox, wake, _process_broker) = WayscriberTray::new_for_tests(false);

    assert!(activate_menu_item(&mut tray, "Toggle Overlay"));
    assert!(
        wake.wait_readable(Some(std::time::Duration::ZERO))
            .expect("fixture observes the tray-toggle daemon wake")
    );
    assert!(matches!(
        inbox.drain().controls.as_slice(),
        [DaemonControlMessage::Visibility(_)]
    ));
}

#[cfg(feature = "tray")]
#[test]
fn tray_menu_exposes_minimal_light_actions() {
    let (tray, _inbox, _wake, _process_broker) = WayscriberTray::new_for_tests(false);

    let labels = menu_labels(&tray);
    assert!(labels.iter().any(|label| label.contains("Light Mode")));
    assert!(labels.iter().any(|label| label.contains("Light Drawing")));
    assert!(!labels.iter().any(|label| label.contains("Light Draw On")));
    assert!(!labels.iter().any(|label| label.contains("Light Draw Off")));
}

#[cfg(feature = "tray")]
#[test]
fn tray_menu_groups_actions_to_fit_short_displays() {
    let (tray, _inbox, _wake, _process_broker) = WayscriberTray::new_for_tests(false);
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
    let (mut tray, inbox, wake, _process_broker) = WayscriberTray::new_for_tests(false);

    assert!(activate_menu_item(&mut tray, "Quit"));
    assert!(
        wake.wait_readable(Some(std::time::Duration::ZERO))
            .expect("fixture observes the tray-quit daemon wake")
    );
    assert!(matches!(
        inbox.drain().controls.as_slice(),
        [DaemonControlMessage::Quit]
    ));
}
