// Exposes the shared-memory instance backing our Cairo buffers.
use smithay_client_toolkit::shm::{Shm, ShmHandler};

use super::super::state::WaylandState;

impl ShmHandler for WaylandState {
    fn shm_state(&mut self) -> &mut Shm {
        self.protocol.shm_mut()
    }
}
