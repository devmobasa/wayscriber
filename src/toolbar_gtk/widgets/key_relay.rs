//! Hands key presses the GTK toolbar receives over to the overlay.
//!
//! The toolbar runs on its own Wayland connection. When the compositor leaves
//! keyboard focus on a toolbar surface — most often right after a click opened
//! one of its popovers — the overlay's keyboard hears nothing, so Escape could
//! not close the popover and tool shortcuts vanished. The relay forwards those
//! presses to the backend, which routes them exactly like its own: Escape
//! closes the open menu, and any other shortcut closes it and then runs.

use gtk4::glib::translate::IntoGlib;
use gtk4::prelude::*;

use super::{FeedbackSender, gdk_pointer_modifiers};
use crate::toolbar_gtk::GtkToolbarFeedback;

/// What the focused toolbar widget does with the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FocusedKeys {
    /// No widget holds focus (toolbar buttons are not focusable).
    Nothing,
    /// A focused control that Space and Return activate, such as a checkbox.
    Activation,
    /// A widget that owns typing and arrow keys: the hex entry and sliders.
    /// These handle Escape themselves.
    Editing,
}

/// Whether a key press stays with GTK instead of going to the overlay.
///
/// Modifier presses stay: their state rides on the next forwarded key. Tab
/// stays so keyboard navigation between popover controls keeps working.
pub(super) fn key_stays_local(
    keyval: gtk4::gdk::Key,
    is_modifier: bool,
    focus: FocusedKeys,
) -> bool {
    use gtk4::gdk::Key;

    if is_modifier || matches!(keyval, Key::Tab | Key::ISO_Left_Tab | Key::KP_Tab) {
        return true;
    }

    match focus {
        FocusedKeys::Nothing => false,
        FocusedKeys::Activation => matches!(
            keyval,
            Key::space | Key::KP_Space | Key::Return | Key::KP_Enter | Key::ISO_Enter
        ),
        FocusedKeys::Editing => true,
    }
}

pub(super) fn forwarded_key_feedback(
    keyval: gtk4::gdk::Key,
    state: gtk4::gdk::ModifierType,
) -> GtkToolbarFeedback {
    let (ctrl, shift, alt, logo) = gdk_pointer_modifiers(state);
    GtkToolbarFeedback::Key {
        keyval: keyval.into_glib(),
        ctrl,
        shift,
        alt,
        logo,
    }
}

fn focused_keys(widget: &gtk4::Widget) -> FocusedKeys {
    let Some(focus) = widget.root().and_then(|root| root.focus()) else {
        return FocusedKeys::Nothing;
    };

    let editing = focus.ancestor(gtk4::Entry::static_type()).is_some()
        || focus.accessible_role() == gtk4::AccessibleRole::Slider;
    if editing {
        FocusedKeys::Editing
    } else {
        FocusedKeys::Activation
    }
}

/// Forward key presses that reach `widget` to the overlay.
///
/// Installed in the capture phase on the toolbar window and on each popover
/// (a popover is its own GTK native), so the relay decides before GTK's own
/// key bindings consume a press.
pub(in crate::toolbar_gtk) fn install_key_relay(
    widget: &impl IsA<gtk4::Widget>,
    feedback: &FeedbackSender,
) {
    let key = gtk4::EventControllerKey::new();
    key.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let feedback = feedback.clone();
    key.connect_key_pressed(move |controller, keyval, _, state| {
        let is_modifier = controller
            .current_event()
            .and_then(|event| event.downcast::<gtk4::gdk::KeyEvent>().ok())
            .is_some_and(|event| event.is_modifier());
        let focus = controller
            .widget()
            .map_or(FocusedKeys::Nothing, |widget| focused_keys(&widget));
        if key_stays_local(keyval, is_modifier, focus) {
            return gtk4::glib::Propagation::Proceed;
        }

        let _ = feedback.send(forwarded_key_feedback(keyval, state));
        gtk4::glib::Propagation::Stop
    });
    widget.as_ref().add_controller(key);
}

/// The relay controller installed on `widget`, if any.
#[cfg(test)]
pub(in crate::toolbar_gtk) fn key_relay_controller(
    widget: &impl IsA<gtk4::Widget>,
) -> Option<gtk4::EventControllerKey> {
    let controllers = widget.as_ref().observe_controllers();
    (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .filter_map(|controller| controller.downcast::<gtk4::EventControllerKey>().ok())
        .find(|controller| controller.propagation_phase() == gtk4::PropagationPhase::Capture)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk4::gdk::{Key, ModifierType};

    #[test]
    fn shortcut_and_escape_keys_leave_the_toolbar() {
        for keyval in [Key::Escape, Key::s, Key::S, Key::z, Key::F1, Key::space] {
            assert!(
                !key_stays_local(keyval, false, FocusedKeys::Nothing),
                "{keyval:?} reaches the overlay"
            );
        }
    }

    #[test]
    fn modifiers_and_tab_stay_with_gtk() {
        assert!(key_stays_local(Key::Control_L, true, FocusedKeys::Nothing));
        assert!(key_stays_local(Key::Tab, false, FocusedKeys::Nothing));
        assert!(key_stays_local(
            Key::ISO_Left_Tab,
            false,
            FocusedKeys::Nothing
        ));
    }

    #[test]
    fn a_focused_control_keeps_only_its_activation_keys() {
        assert!(key_stays_local(Key::space, false, FocusedKeys::Activation));
        assert!(key_stays_local(Key::Return, false, FocusedKeys::Activation));
        assert!(!key_stays_local(
            Key::Escape,
            false,
            FocusedKeys::Activation
        ));
        assert!(!key_stays_local(Key::v, false, FocusedKeys::Activation));
    }

    #[test]
    fn editing_widgets_keep_every_key() {
        for keyval in [Key::Escape, Key::a, Key::Left, Key::BackSpace] {
            assert!(key_stays_local(keyval, false, FocusedKeys::Editing));
        }
    }

    #[test]
    fn forwarded_feedback_carries_the_keyval_and_modifiers() {
        assert_eq!(
            forwarded_key_feedback(Key::z, ModifierType::CONTROL_MASK),
            GtkToolbarFeedback::Key {
                keyval: Key::z.into_glib(),
                ctrl: true,
                shift: false,
                alt: false,
                logo: false,
            }
        );
    }
}
