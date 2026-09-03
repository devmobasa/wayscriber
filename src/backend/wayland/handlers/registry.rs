// Hooks up smithay's registry bookkeeping so other handler modules can request globals.
use smithay_client_toolkit::{
    output::OutputState,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::SeatState,
};

use super::super::state::WaylandState;

impl ProvidesRegistryState for WaylandState {
    fn registry(&mut self) -> &mut RegistryState {
        self.protocol.registry_mut()
    }

    registry_handlers![OutputState, SeatState];
}
