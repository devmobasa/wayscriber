use std::time::Duration;

use wayland_client::{EventQueue, backend::WaylandError};

use super::super::super::state::WaylandState;
use super::super::helpers::dispatch_with_timeout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureReadOutcome {
    Readable,
    WouldBlock,
}

trait CaptureDispatchOps {
    fn dispatch_pending(&mut self) -> Result<(), anyhow::Error>;
    fn flush(&mut self) -> Result<(), anyhow::Error>;
    fn prepare_read(&mut self) -> Result<Option<CaptureReadOutcome>, anyhow::Error>;
}

struct RealCaptureDispatchOps<'a> {
    event_queue: &'a mut EventQueue<WaylandState>,
    state: &'a mut WaylandState,
}

impl CaptureDispatchOps for RealCaptureDispatchOps<'_> {
    fn dispatch_pending(&mut self) -> Result<(), anyhow::Error> {
        self.event_queue
            .dispatch_pending(self.state)
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("Wayland event queue error: {}", e))
    }

    fn flush(&mut self) -> Result<(), anyhow::Error> {
        self.event_queue
            .flush()
            .map_err(|e| anyhow::anyhow!("Wayland flush error: {}", e))
    }

    fn prepare_read(&mut self) -> Result<Option<CaptureReadOutcome>, anyhow::Error> {
        let Some(guard) = self.event_queue.prepare_read() else {
            return Ok(None);
        };

        match guard.read() {
            Ok(_) => Ok(Some(CaptureReadOutcome::Readable)),
            Err(WaylandError::Io(err)) if err.kind() == std::io::ErrorKind::WouldBlock => {
                Ok(Some(CaptureReadOutcome::WouldBlock))
            }
            Err(err) => Err(anyhow::anyhow!("Wayland read error: {}", err)),
        }
    }
}

fn dispatch_capture_active(ops: &mut impl CaptureDispatchOps) -> Result<(), anyhow::Error> {
    ops.dispatch_pending()?;
    ops.flush()?;

    if matches!(ops.prepare_read()?, Some(CaptureReadOutcome::Readable)) {
        ops.dispatch_pending()?;
    }

    Ok(())
}

pub(super) fn dispatch_events(
    event_queue: &mut EventQueue<WaylandState>,
    state: &mut WaylandState,
    capture_active: bool,
    animation_timeout: Option<Duration>,
) -> Result<(), anyhow::Error> {
    if capture_active {
        let mut ops = RealCaptureDispatchOps { event_queue, state };
        dispatch_capture_active(&mut ops)
    } else {
        dispatch_with_timeout(event_queue, state, animation_timeout)
            .map_err(|e| anyhow::anyhow!("Wayland event queue error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeCaptureDispatchOps {
        dispatch_calls: usize,
        flush_calls: usize,
        prepare_calls: usize,
        dispatch_error_on_call: Option<usize>,
        flush_error: Option<anyhow::Error>,
        prepare_result: Result<Option<CaptureReadOutcome>, anyhow::Error>,
    }

    impl FakeCaptureDispatchOps {
        fn new(prepare_result: Result<Option<CaptureReadOutcome>, anyhow::Error>) -> Self {
            Self {
                dispatch_calls: 0,
                flush_calls: 0,
                prepare_calls: 0,
                dispatch_error_on_call: None,
                flush_error: None,
                prepare_result,
            }
        }
    }

    impl CaptureDispatchOps for FakeCaptureDispatchOps {
        fn dispatch_pending(&mut self) -> Result<(), anyhow::Error> {
            self.dispatch_calls += 1;
            if self.dispatch_error_on_call == Some(self.dispatch_calls) {
                return Err(anyhow::anyhow!("dispatch failed"));
            }
            Ok(())
        }

        fn flush(&mut self) -> Result<(), anyhow::Error> {
            self.flush_calls += 1;
            if let Some(err) = self.flush_error.take() {
                return Err(err);
            }
            Ok(())
        }

        fn prepare_read(&mut self) -> Result<Option<CaptureReadOutcome>, anyhow::Error> {
            self.prepare_calls += 1;
            match &self.prepare_result {
                Ok(value) => Ok(*value),
                Err(err) => Err(anyhow::anyhow!(err.to_string())),
            }
        }
    }

    #[test]
    fn capture_dispatch_reads_and_dispatches_again() {
        let mut ops = FakeCaptureDispatchOps::new(Ok(Some(CaptureReadOutcome::Readable)));
        dispatch_capture_active(&mut ops).unwrap();

        assert_eq!(ops.dispatch_calls, 2);
        assert_eq!(ops.flush_calls, 1);
        assert_eq!(ops.prepare_calls, 1);
    }

    #[test]
    fn capture_dispatch_would_block_skips_second_dispatch() {
        let mut ops = FakeCaptureDispatchOps::new(Ok(Some(CaptureReadOutcome::WouldBlock)));
        dispatch_capture_active(&mut ops).unwrap();

        assert_eq!(ops.dispatch_calls, 1);
        assert_eq!(ops.flush_calls, 1);
        assert_eq!(ops.prepare_calls, 1);
    }

    #[test]
    fn capture_dispatch_without_prepared_read_dispatches_once() {
        let mut ops = FakeCaptureDispatchOps::new(Ok(None));
        dispatch_capture_active(&mut ops).unwrap();

        assert_eq!(ops.dispatch_calls, 1);
        assert_eq!(ops.flush_calls, 1);
        assert_eq!(ops.prepare_calls, 1);
    }

    #[test]
    fn capture_dispatch_propagates_flush_error() {
        let mut ops = FakeCaptureDispatchOps::new(Ok(None));
        ops.flush_error = Some(anyhow::anyhow!("flush failed"));

        let err = dispatch_capture_active(&mut ops).unwrap_err();
        assert!(err.to_string().contains("flush failed"));
        assert_eq!(ops.dispatch_calls, 1);
        assert_eq!(ops.prepare_calls, 0);
    }

    #[test]
    fn capture_dispatch_propagates_read_error() {
        let mut ops = FakeCaptureDispatchOps::new(Err(anyhow::anyhow!("read failed")));

        let err = dispatch_capture_active(&mut ops).unwrap_err();
        assert!(err.to_string().contains("read failed"));
        assert_eq!(ops.dispatch_calls, 1);
        assert_eq!(ops.flush_calls, 1);
        assert_eq!(ops.prepare_calls, 1);
    }

    #[test]
    fn capture_dispatch_propagates_second_dispatch_error() {
        let mut ops = FakeCaptureDispatchOps::new(Ok(Some(CaptureReadOutcome::Readable)));
        ops.dispatch_error_on_call = Some(2);

        let err = dispatch_capture_active(&mut ops).unwrap_err();
        assert!(err.to_string().contains("dispatch failed"));
        assert_eq!(ops.dispatch_calls, 2);
        assert_eq!(ops.flush_calls, 1);
        assert_eq!(ops.prepare_calls, 1);
    }
}
