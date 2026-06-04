use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::Command;

const SESSION_FILE_EXTENSION: &str = "wayscriber-session";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland::state::toolbar::events) enum SessionFileDialogMode {
    Open,
    SaveAs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland::state::toolbar::events) enum SessionFileDialogResult {
    Selected(PathBuf),
    Cancelled,
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
    let mut command = Command::new("zenity");
    command
        .arg("--file-selection")
        .arg("--title")
        .arg(match mode {
            SessionFileDialogMode::Open => "Open Wayscriber Session",
            SessionFileDialogMode::SaveAs => "Save Wayscriber Session As",
        });
    match mode {
        SessionFileDialogMode::Open => {
            if let Some(path) = current_path.and_then(Path::parent) {
                command.arg("--filename").arg(path);
            }
        }
        SessionFileDialogMode::SaveAs => {
            command.arg("--save");
            command
                .arg("--filename")
                .arg(default_save_as_path(current_path));
        }
    }
    command
        .arg("--file-filter")
        .arg("Wayscriber sessions | *.wayscriber-session *.session")
        .arg("--file-filter")
        .arg("All files | *");
    run_session_file_dialog_command(command, "zenity")
}

fn run_kdialog_session_file_dialog(
    mode: SessionFileDialogMode,
    current_path: Option<&Path>,
) -> Result<Option<SessionFileDialogResult>> {
    let mut command = Command::new("kdialog");
    match mode {
        SessionFileDialogMode::Open => {
            command.arg("--getopenfilename");
            command.arg(
                current_path
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
                    .unwrap_or_else(default_session_dir),
            );
        }
        SessionFileDialogMode::SaveAs => {
            command.arg("--getsavefilename");
            command.arg(default_save_as_path(current_path));
        }
    }
    command.arg("Wayscriber sessions (*.wayscriber-session *.session);;All files (*)");
    run_session_file_dialog_command(command, "kdialog")
}

fn run_session_file_dialog_command(
    mut command: Command,
    program: &'static str,
) -> Result<Option<SessionFileDialogResult>> {
    let output = match command.output() {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(anyhow!("failed to launch {program}: {err}")),
    };

    let selected = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from);
    if output.status.success() {
        return Ok(Some(match selected {
            Some(path) => SessionFileDialogResult::Selected(path),
            None => SessionFileDialogResult::Cancelled,
        }));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.trim().is_empty() {
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
