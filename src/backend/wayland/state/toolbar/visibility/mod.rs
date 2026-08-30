use super::*;

mod access;
mod pointer;
mod sync;

#[derive(Debug, Clone, Copy)]
struct KeyboardInteractivityPolicyInput {
    keyboard_release_requested: bool,
    main_layer_focus_acquiring: bool,
    layer_shell_available: bool,
    separate_toolbar_visible: bool,
    inline_toolbars_active: bool,
    canvas_modal_active: bool,
}

fn keyboard_interactivity_for(input: KeyboardInteractivityPolicyInput) -> KeyboardInteractivity {
    if input.keyboard_release_requested {
        KeyboardInteractivity::None
    } else if input.main_layer_focus_acquiring {
        KeyboardInteractivity::Exclusive
    } else if input.layer_shell_available
        && input.separate_toolbar_visible
        && !input.inline_toolbars_active
        && !input.canvas_modal_active
    {
        KeyboardInteractivity::OnDemand
    } else {
        KeyboardInteractivity::Exclusive
    }
}

#[cfg(test)]
mod tests;
