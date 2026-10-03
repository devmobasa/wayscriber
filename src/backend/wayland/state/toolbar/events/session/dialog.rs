use crate::backend::wayland::runtime_operation::{
    RuntimeOperationController, RuntimeOperationIdSource, RuntimeOperationPoll,
};

use anyhow::{Result, anyhow};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

const SESSION_FILE_EXTENSION: &str = "wayscriber-session";

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

pub(in crate::backend::wayland::state) struct SessionFileDialogController {
    operation: RuntimeOperationController<SessionFileDialogMode, Result<Option<PathBuf>, String>>,
}

impl std::fmt::Debug for SessionFileDialogController {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SessionFileDialogController")
            .field("active", &self.operation.is_active())
            .finish()
    }
}

impl SessionFileDialogController {
    pub(in crate::backend::wayland::state) fn new(
        runtime_wake: crate::backend::wayland::RuntimeWakeHandle,
    ) -> Self {
        Self {
            operation: RuntimeOperationController::new(
                RuntimeOperationIdSource::new(),
                runtime_wake,
            ),
        }
    }

    pub(in crate::backend::wayland::state) fn start(
        &mut self,
        mode: SessionFileDialogMode,
        current_path: Option<PathBuf>,
    ) -> Result<()> {
        self.submit(mode, move || {
            choose_session_file(mode, current_path.as_deref()).map_err(|error| format!("{error:#}"))
        })
    }

    fn submit(
        &mut self,
        mode: SessionFileDialogMode,
        chooser: impl FnOnce() -> Result<Option<PathBuf>, String> + Send + 'static,
    ) -> Result<()> {
        self.operation
            .try_submit(mode, "wayscriber-session-dialog", chooser)
            .map(|_| ())
            .map_err(|failure| anyhow!(failure.into_parts().0))
    }

    pub(in crate::backend::wayland::state) fn try_receive(
        &mut self,
    ) -> Result<Option<SessionFileDialogCompletion>> {
        let completion = match self.operation.poll() {
            RuntimeOperationPoll::Idle | RuntimeOperationPoll::Pending { .. } => return Ok(None),
            RuntimeOperationPoll::Ready {
                context: mode,
                outcome: result,
                ..
            } => SessionFileDialogCompletion { mode, result },
            RuntimeOperationPoll::ProducerFailed {
                context: mode,
                reason,
                ..
            } => SessionFileDialogCompletion {
                mode,
                result: Err(reason),
            },
            RuntimeOperationPoll::Disconnected { context: mode, .. } => {
                SessionFileDialogCompletion {
                    mode,
                    result: Err("session dialog worker exited without a completion".into()),
                }
            }
        };

        Ok(Some(completion))
    }
}

pub(in crate::backend::wayland::state::toolbar::events) type SessionFileChooser =
    fn(SessionFileDialogMode, Option<&Path>) -> Result<Option<SessionFileDialogResult>>;

pub(in crate::backend::wayland::state::toolbar::events) fn choose_session_file(
    mode: SessionFileDialogMode,
    current_path: Option<&Path>,
) -> Result<Option<PathBuf>> {
    choose_session_file_from(
        mode,
        current_path,
        &[
            run_zenity_session_file_dialog,
            run_kdialog_session_file_dialog,
        ],
    )
}

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

    if errors.is_empty() {
        return Err(anyhow!(
            "No supported session file chooser found; tried zenity and kdialog"
        ));
    }

    Err(anyhow!(
        "No usable session file chooser found; tried zenity and kdialog: {}",
        errors.join("; ")
    ))
}

fn run_zenity_session_file_dialog(
    mode: SessionFileDialogMode,
    current_path: Option<&Path>,
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
            if let Some(path) = current_path.and_then(Path::parent) {
                arguments.push("--filename".into());
                arguments.push(path.as_os_str().into());
            }
        }
        SessionFileDialogMode::SaveAs => {
            arguments.push("--save".into());
            arguments.push("--filename".into());
            arguments.push(default_save_as_path(current_path).into_os_string());
        }
    }
    arguments.extend([
        "--file-filter".into(),
        "Wayscriber sessions | *.wayscriber-session *.session".into(),
        "--file-filter".into(),
        "All files | *".into(),
    ]);
    run_session_file_dialog_command(
        crate::process_broker::HelperKind::SessionZenity,
        "zenity",
        arguments,
    )
}

