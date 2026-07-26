use anyhow::{Context, Result, anyhow};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const SESSION_FILE_EXTENSION: &str = "wayscriber-session";
const SESSION_FILE_DIALOG_EXECUTION_BUDGET: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland::state) enum SessionFileDialogMode {
    Open,
    SaveAs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland::state::toolbar::events) enum SessionFileDialogResult {
    Selected(PathBuf),
    Cancelled,
}

#[derive(Debug)]
pub(in crate::backend::wayland::state) struct SessionFileDialogCompletion {
    pub(in crate::backend::wayland::state) mode: SessionFileDialogMode,
    pub(in crate::backend::wayland::state) result: Result<Option<PathBuf>, String>,
}

type SessionFileDialogMessage = (u64, SessionFileDialogMode, Result<Option<PathBuf>, String>);

#[derive(Debug)]
pub(in crate::backend::wayland::state) struct SessionFileDialogController {
    next_id: u64,
    active: Option<(u64, SessionFileDialogMode)>,
    receiver: Option<mpsc::Receiver<SessionFileDialogMessage>>,
    worker: Option<JoinHandle<()>>,
    runtime_wake: crate::backend::wayland::RuntimeWakeSender,
    process_broker: crate::process_broker::ProcessBrokerHandle,
    default_dir: Result<PathBuf, String>,
}

impl SessionFileDialogController {
    pub(in crate::backend::wayland::state) fn new(
        runtime_wake: crate::backend::wayland::RuntimeWakeSender,
        process_broker: crate::process_broker::ProcessBrokerHandle,
        paths: &crate::paths::PathResolver,
    ) -> Self {
        Self {
            next_id: 1,
            active: None,
            receiver: None,
            worker: None,
            runtime_wake,
            process_broker,
            default_dir: paths.home_dir().map_err(|error| error.to_string()),
        }
    }

    pub(in crate::backend::wayland::state) fn start(
        &mut self,
        mode: SessionFileDialogMode,
        current_path: Option<PathBuf>,
    ) -> Result<()> {
        if self.active.is_some() {
            return Err(anyhow!("a session file dialog is already active"));
        }
        let default_dir = self
            .default_dir
            .clone()
            .map_err(|error| anyhow!("session chooser path is unavailable: {error}"))?;
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| anyhow!("session dialog identity exhausted"))?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let wake = self
            .runtime_wake
            .try_duplicate()
            .context("failed to duplicate session dialog runtime wake")?;
        let process_broker = self.process_broker.clone();
        let worker = std::thread::Builder::new()
            .name(format!("wayscriber-session-dialog-{id}"))
            .spawn(move || {
                let result = choose_session_file(
                    &process_broker,
                    mode,
                    current_path.as_deref(),
                    &default_dir,
                )
                .map_err(|error| format!("{error:#}"));
                let _ = sender.send((id, mode, result));
                if let Err(error) = wake.wake() {
                    log::error!("Failed to wake runtime for session dialog completion: {error}");
                }
            })
            .context("failed to start session dialog worker")?;
        self.active = Some((id, mode));
        self.receiver = Some(receiver);
        self.worker = Some(worker);
        Ok(())
    }

    pub(in crate::backend::wayland::state) fn try_receive(
        &mut self,
    ) -> Result<Option<SessionFileDialogCompletion>> {
        let Some((expected_id, expected_mode)) = self.active else {
            return Ok(None);
        };
        let receiver = self
            .receiver
            .as_ref()
            .ok_or_else(|| anyhow!("active session dialog has no completion receiver"))?;
        let received = match receiver.try_recv() {
            Ok(received) => received,
            Err(mpsc::TryRecvError::Empty) => return Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => (
                expected_id,
                expected_mode,
                Err("session dialog worker exited without a completion".into()),
            ),
        };
        self.active = None;
        self.receiver = None;
        self.join_finished_worker()?;
        let (id, mode, result) = received;
        if id != expected_id || mode != expected_mode {
            return Err(anyhow!("session dialog completion identity mismatch"));
        }
        Ok(Some(SessionFileDialogCompletion { mode, result }))
    }

    fn join_finished_worker(&mut self) -> Result<()> {
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        worker
            .join()
            .map_err(|_| anyhow!("session dialog worker panicked during teardown"))
    }
}

