use wayland_client::{EventQueue, backend::WaylandError};

use super::super::super::state::WaylandState;
use super::super::helpers::dispatch_with_timeout;

pub(super) fn dispatch_events(
    event_queue: &mut EventQueue<WaylandState>,
    state: &mut WaylandState,
    capture_active: bool,
    animation_timeout: Option<std::time::Duration>,
) -> Result<(), anyhow::Error> {
    if capture_active {
        if let Err(e) = event_queue.dispatch_pending(state) {
            return Err(anyhow::anyhow!("Wayland event queue error: {}", e));
        }

        if let Err(e) = event_queue.flush() {
            return Err(anyhow::anyhow!("Wayland flush error: {}", e));
        }

        if let Some(guard) = event_queue.prepare_read() {
            match guard.read() {
                Ok(_) => {
                    if let Err(e) = event_queue.dispatch_pending(state) {
                        return Err(anyhow::anyhow!("Wayland event queue error: {}", e));
                    }
                }
                Err(WaylandError::Io(err)) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(err) => {
                    return Err(anyhow::anyhow!("Wayland read error: {}", err));
                }
            }
        }

        Ok(())
    } else {
        dispatch_with_timeout(event_queue, state, animation_timeout)
            .map_err(|e| anyhow::anyhow!("Wayland event queue error: {}", e))
    }
}