fn run_kdialog_session_file_dialog(
    mode: SessionFileDialogMode,
    current_path: Option<&Path>,
) -> Result<Option<SessionFileDialogResult>> {
    let mut arguments = Vec::new();
    match mode {
        SessionFileDialogMode::Open => {
            arguments.push("--getopenfilename".into());
            arguments.push(
                current_path
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
                    .unwrap_or_else(default_session_dir)
                    .into_os_string(),
            );
        }
        SessionFileDialogMode::SaveAs => {
            arguments.push("--getsavefilename".into());
            arguments.push(default_save_as_path(current_path).into_os_string());
        }
    }
    arguments.push("Wayscriber sessions (*.wayscriber-session *.session);;All files (*)".into());
    run_session_file_dialog_command(
        crate::process_broker::HelperKind::SessionKdialog,
        "kdialog",
        arguments,
    )
}

fn run_session_file_dialog_command(
    kind: crate::process_broker::HelperKind,
    program: &'static str,
    arguments: Vec<OsString>,
) -> Result<Option<SessionFileDialogResult>> {
    let output = match crate::process_broker::current().and_then(|broker| {
        broker.run(
            kind,
            OsStr::new(program),
            &arguments,
            Vec::new(),
            Duration::from_secs(120),
            64 * 1024,
        )
    }) {
        Ok(output) => output,
        Err(err)
            if crate::process_broker::error_kind(&err)
                == Some(crate::process_broker::BrokerErrorKind::MissingExecutable) =>
        {
            return Ok(None);
        }
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

fn default_session_dir() -> PathBuf {
    crate::paths::home_dir().unwrap_or_else(std::env::temp_dir)
}

pub(in crate::backend::wayland::state::toolbar::events) fn default_save_as_path(
    current_path: Option<&Path>,
) -> PathBuf {
    default_save_as_dir().join(save_as_file_name(current_path))
}

fn default_save_as_dir() -> PathBuf {
    let Some(home) = crate::paths::home_dir() else {
        return std::env::temp_dir();
    };
    let documents = home.join("Documents");
    if documents.is_dir() { documents } else { home }
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
    use crate::backend::wayland::RuntimeWakeSource;

    #[test]
    fn every_dialog_terminal_outcome_wakes_and_is_consumed_once() {
        let outcomes = [
            Ok(Some(PathBuf::from("/tmp/session"))),
            Ok(None),
            Err("chooser error".into()),
        ];
        for expected in outcomes {
            let wake = RuntimeWakeSource::new().unwrap();
            let mut controller = SessionFileDialogController::new(wake.handle());
            let result = expected.clone();
            controller
                .submit(SessionFileDialogMode::SaveAs, move || result)
                .unwrap();

            assert!(wake.wait_readable(Some(Duration::from_secs(1))).unwrap());
            let completion = controller.try_receive().unwrap().unwrap();
            assert_eq!(completion.mode, SessionFileDialogMode::SaveAs);
            assert_eq!(completion.result, expected);
            assert!(controller.try_receive().unwrap().is_none());
            assert!(!wake.drain().unwrap());
        }
    }

    #[test]
    fn panicking_dialog_wakes_idle_runtime_with_one_terminal_failure() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller = SessionFileDialogController::new(wake.handle());
        controller
            .submit(SessionFileDialogMode::Open, || panic!("chooser panicked"))
            .unwrap();

        assert!(wake.wait_readable(Some(Duration::from_secs(1))).unwrap());
        let completion = controller.try_receive().unwrap().unwrap();
        assert_eq!(completion.mode, SessionFileDialogMode::Open);
        assert!(completion.result.unwrap_err().contains("chooser panicked"));
        assert!(controller.try_receive().unwrap().is_none());
        assert!(!wake.drain().unwrap());
    }

    #[test]
    fn active_dialog_rejects_overlap_before_spawning_worker() {
        let wake = RuntimeWakeSource::new().unwrap();
        let mut controller = SessionFileDialogController::new(wake.handle());
        let (release, wait) = std::sync::mpsc::channel();
        controller
            .submit(SessionFileDialogMode::Open, move || {
                wait.recv().unwrap();
                Ok(None)
            })
            .unwrap();

        assert!(
            controller
                .start(SessionFileDialogMode::SaveAs, None)
                .is_err()
        );
        release.send(()).unwrap();
        assert!(wake.wait_readable(Some(Duration::from_secs(1))).unwrap());
        assert!(
            controller
                .try_receive()
                .unwrap()
                .unwrap()
                .result
                .unwrap()
                .is_none()
        );
    }
}