impl Drop for SessionFileDialogController {
    fn drop(&mut self) {
        // Both chooser attempts share one execution deadline. At most one
        // broker response-grace interval can extend past that deadline because
        // an expired budget prevents the fallback from starting. Wayland state
        // drops this owner while the broker actor is still live, so the worker
        // can finish and be joined without reversing broker teardown order.
        if let Err(error) = self.join_finished_worker() {
            log::error!("Failed to join session dialog worker: {error:#}");
        }
    }
}

#[cfg(test)]
pub(in crate::backend::wayland::state::toolbar::events) type SessionFileChooser =
    fn(SessionFileDialogMode, Option<&Path>) -> Result<Option<SessionFileDialogResult>>;

pub(in crate::backend::wayland::state::toolbar::events) fn choose_session_file(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    mode: SessionFileDialogMode,
    current_path: Option<&Path>,
    default_dir: &Path,
) -> Result<Option<PathBuf>> {
    let now = Instant::now();
    let deadline = now
        .checked_add(SESSION_FILE_DIALOG_EXECUTION_BUDGET)
        .ok_or_else(|| anyhow!("session file chooser deadline overflow"))?;
    let mut errors = Vec::new();
    for chooser in [
        run_zenity_session_file_dialog as ProcessBrokerSessionFileChooser,
        run_kdialog_session_file_dialog,
    ] {
        let Some(timeout) = remaining_chooser_budget(Instant::now(), deadline) else {
            errors.push("session file chooser execution budget expired".into());
            break;
        };
        match chooser(process_broker, mode, current_path, default_dir, timeout) {
            Ok(Some(SessionFileDialogResult::Selected(path))) => return Ok(Some(path)),
            Ok(Some(SessionFileDialogResult::Cancelled)) => return Ok(None),
            Ok(None) => {}
            Err(err) => {
                let message = format!("{err:#}");
                log::warn!("Session file chooser failed; trying fallback if available: {message}");
                errors.push(message);
            }
        }
    }
    no_session_file_chooser_error(errors)
}

type ProcessBrokerSessionFileChooser = fn(
    &crate::process_broker::ProcessBrokerHandle,
    SessionFileDialogMode,
    Option<&Path>,
    &Path,
    Duration,
) -> Result<Option<SessionFileDialogResult>>;

fn remaining_chooser_budget(now: Instant, deadline: Instant) -> Option<Duration> {
    let remaining = deadline.saturating_duration_since(now);
    if remaining.is_zero() {
        None
    } else {
        Some(remaining)
    }
}

#[cfg(test)]
pub(in crate::backend::wayland::state::toolbar::events) fn choose_session_file_from(
    mode: SessionFileDialogMode,
    current_path: Option<&Path>,
    choosers: &[SessionFileChooser],
) -> Result<Option<PathBuf>> {
    let mut errors = Vec::new();
    for chooser in choosers {
        match chooser(mode, current_path) {
            Ok(Some(SessionFileDialogResult::Selected(path))) => return Ok(Some(path)),
            Ok(Some(SessionFileDialogResult::Cancelled)) => return Ok(None),
            Ok(None) => {}
            Err(err) => {
                let message = format!("{err:#}");
                log::warn!("Session file chooser failed; trying fallback if available: {message}");
                errors.push(message);
            }
        }
    }

    no_session_file_chooser_error(errors)
}

fn no_session_file_chooser_error(errors: Vec<String>) -> Result<Option<PathBuf>> {
    if errors.is_empty() {
        Err(anyhow!(
            "No supported session file chooser found; tried zenity and kdialog"
        ))
    } else {
        Err(anyhow!(
            "No usable session file chooser found; tried zenity and kdialog: {}",
            errors.join("; ")
        ))
    }
}

