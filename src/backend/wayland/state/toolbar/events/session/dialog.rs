use anyhow::{Context, Result, anyhow};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
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

type SessionFileDialogMessage = (u64, SessionFileDialogMode, Result<Option<PathBuf>, String>);

#[derive(Debug)]
pub(in crate::backend::wayland::state) struct SessionFileDialogController {
    next_id: u64,
    active: Option<(u64, SessionFileDialogMode)>,
    receiver: Option<mpsc::Receiver<SessionFileDialogMessage>>,
    runtime_wake: crate::backend::wayland::RuntimeWakeHandle,
}

impl SessionFileDialogController {
    pub(in crate::backend::wayland::state) fn new(
        runtime_wake: crate::backend::wayland::RuntimeWakeHandle,
    ) -> Self {
        Self {
            next_id: 1,
            active: None,
            receiver: None,
            runtime_wake,
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
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| anyhow!("session dialog identity exhausted"))?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let wake = self.runtime_wake.clone();
        std::thread::Builder::new()
            .name(format!("wayscriber-session-dialog-{id}"))
            .spawn(move || {
                let result = choose_session_file(mode, current_path.as_deref())
                    .map_err(|error| format!("{error:#}"));
                let _ = sender.send((id, mode, result));
                if let Err(error) = wake.wake() {
                    log::error!("Failed to wake runtime for session dialog completion: {error}");
                }
            })
            .context("failed to start session dialog worker")?;
        self.active = Some((id, mode));
        self.receiver = Some(receiver);
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
        let (id, mode, result) = received;
        if id != expected_id || mode != expected_mode {
            return Err(anyhow!("session dialog completion identity mismatch"));
        }
        Ok(Some(SessionFileDialogCompletion { mode, result }))
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

    fn controller() -> SessionFileDialogController {
        let wake = crate::backend::wayland::RuntimeWakeSource::new().unwrap();
        SessionFileDialogController::new(wake.handle())
    }

    #[test]
    fn completion_identity_mismatch_is_terminal_and_consumed_once() {
        let mut controller = controller();
        let (sender, receiver) = mpsc::sync_channel(1);
        controller.active = Some((7, SessionFileDialogMode::Open));
        controller.receiver = Some(receiver);
        sender
            .send((
                8,
                SessionFileDialogMode::Open,
                Ok(Some(PathBuf::from("/tmp/session"))),
            ))
            .unwrap();

        assert!(controller.try_receive().is_err());
        assert!(controller.try_receive().unwrap().is_none());
    }

    #[test]
    fn worker_disconnect_produces_one_identified_failure() {
        let mut controller = controller();
        let (sender, receiver) = mpsc::sync_channel(1);
        controller.active = Some((9, SessionFileDialogMode::SaveAs));
        controller.receiver = Some(receiver);
        drop(sender);

        let completion = controller.try_receive().unwrap().unwrap();
        assert_eq!(completion.mode, SessionFileDialogMode::SaveAs);
        assert!(
            completion
                .result
                .unwrap_err()
                .contains("without a completion")
        );
        assert!(controller.try_receive().unwrap().is_none());
    }

    #[test]
    fn active_dialog_rejects_overlap_before_spawning_worker() {
        let mut controller = controller();
        controller.active = Some((1, SessionFileDialogMode::Open));
        assert!(
            controller
                .start(SessionFileDialogMode::SaveAs, None)
                .unwrap_err()
                .to_string()
                .contains("already active")
        );
    }
}
