use super::{
    CLIPBOARD_FINGERPRINT_BYTES, CLIPBOARD_FINGERPRINT_TIMEOUT, CLIPBOARD_PUBLISH_COMMAND_TIMEOUT,
    ClipboardPrefixRead, ClipboardReadError, image,
};
use crate::input::state::ClipboardFingerprint;
use command::{ClipboardCommandRunner, WlClipboardCommandRunner};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::process::Child;
use std::sync::mpsc;
use std::time::{Duration, Instant};

mod command;

pub(super) fn publish_selection_clipboard(payload_json: &str) -> Result<(), String> {
    publish_selection_with_runner(payload_json, &WlClipboardCommandRunner)
}

fn publish_selection_with_runner(
    payload_json: &str,
    runner: &impl ClipboardCommandRunner,
) -> Result<(), String> {
    let mut child = runner
        .spawn_copy_selection()
        .map_err(|err| format!("failed to spawn wl-copy: {}", err))?;

    if let Some(mut stdin) = child.stdin.take() {
        if let Err(err) = stdin.write_all(payload_json.as_bytes()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("failed to write to wl-copy stdin: {}", err));
        }
    } else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("wl-copy stdin unavailable".to_string());
    }

    wait_for_wl_copy_publish(child, CLIPBOARD_PUBLISH_COMMAND_TIMEOUT)
}

fn wait_for_wl_copy_publish(mut child: Child, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                let stderr = read_child_stderr(&mut child).unwrap_or_default();
                if stderr.is_empty() {
                    return Err(format!("wl-copy exited unsuccessfully: {}", status));
                }
                return Err(format!(
                    "wl-copy exited unsuccessfully: {} ({})",
                    status, stderr
                ));
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let stderr = read_child_stderr(&mut child).unwrap_or_default();
                if stderr.is_empty() {
                    return Err("wl-copy did not finish publishing before timeout".to_string());
                }
                return Err(format!(
                    "wl-copy did not finish publishing before timeout: {}",
                    stderr
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(err) => return Err(format!("failed to poll wl-copy status: {}", err)),
        }
    }
}

pub(in crate::backend::wayland) fn clipboard_fingerprint() -> Option<ClipboardFingerprint> {
    let offered = list_mime_types().ok()?;
    let selected_mime_type =
        image::choose_supported_mime(&offered).or_else(|| offered.first().cloned());
    let content_sample = selected_mime_type.as_ref().and_then(|mime| {
        read_clipboard_mime_prefix(
            mime,
            CLIPBOARD_FINGERPRINT_BYTES,
            CLIPBOARD_FINGERPRINT_TIMEOUT,
        )
        .ok()
    });
    let bounded_content_hash = content_sample
        .as_ref()
        .map(|sample| content_hash(&sample.bytes));
    let bounded_content_len = content_sample.as_ref().map(|sample| sample.bytes.len());
    let bounded_content_truncated = content_sample
        .as_ref()
        .is_some_and(|sample| sample.truncated);
    Some(ClipboardFingerprint {
        offered_mime_types: offered,
        selected_mime_type,
        bounded_content_hash,
        bounded_content_len,
        bounded_content_truncated,
    })
}

pub(super) fn list_mime_types() -> Result<Vec<String>, ClipboardReadError> {
    list_mime_types_with_runner(&WlClipboardCommandRunner)
}

fn list_mime_types_with_runner(
    runner: &impl ClipboardCommandRunner,
) -> Result<Vec<String>, ClipboardReadError> {
    let output = runner.list_types().map_err(|err| {
        ClipboardReadError::Unavailable(format!("Failed to spawn wl-paste: {}", err))
    })?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.to_ascii_lowercase().contains("nothing is copied")
            || stderr.to_ascii_lowercase().contains("clipboard is empty")
        {
            Err(ClipboardReadError::Empty)
        } else if stderr.is_empty() {
            Err(ClipboardReadError::Other(
                "wl-paste --list-types exited unsuccessfully".to_string(),
            ))
        } else {
            Err(ClipboardReadError::Other(format!(
                "wl-paste --list-types failed: {}",
                stderr
            )))
        }
    }
}

pub(super) fn read_clipboard_mime(
    mime_type: &str,
    limit: usize,
    timeout: Duration,
) -> Result<Vec<u8>, ClipboardReadError> {
    read_clipboard_mime_with_runner(mime_type, limit, timeout, &WlClipboardCommandRunner)
}

fn read_clipboard_mime_prefix(
    mime_type: &str,
    limit: usize,
    timeout: Duration,
) -> Result<ClipboardPrefixRead, ClipboardReadError> {
    read_clipboard_mime_prefix_with_runner(mime_type, limit, timeout, &WlClipboardCommandRunner)
}

