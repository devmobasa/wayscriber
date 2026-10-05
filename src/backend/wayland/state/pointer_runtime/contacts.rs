//! Motion and release ownership are independent and bound by the real press route.
use crate::backend::wayland::state::ContactOwner;
use crate::input::{InputState, state::interaction::RoutingOutcome};

#[derive(Debug)]
struct HeldContact {
    button: u32,
    owner: ContactOwner,
    motion_active: bool,
    release_active: bool,
    canvas_press: Option<RoutingOutcome>,
}

#[derive(Debug, Default)]
pub(super) struct PointerContacts {
    held: Vec<HeldContact>,
}

impl PointerContacts {
    pub(super) fn bind(&mut self, button: u32, owner: ContactOwner) {
        self.held.retain(|contact| contact.button != button);
        self.held.push(HeldContact {
            button,
            owner,
            motion_active: true,
            release_active: true,
            canvas_press: None,
        });
    }

    pub(super) fn bind_canvas(&mut self, button: u32, outcome: RoutingOutcome, input: &InputState) {
        self.bind(button, ContactOwner::Canvas);
        let contact = self.held.last_mut().expect("bound contact");
        contact.canvas_press = Some(outcome);
        contact.motion_active = outcome.owns_pointer_motion(input);
        contact.release_active = outcome.owns_pointer_release(input);
    }

    pub(super) fn take(&mut self, button: u32, input: &InputState) -> Option<(ContactOwner, bool)> {
        let index = self
            .held
            .iter()
            .position(|contact| contact.button == button)?;
        let contact = self.held.remove(index);
        let release = contact.release_active
            && contact
                .canvas_press
                .is_none_or(|outcome| outcome.owns_pointer_release(input));
        Some((contact.owner, release))
    }

    pub(super) fn motion_owner(&self) -> Option<ContactOwner> {
        self.held
            .iter()
            .find(|contact| contact.motion_active)
            .map(|contact| contact.owner)
    }

    pub(super) fn reconcile(
        &mut self,
        input: &InputState,
        zoom_pan: bool,
        board_pan: bool,
        toolbar_drag: bool,
    ) {
        for contact in &mut self.held {
            if let Some(outcome) = contact.canvas_press {
                contact.motion_active &= outcome.owns_pointer_motion(input);
                contact.release_active &= outcome.owns_pointer_release(input);
            } else {
                let active = match contact.owner {
                    ContactOwner::Canvas => input.has_active_pointer_interaction(),
                    ContactOwner::ZoomPan => zoom_pan,
                    ContactOwner::BoardPan => board_pan,
                    ContactOwner::InlineToolbar => toolbar_drag,
                };
                contact.motion_active &= active;
                contact.release_active &= active;
            }
        }
    }

    pub(super) fn clear(&mut self) {
        self.held.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrelated_releases_and_rebinds_preserve_the_first_held_contact() {
        let mut contacts = PointerContacts::default();
        let input = crate::input::state::test_support::make_test_input_state();
        let canvas = ContactOwner::Canvas;
        contacts.bind(1, canvas);
        contacts.bind(2, ContactOwner::InlineToolbar);
        assert_eq!(contacts.take(3, &input), None);
        assert_eq!(contacts.motion_owner(), Some(canvas));
        assert_eq!(
            contacts.take(2, &input),
            Some((ContactOwner::InlineToolbar, true))
        );
        assert_eq!(contacts.motion_owner(), Some(canvas));
        contacts.bind(1, ContactOwner::BoardPan);
        assert_eq!(contacts.motion_owner(), Some(ContactOwner::BoardPan));
        assert_eq!(
            contacts.take(1, &input),
            Some((ContactOwner::BoardPan, true))
        );
        assert_eq!(contacts.motion_owner(), None);
    }
}

#[cfg(test)]
mod dispatch_regressions {
    use crate::backend::wayland::state::pointer_runtime::PointerRuntime;
    use crate::backend::wayland::state::{ContactMotion, ContactOwner};
    use crate::draw::TextMeasurer;
    use crate::input::{
        MouseButton,
        state::{InputTextResources, test_support::make_test_input_state},
    };
    use crate::ui_text::UiTextEngine;

    #[test]
    fn inert_canvas_presses_cannot_inherit_another_devices_canvas_gesture() {
        for mouse_button in [MouseButton::Right, MouseButton::Middle] {
            let mut pointer = PointerRuntime::new();
            let mut config = crate::config::Config::default();
            config.ui.radial_menu_mouse_binding = crate::config::RadialMenuMouseBinding::Disabled;
            let mut input = crate::input::InputState::from_config(&config);
            let measurer = TextMeasurer::default();
            let engine = UiTextEngine::default();
            let resources = || InputTextResources {
                measurer: &measurer,
                ui_engine: &engine,
            };
            pointer.route_canvas_press(
                &mut input,
                resources(),
                1,
                mouse_button,
                [300, 300, 300, 300],
            );
            assert_eq!(pointer.contact_motion(), ContactMotion::Hover);
            input.close_context_menu();
            // A different device starts drawing after this inert/consumed press.
            input.on_mouse_press_with_canvas_and_resources(
                resources(),
                MouseButton::Left,
                600,
                400,
                600,
                400,
            );
            assert!(input.has_active_pointer_interaction());
            pointer.reconcile_contacts(&input, false, false);
            assert_eq!(pointer.contact_motion(), ContactMotion::Hover);
            assert_eq!(
                pointer.take_contact(1, &input),
                Some((ContactOwner::Canvas, false))
            );
            assert!(input.has_active_pointer_interaction());
        }
    }

