//! Shared-widget unit tests.

use super::*;

/// Focus moving to a control inside a popover must not drop the layer
/// surface's keyboard mode: the popup's keyboard grab is live, and
/// pulling compositor focus from it mid-click stalls the in-flight
/// button release for seconds. Widget construction needs a GTK display,
/// so the body runs in an isolated child process the same way the other
/// GTK widget contract tests do.
#[test]
fn popover_internal_focus_keeps_the_keyboard_grab() {
    const CHILD_ENV: &str = "WAYSCRIBER_GTK_FOCUS_POLICY_CHILD";
    const TEST_NAME: &str =
        "toolbar_gtk::widgets::tests::popover_internal_focus_keeps_the_keyboard_grab";

    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg(TEST_NAME)
            .arg("--exact")
            .arg("--test-threads=1")
            .env(CHILD_ENV, "1")
            .status()
            .expect("run isolated GTK focus-policy test");
        assert!(status.success(), "isolated GTK focus-policy test failed");
        return;
    }

    if let Err(error) = gtk4::init() {
        eprintln!("skipping GTK focus-policy test: {error}");
        return;
    }

    let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    let bar_button = gtk4::Button::new();
    let bar_entry = gtk4::Entry::new();
    bar.append(&bar_button);
    bar.append(&bar_entry);

    let popover_body = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let popover_check = gtk4::CheckButton::new();
    let popover_entry = gtk4::Entry::new();
    popover_body.append(&popover_check);
    popover_body.append(&popover_entry);
    let popover = gtk4::Popover::new();
    popover.set_child(Some(&popover_body));
    popover.set_parent(&bar);

    assert!(
        focus_change_releases_keyboard(bar_button.upcast_ref()),
        "a plain bar control must still hand the keyboard back"
    );
    assert!(
        !focus_change_releases_keyboard(bar_entry.upcast_ref()),
        "the editable hex field keeps keyboard focus"
    );
    assert!(
        !focus_change_releases_keyboard(popover_check.upcast_ref()),
        "a popover checkbox must not break the popup's keyboard grab"
    );
    assert!(
        !focus_change_releases_keyboard(popover_entry.upcast_ref()),
        "a popover entry keeps the grab and its text input"
    );

    popover.unparent();
}

#[test]
fn gtk_feedback_carries_rebind_state_once() {
    let (tx, rx) = std::sync::mpsc::channel();
    let sender = FeedbackSender::new(tx);
    sender.set_rebind_state(ToolbarRebindModifier::CtrlShift, true);
    sender.capture_click_modifiers(
        gtk4::gdk::ModifierType::CONTROL_MASK | gtk4::gdk::ModifierType::SHIFT_MASK,
    );
    send_event(&sender, ToolbarEvent::Undo);
    send_event(&sender, ToolbarEvent::Redo);

    assert_eq!(
        rx.recv().unwrap(),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::Undo,
            rebind_requested: true,
        }
    );
    assert_eq!(
        rx.recv().unwrap(),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::Redo,
            rebind_requested: false,
        }
    );
}

#[test]
fn backend_modifier_latch_survives_focus_reset_during_click() {
    let (tx, rx) = std::sync::mpsc::channel();
    let sender = FeedbackSender::new(tx);
    sender.set_rebind_state(ToolbarRebindModifier::CtrlShift, true);
    sender.capture_click_modifiers(gtk4::gdk::ModifierType::empty());
    sender.set_rebind_state(ToolbarRebindModifier::CtrlShift, false);

    send_event(&sender, ToolbarEvent::Undo);

    assert_eq!(
        rx.recv().unwrap(),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::Undo,
            rebind_requested: true,
        }
    );
}
