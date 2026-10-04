#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};

use crate::backend::wayland::backend::runtime_wake::RuntimeWakeHandle;
#[cfg(test)]
use crate::backend::wayland::backend::runtime_wake::RuntimeWakeSource;
use crate::session::{
    self, ClearToolStateOutcome, LoadSnapshotOutcome, SaveAsOverwrite, SaveSnapshotReport,
    SessionInspection, SessionOptions, SessionSnapshot,
};

mod worker;
use worker::worker_main;

#[cfg(test)]
use worker::save_as_preflight_after_validation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) struct RequestId {
    pub(in crate::backend::wayland) target_epoch: u64,
    pub(in crate::backend::wayland) sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum SaveStrategy {
    Autosave,
    Normal,
}

#[derive(Debug)]
pub(in crate::backend::wayland) enum PersistenceOperation {
    Save {
        snapshot: SessionSnapshot,
        options: SessionOptions,
        strategy: SaveStrategy,
        contentless_clear_boundary: bool,
    },
    SaveAs {
        snapshot: SessionSnapshot,
        options: SessionOptions,
        overwrite: SaveAsOverwrite,
    },
    LoadConfigured {
        options: SessionOptions,
    },
    LoadNamedCandidate {
        options: SessionOptions,
    },
    /// Loads a remembered session file only while it is still a usable
    /// session file, before and after it loads.
    LoadRemembered {
        options: SessionOptions,
    },
    Inspect {
        options: SessionOptions,
    },
    SaveAsOverwritePreflight {
        current_path: PathBuf,
        options: SessionOptions,
    },
    ValidateNamedOpen {
        path: PathBuf,
    },
    ClearToolState {
        options: SessionOptions,
    },
    HasArtifacts {
        options: SessionOptions,
    },
    RecordNamedOpened {
        options: SessionOptions,
    },
    ForgetNamedSessionByPath {
        path: PathBuf,
    },
    #[cfg(test)]
    PanicForTest,
    Shutdown,
}

impl PersistenceOperation {
    fn label(&self) -> &'static str {
        match self {
            Self::Save {
                strategy: SaveStrategy::Autosave,
                ..
            } => "autosave",
            Self::Save {
                strategy: SaveStrategy::Normal,
                ..
            } => "save",
            Self::SaveAs { .. } => "save-as",
            Self::LoadConfigured { .. } => "load-configured",
            Self::LoadNamedCandidate { .. } => "load-named-candidate",
            Self::LoadRemembered { .. } => "load-remembered",
            Self::Inspect { .. } => "inspect",
            Self::SaveAsOverwritePreflight { .. } => "save-as-overwrite-preflight",
            Self::ValidateNamedOpen { .. } => "validate-named-open",
            Self::ClearToolState { .. } => "clear-tool-state",
            Self::HasArtifacts { .. } => "has-artifacts",
            Self::RecordNamedOpened { .. } => "record-named-opened",
            Self::ForgetNamedSessionByPath { .. } => "forget-named-session",
            #[cfg(test)]
            Self::PanicForTest => "panic-for-test",
            Self::Shutdown => "shutdown",
        }
    }
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct SaveCompletion {
    pub(in crate::backend::wayland) report: Option<SaveSnapshotReport>,
    pub(in crate::backend::wayland) committed_board_data: bool,
}

impl SaveCompletion {
    pub(in crate::backend::wayland) fn committed(&self) -> bool {
        self.report.is_some()
    }
}

#[derive(Debug)]
pub(in crate::backend::wayland) enum PersistenceOutcome {
    Save(SaveCompletion),
    SaveAs {
        report: SaveSnapshotReport,
        committed_board_data: bool,
    },
    Load(LoadSnapshotOutcome),
    /// The remembered session file is gone or can no longer be used.
    RememberedUnavailable(anyhow::Error),
    Inspection(SessionInspection),
    SaveAsPreflight {
        same_target: bool,
        overwrite_required: bool,
    },
    ToolStateCleared(ClearToolStateOutcome),
    HasArtifacts(bool),
    CatalogForgotten(bool),
    Unit,
}

#[derive(Debug)]
struct PersistenceRequest {
    id: RequestId,
    queued_at: Instant,
    operation: PersistenceOperation,
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct PersistenceCompletion {
    pub(in crate::backend::wayland) id: RequestId,
    pub(in crate::backend::wayland) result: Result<PersistenceOutcome>,
    pub(in crate::backend::wayland) queue_wait: Duration,
    pub(in crate::backend::wayland) execution_time: Duration,
    pub(in crate::backend::wayland) worker_thread_id: thread::ThreadId,
    pub(in crate::backend::wayland) finished_at: Instant,
}

#[derive(Debug)]
pub(in crate::backend::wayland) enum SubmitError {
    Busy,
    Full,
    Disconnected,
    Unhealthy,
}

impl std::fmt::Display for SubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Busy => "a persistence operation is already active",
            Self::Full => "persistence request channel unexpectedly full",
            Self::Disconnected => "persistence worker disconnected",
            Self::Unhealthy => "persistence worker is unhealthy",
        };
        f.write_str(message)
    }
}