fn run_zenity_session_file_dialog(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    mode: SessionFileDialogMode,
    current_path: Option<&Path>,
    default_dir: &Path,
    timeout: Duration,
) -> Result<Option<SessionFileDialogResult>> {
    let mut arguments = vec![
        OsString::from("--file-selection"),
        OsString::from("--title"),
        OsString::from(match mode {
            SessionFileDialogMode::Open => "Open Wayscriber Session",
            SessionFileDialogMode::SaveAs => "Save Wayscriber Session As",
        }),
    ];
    match mode {
        SessionFileDialogMode::Open => {
            let path = current_path.and_then(Path::parent).unwrap_or(default_dir);
            arguments.push("--filename".into());
            arguments.push(path.as_os_str().into());
        }
        SessionFileDialogMode::SaveAs => {
            arguments.push("--save".into());
            arguments.push("--filename".into());
            arguments.push(default_save_as_path(current_path, default_dir).into_os_string());
        }
    }
    arguments.extend([
        "--file-filter".into(),
        "Wayscriber sessions | *.wayscriber-session *.session".into(),
        "--file-filter".into(),
        "All files | *".into(),
    ]);
    run_session_file_dialog_command(
        process_broker,
        crate::process_broker::HelperKind::SessionZenity,
        "zenity",
        arguments,
        timeout,
    )
}

fn run_kdialog_session_file_dialog(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    mode: SessionFileDialogMode,
    current_path: Option<&Path>,
    default_dir: &Path,
    timeout: Duration,
) -> Result<Option<SessionFileDialogResult>> {
    let mut arguments = Vec::new();
    match mode {
        SessionFileDialogMode::Open => {
            arguments.push("--getopenfilename".into());
            arguments.push(
                current_path
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| default_dir.to_path_buf())
                    .into_os_string(),
            );
        }
        SessionFileDialogMode::SaveAs => {
            arguments.push("--getsavefilename".into());
            arguments.push(default_save_as_path(current_path, default_dir).into_os_string());
        }
    }
    arguments.push("Wayscriber sessions (*.wayscriber-session *.session);;All files (*)".into());
    run_session_file_dialog_command(
        process_broker,
        crate::process_broker::HelperKind::SessionKdialog,
        "kdialog",
        arguments,
        timeout,
    )
}

fn run_session_file_dialog_command(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    kind: crate::process_broker::HelperKind,
    program: &'static str,
    arguments: Vec<OsString>,
    timeout: Duration,
) -> Result<Option<SessionFileDialogResult>> {
    let output = match process_broker.run(
        kind,
        OsStr::new(program),
        &arguments,
        Vec::new(),
        timeout,
        64 * 1024,
    ) {
        Ok(output) => output,
        Err(err) if err.to_string().contains("No such file") => return Ok(None),
        Err(err) => return Err(anyhow!("failed to launch {program}: {err:#}")),
    };

    let selected = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from);
    if !output.timed_out && output.status == 0 {
        return Ok(Some(match selected {
            Some(path) => SessionFileDialogResult::Selected(path),
            None => SessionFileDialogResult::Cancelled,
        }));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.timed_out && stderr.trim().is_empty() {
        return Ok(Some(SessionFileDialogResult::Cancelled));
    }

    Err(anyhow!(
        "{program} session file chooser failed: {}",
        stderr.trim()
    ))
}

pub(in crate::backend::wayland::state::toolbar::events) fn default_save_as_path(
    current_path: Option<&Path>,
    default_dir: &Path,
) -> PathBuf {
    default_save_as_dir(default_dir).join(save_as_file_name(current_path))
}

fn default_save_as_dir(home: &Path) -> PathBuf {
    let documents = home.join("Documents");
    if documents.is_dir() {
        documents
    } else {
        home.to_path_buf()
    }
}

