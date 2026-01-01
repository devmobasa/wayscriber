use super::*;

mod access;
mod pointer;
mod sync;

fn desired_keyboard_interactivity_for(
    layer_shell_available: bool,
    toolbar_visible: bool,
) -> KeyboardInteractivity {
    if layer_shell_available && toolbar_visible {
        KeyboardInteractivity::OnDemand
    } else {
        KeyboardInteractivity::Exclusive
    }
}

#[cfg(test)]
mod tests;
