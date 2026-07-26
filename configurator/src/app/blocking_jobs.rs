use std::collections::VecDeque;
use std::fmt;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use iced::Task;
use wayscriber::config::{Config, ConfigDocument};

use crate::models::{
    DaemonAction, DaemonActionResult, DaemonRuntimeStatus, SessionCatalogActionResult,
    SessionCatalogItem,
};

use super::daemon_setup::{load_daemon_runtime_status, perform_daemon_action};
use super::io::{load_config_from_disk, save_config_to_disk};
use super::session_catalog::{
    clear_session_catalog_entry, clear_session_catalog_tool_state_entry,
    duplicate_session_catalog_entry, forget_session_catalog_entry, load_session_catalog,
    move_session_catalog_entry, rename_session_catalog_entry, reveal_session_catalog_entry,
};

const PRODUCTION_BLOCKING_JOB_LIMIT: NonZeroUsize = match NonZeroUsize::new(2) {
    Some(limit) => limit,
    None => NonZeroUsize::MIN,
};
const SLOW_JOB_THRESHOLD: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct BlockingJobId(Vec<u8>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockingJobPurpose {
    ConfigLoad,
    ConfigSave,
    DaemonStatus { preserve_feedback: bool },
    DaemonAction,
    SessionCatalogLoad,
    SessionCatalogMutation,
}

impl fmt::Display for BlockingJobPurpose {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::ConfigLoad => "config load",
            Self::ConfigSave => "config save",
            Self::DaemonStatus { .. } => "daemon status",
            Self::DaemonAction => "daemon action",
            Self::SessionCatalogLoad => "session catalog load",
            Self::SessionCatalogMutation => "session catalog mutation",
        };
        formatter.write_str(label)
    }
}

#[derive(Debug)]
pub(super) enum BlockingJobRequest {
    #[cfg(test)]
    FixtureOutput(u8),
    ConfigLoad,
    ConfigSave {
        document: Box<ConfigDocument>,
        config: Box<Config>,
    },
    DaemonStatus {
        preserve_feedback: bool,
    },
    DaemonAction {
        action: DaemonAction,
        shortcut_input: String,
    },
    SessionCatalogLoad,
    SessionCatalogMutation(SessionCatalogMutation),
}

impl BlockingJobRequest {
    pub(super) fn purpose(&self) -> BlockingJobPurpose {
        match self {
            #[cfg(test)]
            Self::FixtureOutput(_) => BlockingJobPurpose::ConfigLoad,
            Self::ConfigLoad => BlockingJobPurpose::ConfigLoad,
            Self::ConfigSave { .. } => BlockingJobPurpose::ConfigSave,
            Self::DaemonStatus { preserve_feedback } => BlockingJobPurpose::DaemonStatus {
                preserve_feedback: *preserve_feedback,
            },
            Self::DaemonAction { .. } => BlockingJobPurpose::DaemonAction,
            Self::SessionCatalogLoad => BlockingJobPurpose::SessionCatalogLoad,
            Self::SessionCatalogMutation(_) => BlockingJobPurpose::SessionCatalogMutation,
        }
    }

    fn newest_family(&self) -> Option<NewestJobFamily> {
        match self {
            #[cfg(test)]
            Self::FixtureOutput(_) => None,
            Self::ConfigLoad => Some(NewestJobFamily::ConfigLoad),
            Self::DaemonStatus { .. } => Some(NewestJobFamily::DaemonStatus),
            Self::SessionCatalogLoad => Some(NewestJobFamily::SessionCatalogLoad),
            Self::ConfigSave { .. }
            | Self::DaemonAction { .. }
            | Self::SessionCatalogMutation(_) => None,
        }
    }