pub(in crate::backend::wayland::state::toolbar::events) fn save_as_file_name(
    current_path: Option<&Path>,
) -> String {
    let Some(current) = current_path
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
    else {
        return format!("session-copy.{SESSION_FILE_EXTENSION}");
    };
    let path = Path::new(current);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("session");
    format!("{stem}-copy.{SESSION_FILE_EXTENSION}")
}

pub(in crate::backend::wayland::state::toolbar::events) fn ensure_save_as_extension(
    path: PathBuf,
) -> PathBuf {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .is_some()
    {
        return path;
    }

    path.with_extension(SESSION_FILE_EXTENSION)
}

#[cfg(test)]
mod controller_tests {
    use super::*;

    fn controller() -> (
        crate::process_broker::ProcessBrokerOwner,
        SessionFileDialogController,
    ) {
        let wake = crate::backend::wayland::RuntimeWakeSource::new()
            .expect("session-dialog fixture creates its runtime eventfd");
        let process_broker_owner = crate::process_broker::start_for_runtime()
            .expect("session-dialog fixture starts an isolated process broker");
        let paths =
            crate::paths::PathResolver::from_environment(crate::paths::PathEnvironment::for_test(
                &[(crate::env_vars::HOME_ENV, std::ffi::OsStr::new("/tmp"))],
            ));
        let controller = SessionFileDialogController::new(
            wake.try_sender()
                .expect("test duplicates its session-dialog runtime eventfd"),
            process_broker_owner.handle(),
            &paths,
        );
        (process_broker_owner, controller)
    }

    #[test]
    fn completion_identity_mismatch_is_terminal_and_consumed_once() {
        let (_process_broker_owner, mut controller) = controller();
        let (sender, receiver) = mpsc::sync_channel(1);
        controller.active = Some((7, SessionFileDialogMode::Open));
        controller.receiver = Some(receiver);
        sender
            .send((
                8,
                SessionFileDialogMode::Open,
                Ok(Some(PathBuf::from("/tmp/session"))),
            ))
            .expect("identity-mismatch fixture retains its completion receiver");

        assert!(controller.try_receive().is_err());
        assert!(
            controller
                .try_receive()
                .expect("terminal mismatch fixture permits a second poll")
                .is_none()
        );
    }

    #[test]
    fn worker_disconnect_produces_one_identified_failure() {
        let (_process_broker_owner, mut controller) = controller();
        let (sender, receiver) = mpsc::sync_channel(1);
        controller.active = Some((9, SessionFileDialogMode::SaveAs));
        controller.receiver = Some(receiver);
        drop(sender);

        let completion = controller
            .try_receive()
            .expect("disconnected-worker fixture returns a typed completion")
            .expect("disconnected-worker fixture has one terminal completion");
        assert_eq!(completion.mode, SessionFileDialogMode::SaveAs);
        assert!(
            completion
                .result
                .expect_err("disconnected-worker fixture returns its typed failure reason")
                .contains("without a completion")
        );
        assert!(
            controller
                .try_receive()
                .expect("disconnected-worker fixture permits a second poll")
                .is_none()
        );
    }

    #[test]
    fn active_dialog_rejects_overlap_before_spawning_worker() {
        let (_process_broker_owner, mut controller) = controller();
        controller.active = Some((1, SessionFileDialogMode::Open));
        assert!(
            controller
                .start(SessionFileDialogMode::SaveAs, None)
                .expect_err("active-dialog fixture rejects an overlapping request")
                .to_string()
                .contains("already active")
        );
    }

    #[test]
    fn chooser_attempts_share_one_execution_budget() {
        let now = Instant::now();
        let deadline = now
            .checked_add(Duration::from_secs(7))
            .expect("budget fixture adds a small duration to a live instant");

        assert_eq!(
            remaining_chooser_budget(now, deadline),
            Some(Duration::from_secs(7))
        );
        assert_eq!(remaining_chooser_budget(deadline, deadline), None);
    }
}
