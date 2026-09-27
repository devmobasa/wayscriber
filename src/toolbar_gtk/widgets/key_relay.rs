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
    /// A slider owns its value-navigation keys, and Escape, which releases
    /// its focus (or closes the popover hosting it) instead of exiting.
    Slider,
    /// A widget that owns typing and arrow keys: the hex entry.
    /// Escape is relayed so dismissal shares the overlay's exit guard.
    Editing,
}

/// Whether a key press stays with GTK instead of going to the overlay.
///
/// Modifier presses stay: their state rides on the next forwarded key. Tab
/// stays so keyboard navigation between popover controls keeps working. A
/// `chord` (Ctrl, Alt, or Super held) is always a shortcut to a slider or an
/// activatable control, so Ctrl+PgUp switches boards even while a slider has
/// focus; the hex entry keeps its editing chords.
pub(super) fn key_stays_local(
    keyval: gtk4::gdk::Key,
    is_modifier: bool,
    chord: bool,
    focus: FocusedKeys,
) -> bool {
    use gtk4::gdk::Key;

    if is_modifier || matches!(keyval, Key::Tab | Key::ISO_Left_Tab | Key::KP_Tab) {
        return true;
    }

    match focus {
        FocusedKeys::Nothing => false,
        FocusedKeys::Activation => {
            !chord
                && matches!(
                    keyval,
                    Key::space | Key::KP_Space | Key::Return | Key::KP_Enter | Key::ISO_Enter
                )
        }
        FocusedKeys::Slider => {
            !chord && (keyval == Key::Escape || super::slider::is_slider_navigation_key(keyval))
        }
        FocusedKeys::Editing => keyval != Key::Escape,
    }
}

/// Whether Ctrl, Alt, or Super is held, which makes a press a shortcut.
fn is_chord(state: gtk4::gdk::ModifierType) -> bool {
    let (ctrl, _shift, alt, logo) = gdk_pointer_modifiers(state);

    ctrl || alt || logo
}