    fn execute(self, path_resolver: &wayscriber::paths::PathResolver) -> BlockingJobOutput {
        match self {
            #[cfg(test)]
            Self::FixtureOutput(value) => BlockingJobOutput::Fixture(value),
            Self::ConfigLoad => BlockingJobOutput::ConfigLoaded(
                wayscriber::config::ConfigStore::from_resolver(path_resolver)
                    .map_err(|error| error.to_string())
                    .and_then(|store| load_config_from_disk(&store)),
            ),
            Self::ConfigSave { document, config } => {
                match save_config_to_disk(&document, *config) {
                    Ok((backup_path, saved_document)) => BlockingJobOutput::ConfigSaved {
                        document: Box::new(saved_document),
                        outcome: Ok(backup_path),
                    },
                    Err(error) => BlockingJobOutput::ConfigSaved {
                        document,
                        outcome: Err(error),
                    },
                }
            }
            Self::DaemonStatus { preserve_feedback } => BlockingJobOutput::DaemonStatusLoaded {
                preserve_feedback,
                result: load_daemon_runtime_status(path_resolver),
            },
            Self::DaemonAction {
                action,
                shortcut_input,
            } => BlockingJobOutput::DaemonActionCompleted(perform_daemon_action(
                action,
                shortcut_input,
                path_resolver,
            )),
            Self::SessionCatalogLoad => {
                BlockingJobOutput::SessionCatalogLoaded(load_session_catalog(path_resolver))
            }
            Self::SessionCatalogMutation(mutation) => {
                BlockingJobOutput::SessionCatalogActionCompleted(mutation.execute(path_resolver))
            }
        }
    }
}

#[derive(Debug)]
pub(super) enum SessionCatalogMutation {
    Forget { id: String },
    Rename { id: String, display_name: String },
    Duplicate { id: String, target: PathBuf },
    Move { id: String, target: PathBuf },
    Reveal { id: String },
    ClearToolState { id: String },
    Clear { id: String },
}

impl SessionCatalogMutation {
    fn execute(
        self,
        path_resolver: &wayscriber::paths::PathResolver,
    ) -> Result<SessionCatalogActionResult, String> {
        match self {
            Self::Forget { id } => forget_session_catalog_entry(id, path_resolver),
            Self::Rename { id, display_name } => {
                rename_session_catalog_entry(id, display_name, path_resolver)
            }
            Self::Duplicate { id, target } => {
                duplicate_session_catalog_entry(id, target, path_resolver)
            }
            Self::Move { id, target } => move_session_catalog_entry(id, target, path_resolver),
            Self::Reveal { id } => reveal_session_catalog_entry(id, path_resolver),
            Self::ClearToolState { id } => {
                clear_session_catalog_tool_state_entry(id, path_resolver)
            }
            Self::Clear { id } => clear_session_catalog_entry(id, path_resolver),
        }
    }
}