    #[test]
    fn canceled_canvas_and_non_drag_strip_contacts_keep_only_their_release_ownership() {
        let mut pointer = PointerRuntime::new();
        let mut input = make_test_input_state();
        let measurer = TextMeasurer::default();
        let engine = UiTextEngine::default();
        pointer.route_canvas_press(
            &mut input,
            InputTextResources {
                measurer: &measurer,
                ui_engine: &engine,
            },
            1,
            MouseButton::Left,
            [600, 400, 600, 400],
        );
        assert_eq!(pointer.contact_motion(), ContactMotion::Canvas);
        input.cancel_active_interaction_with(&measurer);
        pointer.reconcile_contacts(&input, false, false);
        assert_eq!(pointer.contact_motion(), ContactMotion::Hover);
        input.on_mouse_press_with_canvas_and_resources(
            InputTextResources {
                measurer: &measurer,
                ui_engine: &engine,
            },
            MouseButton::Left,
            700,
            500,
            700,
            500,
        );
        assert!(input.has_active_pointer_interaction());
        assert_eq!(
            pointer.take_contact(1, &input),
            Some((ContactOwner::Canvas, false))
        );
        pointer.bind_contact(2, ContactOwner::InlineToolbar);
        pointer.reconcile_contacts(&input, false, false);
        assert_eq!(pointer.contact_motion(), ContactMotion::Hover);
        assert_eq!(
            pointer.take_contact(2, &input),
            Some((ContactOwner::InlineToolbar, false))
        );
        pointer.bind_contact(3, ContactOwner::InlineToolbar);
        pointer.reconcile_contacts(&input, false, true);
        assert_eq!(pointer.contact_motion(), ContactMotion::Toolbar);
        assert_eq!(
            pointer.take_contact(3, &input),
            Some((ContactOwner::InlineToolbar, true))
        );
    }
}

#[cfg(test)]
mod popup_dispatch_regressions {
    use crate::backend::wayland::state::pointer_runtime::PointerRuntime;
    use crate::backend::wayland::state::{ContactMotion, ContactOwner};
    use crate::draw::TextMeasurer;
    use crate::input::{
        MouseButton,
        state::{InputTextResources, test_support::make_test_input_state},
    };
    use crate::ui_text::UiTextEngine;

    #[test]
    fn popup_slider_motion_and_button_releases_keep_their_own_contact() {
        for kind in ["button", "slider", "closed button"] {
            let slider = kind == "slider";
            let mut input = make_test_input_state();
            let mut pointer = PointerRuntime::new();
            let measurer = TextMeasurer::default();
            let engine = UiTextEngine::default();
            input.open_color_picker_popup();
            input.update_color_picker_popup_layout(1280, 720);
            let layout = input.color_picker_popup_layout().unwrap();
            let (x, y) = if slider {
                (layout.hue_x + 2.0, layout.hue_y + 2.0)
            } else {
                (layout.ok_btn_x + 2.0, layout.ok_btn_y + 2.0)
            };
            let coords = [x as i32, y as i32, x as i32, y as i32];
            pointer.route_canvas_press(
                &mut input,
                InputTextResources {
                    measurer: &measurer,
                    ui_engine: &engine,
                },
                1,
                MouseButton::Left,
                coords,
            );
            assert_eq!(
                pointer.contact_motion(),
                if slider {
                    ContactMotion::Canvas
                } else {
                    ContactMotion::Hover
                }
            );
            if kind == "closed button" {
                input.close_color_picker_popup(true);
                input.on_mouse_press_with_canvas_and_resources(
                    InputTextResources {
                        measurer: &measurer,
                        ui_engine: &engine,
                    },
                    MouseButton::Left,
                    700,
                    500,
                    700,
                    500,
                );
                assert!(input.has_active_pointer_interaction());
                assert_eq!(
                    pointer.take_contact(1, &input),
                    Some((ContactOwner::Canvas, false))
                );
                assert!(input.has_active_pointer_interaction());
                continue;
            }
            assert_eq!(
                pointer.take_contact(1, &input),
                Some((ContactOwner::Canvas, true))
            );
            input.on_mouse_release_with_canvas_and_resources(
                InputTextResources {
                    measurer: &measurer,
                    ui_engine: &engine,
                },
                MouseButton::Left,
                coords[0],
                coords[1],
                coords[2],
                coords[3],
            );
            assert!(!input.color_picker_popup_is_dragging());
            assert_eq!(input.is_color_picker_popup_open(), slider);
        }
    }
}
