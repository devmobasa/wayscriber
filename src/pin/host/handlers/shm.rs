use smithay_client_toolkit::shm::{Shm, ShmHandler};

use super::super::PinHost;

impl ShmHandler for PinHost {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