/// Whether this press is an Escape the focused widget spends on its own
/// dismissal. The overlay still has to hear about it, so a second Escape
/// right behind it does not exit.
pub(super) fn escape_dismisses_locally(keyval: gtk4::gdk::Key, focus: FocusedKeys) -> bool {
    focus == FocusedKeys::Slider && keyval == gtk4::gdk::Key::Escape
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

/// Whether the focused widget lives in a popover. Escape there closes the
/// popover and the toolbar window keeps the keyboard; outside one it drops
/// the toolbar's keyboard focus altogether.
fn focus_in_popover(widget: &gtk4::Widget) -> bool {
    widget
        .root()
        .and_then(|root| root.focus())
        .is_some_and(|focus| focus.ancestor(gtk4::Popover::static_type()).is_some())
}

fn focused_keys(widget: &gtk4::Widget) -> FocusedKeys {
    let Some(focus) = widget.root().and_then(|root| root.focus()) else {
        return FocusedKeys::Nothing;
    };

    if focus.accessible_role() == gtk4::AccessibleRole::Slider {
        FocusedKeys::Slider
    } else if focus.ancestor(gtk4::Entry::static_type()).is_some() {
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
        if key_stays_local(keyval, is_modifier, is_chord(state), focus) {
            if escape_dismisses_locally(keyval, focus) {
                let released_keyboard = controller
                    .widget()
                    .is_some_and(|widget| !focus_in_popover(&widget));
                let _ = feedback.send(GtkToolbarFeedback::EscapeDismissed { released_keyboard });
            }
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
                !key_stays_local(keyval, false, false, FocusedKeys::Nothing),
                "{keyval:?} reaches the overlay"
            );
        }
    }

    #[test]
    fn modifiers_and_tab_stay_with_gtk() {
        assert!(key_stays_local(
            Key::Control_L,
            true,
            false,
            FocusedKeys::Nothing
        ));
        assert!(key_stays_local(
            Key::Tab,
            false,
            false,
            FocusedKeys::Nothing
        ));
        assert!(key_stays_local(
            Key::ISO_Left_Tab,
            false,
            false,
            FocusedKeys::Nothing
        ));
    }

    #[test]
    fn a_focused_control_keeps_only_its_activation_keys() {
        assert!(key_stays_local(
            Key::space,
            false,
            false,
            FocusedKeys::Activation
        ));
        assert!(key_stays_local(
            Key::Return,
            false,
            false,
            FocusedKeys::Activation
        ));
        assert!(!key_stays_local(
            Key::Escape,
            false,
            false,
            FocusedKeys::Activation
        ));
        assert!(!key_stays_local(
            Key::v,
            false,
            false,
            FocusedKeys::Activation
        ));
    }

    #[test]
    fn focused_slider_relays_shortcuts_but_keeps_navigation() {
        for keyval in [Key::h, Key::w, Key::space, Key::Return] {
            assert!(!key_stays_local(keyval, false, false, FocusedKeys::Slider));
        }
        for keyval in [
            Key::Left,
            Key::Right,
            Key::Up,
            Key::Down,
            Key::Home,
            Key::End,
            Key::Page_Up,
            Key::Page_Down,
        ] {
            assert!(key_stays_local(keyval, false, false, FocusedKeys::Slider));
        }
    }

    /// Ctrl+PgUp/PgDn switch boards and Ctrl+Arrows are shortcuts too, so a
    /// focused slider must not keep them just because the bare key is its own.
    /// Shift alone is not a chord; the keypad arrows are slider keys.
    #[test]
    fn chords_leave_a_focused_slider_for_the_overlay() {
        for keyval in [
            Key::Page_Up,
            Key::Page_Down,
            Key::Left,
            Key::Right,
            Key::Home,
            Key::End,
            Key::Escape,
        ] {
            assert!(
                !key_stays_local(keyval, false, true, FocusedKeys::Slider),
                "a chord on {keyval:?} reaches the overlay"
            );
        }
        assert!(!key_stays_local(
            Key::Return,
            false,
            true,
            FocusedKeys::Activation
        ));

        assert!(key_stays_local(
            Key::Left,
            false,
            false,
            FocusedKeys::Slider
        ));
        assert!(key_stays_local(
            Key::KP_Left,
            false,
            false,
            FocusedKeys::Slider
        ));
        assert!(key_stays_local(
            Key::KP_Page_Up,
            false,
            false,
            FocusedKeys::Slider
        ));
        assert!(key_stays_local(Key::a, false, true, FocusedKeys::Editing));
    }

    #[test]
    fn only_ctrl_alt_and_super_make_a_chord() {
        use gtk4::gdk::ModifierType;

        assert!(!is_chord(ModifierType::empty()));
        assert!(!is_chord(ModifierType::SHIFT_MASK));
        assert!(is_chord(ModifierType::CONTROL_MASK));
        assert!(is_chord(ModifierType::ALT_MASK));
        assert!(is_chord(ModifierType::SUPER_MASK));
    }

    /// Escape on a Tab-focused slider releases the slider's focus instead of
    /// reaching the overlay, where no open menu would turn it into Exit. The
    /// overlay is still told, so its guard swallows a second Escape.
    #[test]
    fn escape_on_a_focused_slider_dismisses_locally_and_arms_the_guard() {
        assert!(key_stays_local(
            Key::Escape,
            false,
            false,
            FocusedKeys::Slider
        ));
        assert!(escape_dismisses_locally(Key::Escape, FocusedKeys::Slider));

        assert!(!escape_dismisses_locally(Key::Left, FocusedKeys::Slider));
        for focus in [
            FocusedKeys::Nothing,
            FocusedKeys::Activation,
            FocusedKeys::Editing,
        ] {
            assert!(!escape_dismisses_locally(Key::Escape, focus));
        }
    }

    #[test]
    fn editing_widgets_keep_typing_keys_and_relay_escape() {
        assert!(!key_stays_local(
            Key::Escape,
            false,
            false,
            FocusedKeys::Editing
        ));
        for keyval in [Key::a, Key::Left, Key::BackSpace] {
            assert!(key_stays_local(keyval, false, false, FocusedKeys::Editing));
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
