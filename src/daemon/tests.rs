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
    let daemon_true = Daemon::new(None, false, Some(true));
    let daemon_false = Daemon::new(None, false, Some(false));
    let daemon_none = Daemon::new(None, false, None);

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
    for item in tray.menu() {
        match item {
            MenuItem::Standard(standard) if standard.label.contains(label) => {
                let activate = standard.activate;
                activate(tray);
                return;
            }
            MenuItem::Checkmark(check) if check.label.contains(label) => {
                let activate = check.activate;
                activate(tray);
                return;
            }
            _ => {}
        }
    }
    panic!("Menu item '{label}' not found");
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
fn tray_quit_action_sets_quit_flag() {
    let toggle = Arc::new(AtomicBool::new(false));
    let quit = Arc::new(AtomicBool::new(false));
    let mut tray = WayscriberTray::new_for_tests(toggle, quit.clone(), false);

    activate_menu_item(&mut tray, "Quit");
    assert!(quit.load(AtomicOrdering::SeqCst));
}