impl std::error::Error for SubmitError {}

#[derive(Debug)]
pub(in crate::backend::wayland) struct SubmitFailure {
    pub(in crate::backend::wayland) error: SubmitError,
    pub(in crate::backend::wayland) operation: Box<PersistenceOperation>,
}

pub(in crate::backend::wayland) struct PersistenceController {
    request_tx: Option<SyncSender<PersistenceRequest>>,
    completion_rx: Receiver<PersistenceCompletion>,
    worker: Option<JoinHandle<()>>,
    active_id: Option<RequestId>,
    next_sequence: u64,
    healthy: bool,
}

impl PersistenceController {
    pub(in crate::backend::wayland) fn start(wake: RuntimeWakeHandle) -> Result<Self> {
        assert_send_static::<SessionSnapshot>();
        assert_send_static::<LoadSnapshotOutcome>();
        assert_send_static::<PersistenceOperation>();
        assert_send_static::<PersistenceOutcome>();

        let (request_tx, request_rx) = mpsc::sync_channel(1);
        let (completion_tx, completion_rx) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("wayscriber-persistence".to_string())
            .spawn(move || worker_main(request_rx, completion_tx, wake))
            .context("failed to start session persistence worker")?;

        Ok(Self {
            request_tx: Some(request_tx),
            completion_rx,
            worker: Some(worker),
            active_id: None,
            next_sequence: 0,
            healthy: true,
        })
    }

    #[cfg(test)]
    pub(in crate::backend::wayland) fn start_for_test() -> Result<Self> {
        let wake = RuntimeWakeSource::new().context("failed to create test runtime wake source")?;
        Self::start(wake.handle())
    }

    pub(in crate::backend::wayland) fn is_active(&self) -> bool {
        self.active_id.is_some()
    }

    pub(in crate::backend::wayland) fn is_healthy(&self) -> bool {
        self.healthy
    }

    pub(in crate::backend::wayland) fn is_stopped(&self) -> bool {
        self.worker.is_none()
    }

    fn next_id(&mut self, target_epoch: u64) -> RequestId {
        self.next_sequence = self.next_sequence.wrapping_add(1);
        RequestId {
            target_epoch,
            sequence: self.next_sequence,
        }
    }

    pub(in crate::backend::wayland) fn try_submit(
        &mut self,
        target_epoch: u64,
        operation: PersistenceOperation,
    ) -> std::result::Result<RequestId, SubmitFailure> {
        if !self.healthy {
            return Err(SubmitFailure {
                error: SubmitError::Unhealthy,
                operation: Box::new(operation),
            });
        }
        if self.active_id.is_some() {
            return Err(SubmitFailure {
                error: SubmitError::Busy,
                operation: Box::new(operation),
            });
        }
        let id = self.next_id(target_epoch);
        let request = PersistenceRequest {
            id,
            queued_at: Instant::now(),
            operation,
        };
        let Some(sender) = self.request_tx.as_ref() else {
            self.healthy = false;
            return Err(SubmitFailure {
                error: SubmitError::Disconnected,
                operation: Box::new(request.operation),
            });
        };
        match sender.try_send(request) {
            Ok(()) => {
                self.active_id = Some(id);
                Ok(id)
            }
            Err(TrySendError::Full(request)) => {
                self.healthy = false;
                Err(SubmitFailure {
                    error: SubmitError::Full,
                    operation: Box::new(request.operation),
                })
            }
            Err(TrySendError::Disconnected(request)) => {
                self.healthy = false;
                Err(SubmitFailure {
                    error: SubmitError::Disconnected,
                    operation: Box::new(request.operation),
                })
            }
        }
    }

