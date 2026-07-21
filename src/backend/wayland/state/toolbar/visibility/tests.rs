use super::*;

#[test]
fn desired_keyboard_interactivity_requires_layer_shell_and_layer_toolbars() {
    assert_eq!(
        desired_keyboard_interactivity_for(true, true, false, false),
        KeyboardInteractivity::OnDemand
    );
    assert_eq!(
        desired_keyboard_interactivity_for(true, false, false, false),
        KeyboardInteractivity::Exclusive
    );
    assert_eq!(
        desired_keyboard_interactivity_for(false, true, false, false),
        KeyboardInteractivity::Exclusive
    );
    assert_eq!(
        desired_keyboard_interactivity_for(true, true, true, false),
        KeyboardInteractivity::Exclusive
    );
    assert_eq!(
        desired_keyboard_interactivity_for(true, true, false, true),
        KeyboardInteractivity::Exclusive
    );
}
