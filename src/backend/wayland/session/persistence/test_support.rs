//! Controlled channel peer for session driver tests, not a second command driver.
use super::{worker::execute, *};

impl PersistenceController {
    pub(in crate::backend::wayland) fn controlled_for_test() -> (Self, ControlledPersistenceWorker)
    {
        let (request_tx, requests) = mpsc::sync_channel(1);
        let (completions, completion_rx) = mpsc::sync_channel(1);

        (
            Self {
                request_tx: Some(request_tx),
                completion_rx,
                worker: None,
                active_id: None,
                next_sequence: 0,
                healthy: true,
            },
            ControlledPersistenceWorker {
                requests,
                completions,
            },
        )
    }
}

pub(in crate::backend::wayland) struct ControlledPersistenceWorker {
    requests: Receiver<PersistenceRequest>,
    completions: SyncSender<PersistenceCompletion>,
}

impl ControlledPersistenceWorker {
    pub(in crate::backend::wayland) fn complete_next(&self) {
        self.respond_with(execute);
    }

    pub(in crate::backend::wayland) fn respond_with(
        &self,
        result: impl FnOnce(PersistenceOperation) -> Result<PersistenceOutcome>,
    ) {
        let request = self
            .requests
            .recv_timeout(Duration::from_secs(5))
            .expect("driver submitted work");
        let completion = PersistenceCompletion {
            id: request.id,
            result: result(request.operation),
            queue_wait: Duration::ZERO,
            execution_time: Duration::ZERO,
            worker_thread_id: thread::current().id(),
            finished_at: Instant::now(),
        };

        self.completions.send(completion).unwrap();
    }

    pub(in crate::backend::wayland) fn has_request(&self) -> bool {
        match self.requests.try_recv() {
            Err(TryRecvError::Empty) => false,
            other => panic!("unexpected worker request: {other:?}"),
        }
    }
}
