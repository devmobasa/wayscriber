use super::*;

#[test]
fn desired_keyboard_interactivity_requires_layer_shell_and_visibility() {
    assert_eq!(
        desired_keyboard_interactivity_for(true, true),
        KeyboardInteractivity::OnDemand
    );
    assert_eq!(
        desired_keyboard_interactivity_for(true, false),
        KeyboardInteractivity::Exclusive
    );
    assert_eq!(
        desired_keyboard_interactivity_for(false, true),
        KeyboardInteractivity::Exclusive
    );
}