#[derive(Debug)]
pub(super) enum BlockingJobOutput {
    #[cfg(test)]
    Fixture(u8),
    ConfigLoaded(Result<(Box<ConfigDocument>, Option<String>), String>),
    ConfigSaved {
        document: Box<ConfigDocument>,
        outcome: Result<Option<PathBuf>, String>,
    },
    DaemonStatusLoaded {
        preserve_feedback: bool,
        result: Result<DaemonRuntimeStatus, String>,
    },
    DaemonActionCompleted(Result<DaemonActionResult, String>),
    SessionCatalogLoaded(Result<Vec<SessionCatalogItem>, String>),
    SessionCatalogActionCompleted(Result<SessionCatalogActionResult, String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BlockingJobTaskFailure {
    WorkerCancelled,
    WorkerPanicked,
    WorkerJoin(String),
    ResultMissing,
    ResultTransportClosed,
}

impl BlockingJobTaskFailure {
    pub(super) fn describe(&self, purpose: BlockingJobPurpose) -> String {
        match self {
            Self::WorkerCancelled => format!("{purpose} blocking job was cancelled"),
            Self::WorkerPanicked => format!("{purpose} blocking job panicked"),
            Self::WorkerJoin(error) => {
                format!("{purpose} blocking job did not complete: {error}")
            }
            Self::ResultMissing => {
                format!("{purpose} blocking job completed without reporting its result")
            }
            Self::ResultTransportClosed => {
                format!("{purpose} blocking job result transport closed unexpectedly")
            }
        }
    }
}

#[derive(Debug)]
enum BlockingJobCompletion {
    Completed(BlockingJobOutput),
    Failed(BlockingJobTaskFailure),
}

#[derive(Debug)]
pub(super) enum BlockingJobTransition {
    Completed(BlockingJobOutput),
    Failed {
        purpose: BlockingJobPurpose,
        failure: BlockingJobTaskFailure,
    },
    Canceled {
        purpose: BlockingJobPurpose,
    },
    Stale {
        id: BlockingJobId,
    },
}

#[derive(Debug)]
pub(super) struct BlockingJobUpdate {
    pub(super) transition: BlockingJobTransition,
    pub(super) started: Task<BlockingJobId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockingJobCancellation {
    None,
    Superseded { running: usize, queued: usize },
}

#[derive(Debug)]
pub(super) struct BlockingJobSubmission {
    pub(super) id: BlockingJobId,
    pub(super) started: Task<BlockingJobId>,
    pub(super) cancellation: BlockingJobCancellation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NewestJobFamily {
    ConfigLoad,
    DaemonStatus,
    SessionCatalogLoad,
}

impl BlockingJobPurpose {
    fn newest_family(self) -> Option<NewestJobFamily> {
        match self {
            Self::ConfigLoad => Some(NewestJobFamily::ConfigLoad),
            Self::DaemonStatus { .. } => Some(NewestJobFamily::DaemonStatus),
            Self::SessionCatalogLoad => Some(NewestJobFamily::SessionCatalogLoad),
            Self::ConfigSave | Self::DaemonAction | Self::SessionCatalogMutation => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveJobStatus {
    Current,
    Canceled,
}

#[derive(Debug)]
struct ActiveJob {
    id: BlockingJobId,
    purpose: BlockingJobPurpose,
    status: ActiveJobStatus,
    result: Receiver<BlockingJobCompletion>,
}

#[derive(Debug)]
struct QueuedJob {
    id: BlockingJobId,
    request: BlockingJobRequest,
    queued_at: Instant,
}

#[derive(Debug)]
pub(super) struct BlockingJobs {
    path_resolver: wayscriber::paths::PathResolver,
    next_job_id: Vec<u8>,
    max_concurrent_jobs: NonZeroUsize,
    active: Vec<ActiveJob>,
    pending: VecDeque<QueuedJob>,
}

impl BlockingJobs {
    pub(super) fn new(path_resolver: wayscriber::paths::PathResolver) -> Self {
        Self::with_limit(PRODUCTION_BLOCKING_JOB_LIMIT, path_resolver)
    }

    fn with_limit(
        max_concurrent_jobs: NonZeroUsize,
        path_resolver: wayscriber::paths::PathResolver,
    ) -> Self {
        Self {
            path_resolver,
            next_job_id: vec![1],
            max_concurrent_jobs,
            active: Vec::new(),
            pending: VecDeque::new(),
        }
    }

    pub(super) fn submit(&mut self, request: BlockingJobRequest) -> BlockingJobSubmission {
        let cancellation = request
            .newest_family()
            .map_or(BlockingJobCancellation::None, |family| {
                self.cancel_family(family)
            });
        let id = self.mint_job_id();
        self.pending.push_back(QueuedJob {
            id: id.clone(),
            request,
            queued_at: Instant::now(),
        });
        let started = self.start_ready_jobs();
        BlockingJobSubmission {
            id,
            started,
            cancellation,
        }
    }

    pub(super) fn cancel_daemon_status(&mut self) -> BlockingJobCancellation {
        self.cancel_family(NewestJobFamily::DaemonStatus)
    }

    pub(super) fn handle_ready(&mut self, id: BlockingJobId) -> BlockingJobUpdate {
        let Some(index) = self.active.iter().position(|job| job.id == id) else {
            return BlockingJobUpdate {
                transition: BlockingJobTransition::Stale { id },
                started: Task::none(),
            };
        };

        let job = self.active.remove(index);
        let completion = job.result.try_recv();
        let transition = if job.status == ActiveJobStatus::Canceled {
            BlockingJobTransition::Canceled {
                purpose: job.purpose,
            }
        } else {
            match completion {
                Ok(BlockingJobCompletion::Completed(output)) => {
                    BlockingJobTransition::Completed(output)
                }
                Ok(BlockingJobCompletion::Failed(failure)) => BlockingJobTransition::Failed {
                    purpose: job.purpose,
                    failure,
                },
                Err(TryRecvError::Empty) => BlockingJobTransition::Failed {
                    purpose: job.purpose,
                    failure: BlockingJobTaskFailure::ResultMissing,
                },
                Err(TryRecvError::Disconnected) => BlockingJobTransition::Failed {
                    purpose: job.purpose,
                    failure: BlockingJobTaskFailure::ResultTransportClosed,
                },
            }
        };

        BlockingJobUpdate {
            transition,
            started: self.start_ready_jobs(),
        }
    }

    fn mint_job_id(&mut self) -> BlockingJobId {
        let id = BlockingJobId(self.next_job_id.clone());
        let mut carry = true;
        for digit in self.next_job_id.iter_mut().rev() {
            if !carry {
                break;
            }
            let (next, overflowed) = digit.overflowing_add(1);
            *digit = next;
            carry = overflowed;
        }
        if carry {
            self.next_job_id.insert(0, 1);
        }
        id
    }

    fn cancel_family(&mut self, family: NewestJobFamily) -> BlockingJobCancellation {
        let mut running = 0;
        for job in &mut self.active {
            if job.purpose.newest_family() == Some(family) && job.status == ActiveJobStatus::Current
            {
                job.status = ActiveJobStatus::Canceled;
                running += 1;
            }
        }

        let before = self.pending.len();
        self.pending
            .retain(|job| job.request.newest_family() != Some(family));
        let queued = before.saturating_sub(self.pending.len());
        if running == 0 && queued == 0 {
            BlockingJobCancellation::None
        } else {
            BlockingJobCancellation::Superseded { running, queued }
        }
    }

    fn start_ready_jobs(&mut self) -> Task<BlockingJobId> {
        let mut tasks = Vec::new();
        while self.active.len() < self.max_concurrent_jobs.get() {
            let Some(queued) = self.pending.pop_front() else {
                break;
            };
            let purpose = queued.request.purpose();
            report_slow_phase(purpose, "queue wait", queued.queued_at.elapsed());
            let id = queued.id;
            let task_id = id.clone();
            let path_resolver = self.path_resolver.clone();
            let (result_sender, result) = mpsc::channel();
            let task = Task::perform(
                run_blocking_job(
                    task_id,
                    purpose,
                    queued.request,
                    path_resolver,
                    result_sender,
                ),
                |completed_id| completed_id,
            );
            self.active.push(ActiveJob {
                id,
                purpose,
                status: ActiveJobStatus::Current,
                result,
            });
            tasks.push(task);
        }
        Task::batch(tasks)
    }
}

async fn run_blocking_job(
    id: BlockingJobId,
    purpose: BlockingJobPurpose,
    request: BlockingJobRequest,
    path_resolver: wayscriber::paths::PathResolver,
    result_sender: mpsc::Sender<BlockingJobCompletion>,
) -> BlockingJobId {
    let joined = tokio::task::spawn_blocking(move || {
        let started_at = Instant::now();
        let output = request.execute(&path_resolver);
        report_slow_phase(purpose, "execution", started_at.elapsed());
        output
    })
    .await;
    let completion = match joined {
        Ok(output) => BlockingJobCompletion::Completed(output),
        Err(error) if error.is_cancelled() => {
            BlockingJobCompletion::Failed(BlockingJobTaskFailure::WorkerCancelled)
        }
        Err(error) if error.is_panic() => {
            BlockingJobCompletion::Failed(BlockingJobTaskFailure::WorkerPanicked)
        }
        Err(error) => {
            BlockingJobCompletion::Failed(BlockingJobTaskFailure::WorkerJoin(error.to_string()))
        }
    };
    let _send_result = result_sender.send(completion);
    id
}

fn report_slow_phase(purpose: BlockingJobPurpose, phase: &str, elapsed: Duration) {
    if elapsed >= SLOW_JOB_THRESHOLD {
        eprintln!(
            "wayscriber configurator: slow {purpose} blocking job {phase}: {:.0} ms",
            elapsed.as_secs_f64() * 1_000.0
        );
    }
}

#[cfg(test)]
#[path = "blocking_jobs/tests.rs"]
mod tests;