    pub(in crate::backend::wayland) fn run(
        &mut self,
        target_epoch: u64,
        operation: PersistenceOperation,
    ) -> Result<PersistenceOutcome> {
        if self.active_id.is_some() {
            return Err(anyhow!(SubmitError::Busy));
        }
        if !self.healthy {
            return Err(anyhow!(SubmitError::Unhealthy));
        }
        let id = self.next_id(target_epoch);
        let request = PersistenceRequest {
            id,
            queued_at: Instant::now(),
            operation,
        };
        let sender = self
            .request_tx
            .as_ref()
            .ok_or_else(|| anyhow!(SubmitError::Disconnected))?;
        sender.send(request).map_err(|err| {
            self.healthy = false;
            anyhow!("failed to submit persistence operation: {err}")
        })?;
        self.active_id = Some(id);
        let completion = self.receive_active(true)?.ok_or_else(|| {
            self.healthy = false;
            anyhow!("persistence worker returned no completion")
        })?;
        completion.result
    }

    pub(in crate::backend::wayland) fn try_receive(
        &mut self,
    ) -> Result<Option<PersistenceCompletion>> {
        self.receive_active(false)
    }

    pub(in crate::backend::wayland) fn wait_for_completion(
        &mut self,
    ) -> Result<Option<PersistenceCompletion>> {
        self.receive_active(true)
    }

    fn receive_active(&mut self, block: bool) -> Result<Option<PersistenceCompletion>> {
        let Some(expected) = self.active_id else {
            return Ok(None);
        };
        let received = if block {
            Some(self.completion_rx.recv().map_err(|err| {
                self.healthy = false;
                self.active_id = None;
                anyhow!("persistence worker disconnected before completion: {err}")
            })?)
        } else {
            match self.completion_rx.try_recv() {
                Ok(completion) => Some(completion),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    self.healthy = false;
                    self.active_id = None;
                    return Err(anyhow!(
                        "persistence worker disconnected before publishing completion"
                    ));
                }
            }
        };
        let Some(completion) = received else {
            return Ok(None);
        };
        self.active_id = None;
        if completion.id != expected {
            self.healthy = false;
            return Err(anyhow!(
                "persistence completion identity mismatch: expected {:?}, received {:?}",
                expected,
                completion.id
            ));
        }
        log::debug!(
            "Persistence completion {:?}: worker={:?}, queue_wait={:?}, execution={:?}, application_delay={:?}",
            completion.id,
            completion.worker_thread_id,
            completion.queue_wait,
            completion.execution_time,
            completion.finished_at.elapsed()
        );
        Ok(Some(completion))
    }

    pub(in crate::backend::wayland) fn shutdown(&mut self, target_epoch: u64) -> Result<()> {
        if self.active_id.is_some() && self.healthy {
            return Err(anyhow!(
                "cannot stop persistence worker while a request is active"
            ));
        }
        if self.worker.is_none() {
            return Ok(());
        }

        let mut shutdown_error = None;
        if self.healthy
            && self.request_tx.is_some()
            && let Err(err) = self.run(target_epoch, PersistenceOperation::Shutdown)
        {
            shutdown_error = Some(err);
        }
        self.request_tx.take();
        let Some(worker) = self.worker.take() else {
            return shutdown_error.map_or(Ok(()), Err);
        };
        let join = worker.join();
        self.active_id = None;
        if join.is_err() {
            self.healthy = false;
            return Err(anyhow!(
                "session persistence worker panicked during shutdown"
            ));
        }
        if let Some(err) = shutdown_error {
            return Err(err);
        }
        Ok(())
    }
}

