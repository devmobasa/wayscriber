use smithay_client_toolkit::shm::{Shm, ShmHandler};

use super::super::AboutWindowState;

impl ShmHandler for AboutWindowState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
