use super::core::Daemon;
#[cfg(feature = "tray")]
use super::tray::WayscriberTray;
use super::types::{BackendRunner, OverlayState};
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
    let mut tray = WayscriberTray::new_for_tests(toggle.clone(), quit, false);

    activate_menu_item(&mut tray, "Toggle Overlay");
    assert!(toggle.load(AtomicOrdering::SeqCst));
}

#[cfg(feature = "tray")]
#[test]
fn tray_menu_exposes_minimal_light_actions() {
    let toggle = Arc::new(AtomicBool::new(false));
    let quit = Arc::new(AtomicBool::new(false));
    let tray = WayscriberTray::new_for_tests(toggle, quit, false);

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
    let tray = WayscriberTray::new_for_tests(toggle, quit, false);
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
    let mut tray = WayscriberTray::new_for_tests(toggle, quit.clone(), false);

    activate_menu_item(&mut tray, "Quit");
    assert!(quit.load(AtomicOrdering::SeqCst));
}
