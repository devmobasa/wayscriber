use crate::config::Config;
use crate::input::InputState;

pub(super) fn build_input_state(config: &Config) -> InputState {
    InputState::from_config(config)
}