fn read_clipboard_mime_with_runner(
    mime_type: &str,
    limit: usize,
    timeout: Duration,
    runner: &impl ClipboardCommandRunner,
) -> Result<Vec<u8>, ClipboardReadError> {
    let mut child = runner.spawn_paste_mime(mime_type).map_err(|err| {
        ClipboardReadError::Unavailable(format!("Failed to spawn wl-paste: {}", err))
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ClipboardReadError::Other("wl-paste stdout unavailable".to_string()))?;
    let result = read_pipe_with_timeout(stdout, limit, timeout);
    match result {
        Err(ClipboardReadError::TimedOut) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(ClipboardReadError::TimedOut)
        }
        Err(ClipboardReadError::TooLarge { limit }) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(ClipboardReadError::TooLarge { limit })
        }
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(err)
        }
        Ok(bytes) => {
            let status = child.wait().map_err(|err| {
                ClipboardReadError::Other(format!("Failed to wait for wl-paste: {}", err))
            })?;
            if status.success() {
                Ok(bytes)
            } else {
                let stderr = read_child_stderr(&mut child).unwrap_or_default();
                if stderr.to_ascii_lowercase().contains("nothing is copied") {
                    Err(ClipboardReadError::Empty)
                } else if stderr.is_empty() {
                    Err(ClipboardReadError::Other(
                        "wl-paste exited unsuccessfully".to_string(),
                    ))
                } else {
                    Err(ClipboardReadError::Other(format!(
                        "wl-paste exited unsuccessfully: {}",
                        stderr
                    )))
                }
            }
        }
    }
}

fn read_clipboard_mime_prefix_with_runner(
    mime_type: &str,
    limit: usize,
    timeout: Duration,
    runner: &impl ClipboardCommandRunner,
) -> Result<ClipboardPrefixRead, ClipboardReadError> {
    let mut child = runner.spawn_paste_mime(mime_type).map_err(|err| {
        ClipboardReadError::Unavailable(format!("Failed to spawn wl-paste: {}", err))
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ClipboardReadError::Other("wl-paste stdout unavailable".to_string()))?;
    let result = read_pipe_prefix_with_timeout(stdout, limit, timeout);
    match result {
        Err(ClipboardReadError::TimedOut) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(ClipboardReadError::TimedOut)
        }
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(err)
        }
        Ok(sample) if sample.truncated => {
            let _ = child.kill();
            let _ = child.wait();
            Ok(sample)
        }
        Ok(sample) => {
            let status = child.wait().map_err(|err| {
                ClipboardReadError::Other(format!("Failed to wait for wl-paste: {}", err))
            })?;
            if status.success() {
                Ok(sample)
            } else {
                let stderr = read_child_stderr(&mut child).unwrap_or_default();
                if stderr.to_ascii_lowercase().contains("nothing is copied") {
                    Err(ClipboardReadError::Empty)
                } else if stderr.is_empty() {
                    Err(ClipboardReadError::Other(
                        "wl-paste exited unsuccessfully".to_string(),
                    ))
                } else {
                    Err(ClipboardReadError::Other(format!(
                        "wl-paste exited unsuccessfully: {}",
                        stderr
                    )))
                }
            }
        }
    }
}

pub(super) fn read_pipe_with_timeout<R>(
    reader: R,
    limit: usize,
    timeout: Duration,
) -> Result<Vec<u8>, ClipboardReadError>
where
    R: Read + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_limited(reader, limit));
    });
    rx.recv_timeout(timeout)
        .map_err(|_| ClipboardReadError::TimedOut)?
}

fn read_pipe_prefix_with_timeout<R>(
    reader: R,
    limit: usize,
    timeout: Duration,
) -> Result<ClipboardPrefixRead, ClipboardReadError>
where
    R: Read + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_prefix(reader, limit));
    });
    rx.recv_timeout(timeout)
        .map_err(|_| ClipboardReadError::TimedOut)?
}

fn read_limited<R: Read>(mut reader: R, limit: usize) -> Result<Vec<u8>, ClipboardReadError> {
    let mut data = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader.read(&mut buffer).map_err(|err| {
            ClipboardReadError::Other(format!("Failed to read clipboard: {}", err))
        })?;
        if read == 0 {
            break;
        }
        if data.len().saturating_add(read) > limit {
            return Err(ClipboardReadError::TooLarge { limit });
        }
        data.extend_from_slice(&buffer[..read]);
    }
    Ok(data)
}

fn read_prefix<R: Read>(
    mut reader: R,
    limit: usize,
) -> Result<ClipboardPrefixRead, ClipboardReadError> {
    let mut data = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        if data.len() >= limit {
            return Ok(ClipboardPrefixRead {
                bytes: data,
                truncated: true,
            });
        }

        let read = reader.read(&mut buffer).map_err(|err| {
            ClipboardReadError::Other(format!("Failed to read clipboard: {}", err))
        })?;
        if read == 0 {
            break;
        }

        let remaining = limit.saturating_sub(data.len());
        if read > remaining {
            data.extend_from_slice(&buffer[..remaining]);
            return Ok(ClipboardPrefixRead {
                bytes: data,
                truncated: true,
            });
        }
        data.extend_from_slice(&buffer[..read]);
    }
    Ok(ClipboardPrefixRead {
        bytes: data,
        truncated: false,
    })
}

fn read_child_stderr(child: &mut std::process::Child) -> Option<String> {
    child.stderr.take().and_then(|mut stderr| {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        let text = String::from_utf8_lossy(&bytes).trim().to_string();
        (!text.is_empty()).then_some(text)
    })
}

fn content_hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests;