impl Drop for PersistenceController {
    fn drop(&mut self) {
        if self.worker.is_none() {
            return;
        }
        let shutdown_id = (self.active_id.is_none() && self.healthy).then(|| self.next_id(0));
        if let (Some(id), Some(sender)) = (shutdown_id, self.request_tx.as_ref()) {
            let _ = sender.try_send(PersistenceRequest {
                id,
                queued_at: Instant::now(),
                operation: PersistenceOperation::Shutdown,
            });
        }
        self.request_tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod test_support;

fn assert_send_static<T: Send + 'static>() {}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;

    use super::*;
    use crate::session::SaveSnapshotOutcome;

    #[test]
    fn request_and_completion_payloads_are_send_and_static() {
        assert_send_static::<PersistenceOperation>();
        assert_send_static::<PersistenceOutcome>();
        assert_send_static::<PersistenceCompletion>();
    }

    fn test_options(base_dir: PathBuf) -> SessionOptions {
        let mut options = SessionOptions::new(base_dir, "worker-test");
        options.persist_transparent = true;
        options.persist_whiteboard = false;
        options.persist_blackboard = false;
        options.persist_history = false;
        options.restore_tool_state = false;
        options
    }

    fn controller_with_wake() -> (RuntimeWakeSource, PersistenceController) {
        let wake = RuntimeWakeSource::new().unwrap();
        let controller = PersistenceController::start(wake.handle()).unwrap();
        (wake, controller)
    }

    fn wait_for_runtime_wake(wake: &RuntimeWakeSource) {
        let mut pollfd = libc::pollfd {
            fd: wake.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        loop {
            // SAFETY: pollfd and the runtime-owned descriptor remain valid for
            // this bounded test wait.
            let ready = unsafe { libc::poll(&mut pollfd, 1, 1_000) };
            if ready > 0 {
                assert_ne!(pollfd.revents & libc::POLLIN, 0);
                return;
            }
            if ready == 0 {
                panic!("persistence worker did not wake runtime within one second");
            }
            let err = std::io::Error::last_os_error();
            if err.kind() != std::io::ErrorKind::Interrupted {
                panic!("runtime wake poll failed: {err}");
            }
        }
    }

    #[test]
    fn real_worker_publishes_completion_before_waking_runtime() {
        let temp = crate::test_temp::tempdir().unwrap();
        let options = test_options(temp.path().to_path_buf());
        let (wake, mut controller) = controller_with_wake();
        controller
            .try_submit(7, PersistenceOperation::HasArtifacts { options })
            .unwrap();

        wait_for_runtime_wake(&wake);
        wake.drain().unwrap();
        let completion = controller
            .try_receive()
            .unwrap()
            .expect("wake must follow completion publication");
        assert!(matches!(
            completion.result.unwrap(),
            PersistenceOutcome::HasArtifacts(false)
        ));
        controller.shutdown(7).unwrap();
    }

    #[test]
    fn production_error_completion_wakes_runtime_and_worker_remains_healthy() {
        let temp = crate::test_temp::tempdir().unwrap();
        let missing = temp.path().join("missing.wayscriber-session");
        let (wake, mut controller) = controller_with_wake();
        controller
            .try_submit(0, PersistenceOperation::ValidateNamedOpen { path: missing })
            .unwrap();

        wait_for_runtime_wake(&wake);
        wake.drain().unwrap();
        let completion = controller.try_receive().unwrap().unwrap();
        assert!(completion.result.is_err());
        assert!(controller.is_healthy());
        controller.shutdown(0).unwrap();
    }

    #[test]
    fn worker_panic_closes_completion_channel_before_waking_runtime() {
        let (wake, mut controller) = controller_with_wake();
        controller
            .try_submit(0, PersistenceOperation::PanicForTest)
            .unwrap();

        wait_for_runtime_wake(&wake);
        wake.drain().unwrap();
        let err = controller
            .try_receive()
            .expect_err("exit wake must expose the already-closed completion channel");
        assert!(err.to_string().contains("disconnected"));
        assert!(!controller.is_healthy());
        assert!(controller.shutdown(0).is_err());
        assert!(controller.is_stopped());
    }

    #[test]
    fn synchronous_barriers_and_shutdown_tolerate_redundant_unread_wake() {
        let temp = crate::test_temp::tempdir().unwrap();
        let options = test_options(temp.path().to_path_buf());
        let (wake, mut controller) = controller_with_wake();

        assert!(matches!(
            controller
                .run(0, PersistenceOperation::HasArtifacts { options })
                .unwrap(),
            PersistenceOutcome::HasArtifacts(false)
        ));
        controller.shutdown(0).unwrap();
        assert!(wake.drain().unwrap());
    }

    #[test]
    fn real_worker_executes_production_operation_off_caller_thread() {
        let temp = crate::test_temp::tempdir().unwrap();
        let options = test_options(temp.path().to_path_buf());
        let mut controller = PersistenceController::start_for_test().unwrap();
        controller
            .try_submit(7, PersistenceOperation::HasArtifacts { options })
            .unwrap();
        let completion = controller
            .wait_for_completion()
            .unwrap()
            .expect("worker completion");
        assert_ne!(completion.worker_thread_id, thread::current().id());
        assert!(matches!(
            completion.result.unwrap(),
            PersistenceOutcome::HasArtifacts(false)
        ));
        controller.shutdown(7).unwrap();
    }

    #[test]
    fn real_worker_uses_existing_save_pipeline_and_reports_committed_state() {
        let temp = crate::test_temp::tempdir().unwrap();
        let options = test_options(temp.path().to_path_buf());
        let snapshot = SessionSnapshot {
            active_board_id: "transparent".to_string(),
            boards: Vec::new(),
            tool_state: None,
        };
        let mut controller = PersistenceController::start_for_test().unwrap();
        let outcome = controller
            .run(
                0,
                PersistenceOperation::Save {
                    snapshot,
                    options: options.clone(),
                    strategy: SaveStrategy::Normal,
                    contentless_clear_boundary: true,
                },
            )
            .unwrap();
        let PersistenceOutcome::Save(save) = outcome else {
            panic!("unexpected worker outcome");
        };
        assert!(save.committed());
        assert!(!save.committed_board_data);
        assert!(matches!(
            save.report.unwrap().outcome,
            SaveSnapshotOutcome::ClearedEmpty
        ));
        assert!(matches!(
            controller
                .run(0, PersistenceOperation::LoadConfigured { options })
                .unwrap(),
            PersistenceOutcome::Load(LoadSnapshotOutcome::Empty)
        ));
        controller.shutdown(0).unwrap();
    }

    #[test]
    fn production_error_does_not_kill_worker() {
        let temp = crate::test_temp::tempdir().unwrap();
        let missing = temp.path().join("missing.wayscriber-session");
        let options = test_options(temp.path().to_path_buf());
        let mut controller = PersistenceController::start_for_test().unwrap();
        assert!(
            controller
                .run(0, PersistenceOperation::ValidateNamedOpen { path: missing })
                .is_err()
        );
        assert!(controller.is_healthy());
        assert!(matches!(
            controller
                .run(0, PersistenceOperation::HasArtifacts { options })
                .unwrap(),
            PersistenceOutcome::HasArtifacts(false)
        ));
        controller.shutdown(0).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn failed_autosave_can_be_followed_by_normal_save_on_same_worker() {
        use std::os::unix::fs::symlink;

        let temp = crate::test_temp::tempdir().unwrap();
        let target = temp.path().join("autosave-target.wayscriber-session");
        let alias = temp.path().join("autosave-alias.wayscriber-session");
        std::fs::write(&target, b"{}").unwrap();
        symlink(&target, &alias).unwrap();
        let mut invalid_options = test_options(temp.path().to_path_buf());
        invalid_options.set_named_file_target(alias);
        let valid_options = test_options(temp.path().join("valid"));
        let snapshot = SessionSnapshot {
            active_board_id: "transparent".to_string(),
            boards: Vec::new(),
            tool_state: None,
        };
        let mut controller = PersistenceController::start_for_test().unwrap();

        assert!(
            controller
                .run(
                    0,
                    PersistenceOperation::Save {
                        snapshot: snapshot.clone(),
                        options: invalid_options,
                        strategy: SaveStrategy::Autosave,
                        contentless_clear_boundary: true,
                    },
                )
                .is_err()
        );
        assert!(controller.is_healthy());
        let outcome = controller
            .run(
                0,
                PersistenceOperation::Save {
                    snapshot,
                    options: valid_options,
                    strategy: SaveStrategy::Normal,
                    contentless_clear_boundary: true,
                },
            )
            .unwrap();

        assert!(matches!(
            outcome,
            PersistenceOutcome::Save(ref save) if save.committed()
        ));
        controller.shutdown(0).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn save_as_preflight_rejects_same_target_symlink_before_identity_matching() {
        use std::os::unix::fs::symlink;

        let temp = crate::test_temp::tempdir().unwrap();
        let current_path = temp.path().join("current.wayscriber-session");
        let alias_path = temp.path().join("current-alias.wayscriber-session");
        std::fs::write(&current_path, b"{}").unwrap();
        symlink(&current_path, &alias_path).unwrap();

        let mut options = test_options(temp.path().to_path_buf());
        options.set_named_file_target(alias_path);
        let mut controller = PersistenceController::start_for_test().unwrap();
        let err = controller
            .run(
                0,
                PersistenceOperation::SaveAsOverwritePreflight {
                    current_path,
                    options,
                },
            )
            .expect_err("symlink alias must fail validation before matching the current target");

        assert!(format!("{err:#}").contains("symlink"));
        assert!(controller.is_healthy());
        controller.shutdown(0).unwrap();
    }

    #[test]
    fn save_as_preflight_reports_regular_current_target_after_validation() {
        let temp = crate::test_temp::tempdir().unwrap();
        let current_path = temp.path().join("current.wayscriber-session");
        std::fs::write(&current_path, b"{}").unwrap();
        let mut options = test_options(temp.path().to_path_buf());
        options.set_named_file_target(current_path.clone());
        let mut controller = PersistenceController::start_for_test().unwrap();

        let outcome = controller
            .run(
                0,
                PersistenceOperation::SaveAsOverwritePreflight {
                    current_path,
                    options,
                },
            )
            .unwrap();

        assert!(matches!(
            outcome,
            PersistenceOutcome::SaveAsPreflight {
                same_target: true,
                overwrite_required: false
            }
        ));
        controller.shutdown(0).unwrap();
    }

    #[test]
    fn same_target_preflight_skips_overwrite_discovery() {
        let current_path = Path::new("/tmp/current.wayscriber-session");
        let mut discovery_called = false;

        let result = save_as_preflight_after_validation(current_path, current_path, || {
            discovery_called = true;
            Ok(true)
        })
        .unwrap();

        assert_eq!(result, (true, false));
        assert!(!discovery_called);
    }

    #[test]
    fn different_target_preflight_runs_overwrite_discovery() {
        let current_path = Path::new("/tmp/current.wayscriber-session");
        let target_path = Path::new("/tmp/target.wayscriber-session");
        let mut discovery_called = false;

        let result = save_as_preflight_after_validation(current_path, target_path, || {
            discovery_called = true;
            Ok(true)
        })
        .unwrap();

        assert_eq!(result, (false, true));
        assert!(discovery_called);
    }

    #[test]
    fn full_try_submit_returns_owned_operation_without_installing_ticket() {
        let temp = crate::test_temp::tempdir().unwrap();
        let options = test_options(temp.path().to_path_buf());
        let (request_tx, request_rx) = mpsc::sync_channel(1);
        request_tx
            .send(PersistenceRequest {
                id: RequestId {
                    target_epoch: 0,
                    sequence: 99,
                },
                queued_at: Instant::now(),
                operation: PersistenceOperation::Shutdown,
            })
            .unwrap();
        let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
        let mut controller = PersistenceController {
            request_tx: Some(request_tx),
            completion_rx,
            worker: None,
            active_id: None,
            next_sequence: 0,
            healthy: true,
        };

        let result = controller.try_submit(
            0,
            PersistenceOperation::HasArtifacts {
                options: options.clone(),
            },
        );
        let failure = result.expect_err("full request channel must reject submission");
        assert!(matches!(failure.error, SubmitError::Full));
        assert!(matches!(
            *failure.operation,
            PersistenceOperation::HasArtifacts { .. }
        ));
        assert!(controller.active_id.is_none());
        assert!(!controller.is_healthy());
        drop(request_rx);
    }

    #[test]
    fn disconnected_try_submit_returns_owned_operation_without_installing_ticket() {
        let temp = crate::test_temp::tempdir().unwrap();
        let options = test_options(temp.path().to_path_buf());
        let (request_tx, request_rx) = mpsc::sync_channel(1);
        drop(request_rx);
        let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
        let mut controller = PersistenceController {
            request_tx: Some(request_tx),
            completion_rx,
            worker: None,
            active_id: None,
            next_sequence: 0,
            healthy: true,
        };

        let result = controller.try_submit(0, PersistenceOperation::HasArtifacts { options });
        let failure = result.expect_err("disconnected channel must reject submission");
        assert!(matches!(failure.error, SubmitError::Disconnected));
        assert!(matches!(
            *failure.operation,
            PersistenceOperation::HasArtifacts { .. }
        ));
        assert!(controller.active_id.is_none());
        assert!(!controller.is_healthy());
    }
}
