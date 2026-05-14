//! Clipboard publish and paste helpers for drawable shape selections and images.

use super::WaylandState;
use crate::draw::EmbeddedImage;
use crate::image_decode::{decode_rgba, format_from_mime_or_bytes, image_dimensions};
use crate::input::state::{
    ClipboardFingerprint, ClipboardPasteRequest, UiToastKind, WayscriberClipboardSelection,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub(super) const WAYSCRIBER_SELECTION_MIME: &str = "application/vnd.wayscriber.selection+json";

// A pasted image is persisted in the visible frame and in the Create undo action.
// Keep one accepted image comfortably below the default 10 MiB session JSON budget.
const MAX_CLIPBOARD_IMAGE_BYTES: usize = 3 * 1024 * 1024;
const MAX_CLIPBOARD_SELECTION_BYTES: usize = 2 * 1024 * 1024;
const MAX_CLIPBOARD_IMAGE_PIXELS: u64 = 48_000_000;
const CLIPBOARD_READ_TIMEOUT: Duration = Duration::from_millis(1500);
const CLIPBOARD_FINGERPRINT_BYTES: usize = 4096;
const CLIPBOARD_FINGERPRINT_TIMEOUT: Duration = Duration::from_millis(300);
const CLIPBOARD_PUBLISH_COMMAND_TIMEOUT: Duration = Duration::from_millis(750);

#[derive(Debug)]
pub(in crate::backend::wayland) struct ClipboardPasteCompletion {
    request: ClipboardPasteRequest,
    result: ClipboardPasteResult,
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct ClipboardPublishCompletion {
    generation: u64,
    fingerprint: Option<ClipboardFingerprint>,
    copied: bool,
    warning: Option<&'static str>,
}

#[derive(Debug)]
enum ClipboardPasteResult {
    PrivateSelection(WayscriberClipboardSelection),
    Image(EmbeddedImage),
    ClipboardEmpty,
    NoSupportedMime { offered: Vec<String> },
    TooLarge { limit: usize },
    TooManyPixels { width: u32, height: u32, limit: u64 },
    DecodeFailed(String),
    ReadTimedOut,
    ClipboardUnavailable(String),
    ClipboardError(String),
    MalformedPrivateSelection(String),
}

#[derive(Debug)]
enum ClipboardReadError {
    Empty,
    TooLarge { limit: usize },
    TimedOut,
    Unavailable(String),
    Other(String),
}

#[derive(Debug)]
struct ClipboardPrefixRead {
    bytes: Vec<u8>,
    truncated: bool,
}

impl WaylandState {
    pub(in crate::backend::wayland) fn drain_clipboard_requests(&mut self) {
        if self.clipboard_publish_rx.is_none()
            && let Some(request) = self.input_state.take_pending_selection_clipboard_publish()
        {
            self.start_selection_clipboard_publish(request.generation, request.payload_json);
        }

        if let Some(request) = self.input_state.take_pending_clipboard_paste_request() {
            self.start_clipboard_paste(request);
        }
    }

    pub(in crate::backend::wayland) fn poll_clipboard_publish_completion(&mut self) {
        let Some(rx) = &self.clipboard_publish_rx else {
            return;
        };
        let Ok(completion) = rx.try_recv() else {
            return;
        };
        self.clipboard_publish_rx = None;
        self.apply_selection_clipboard_publish_completion(completion);
    }

    pub(in crate::backend::wayland) fn poll_clipboard_paste_completion(&mut self) {
        let Some(rx) = &self.clipboard_paste_rx else {
            return;
        };
        let Ok(completion) = rx.try_recv() else {
            return;
        };
        self.clipboard_paste_rx = None;
        self.apply_clipboard_paste_completion(completion);
    }

    fn start_selection_clipboard_publish(&mut self, generation: u64, payload_json: String) {
        self.suppress_focus_exit_for(Duration::from_millis(1500));
        let (tx, rx) = mpsc::channel();
        self.clipboard_publish_rx = Some(rx);
        std::thread::spawn(move || {
            let completion = resolve_selection_clipboard_publish(generation, payload_json);
            let _ = tx.send(completion);
        });
    }

    fn apply_selection_clipboard_publish_completion(
        &mut self,
        completion: ClipboardPublishCompletion,
    ) {
        let applied = self.input_state.complete_selection_clipboard_publish(
            completion.generation,
            completion.fingerprint,
            completion.copied,
        );
        if applied && let Some(warning) = completion.warning {
            self.input_state.set_ui_toast(UiToastKind::Warning, warning);
        }
    }

    fn start_clipboard_paste(&mut self, request: ClipboardPasteRequest) {
        self.suppress_focus_exit_for(Duration::from_millis(1500));

        if let Some(shapes) = self
            .input_state
            .local_selection_shapes_for_pending_publish(request.local_selection_fallback_generation)
        {
            let pasted = self
                .input_state
                .paste_clipboard_shapes_from_request(&request, shapes);
            self.input_state.finish_clipboard_paste_request(request.id);
            if pasted == 0 {
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, "Nothing pasted.");
                self.input_state.trigger_blocked_feedback();
            }
            return;
        }

        if self
            .input_state
            .has_failed_local_selection_for_generation(request.local_selection_fallback_generation)
        {
            let fingerprint = clipboard_fingerprint();
            if let Some(shapes) = self
                .input_state
                .failed_local_selection_after_fingerprint_probe(
                    request.local_selection_fallback_generation,
                    fingerprint,
                )
            {
                let pasted = self
                    .input_state
                    .paste_clipboard_shapes_from_request(&request, shapes);
                self.input_state.finish_clipboard_paste_request(request.id);
                if pasted == 0 {
                    self.input_state
                        .set_ui_toast(UiToastKind::Warning, "Nothing pasted.");
                    self.input_state.trigger_blocked_feedback();
                }
                return;
            }
        }

        let (tx, rx) = mpsc::channel();
        self.clipboard_paste_rx = Some(rx);
        std::thread::spawn(move || {
            let result = resolve_system_clipboard();
            let _ = tx.send(ClipboardPasteCompletion { request, result });
        });
    }

    fn apply_clipboard_paste_completion(&mut self, completion: ClipboardPasteCompletion) {
        let ClipboardPasteCompletion { request, result } = completion;
        if self.input_state.active_clipboard_paste_request_id() != Some(request.id) {
            return;
        }

        match result {
            ClipboardPasteResult::PrivateSelection(payload) => {
                let payload_matches_local = self
                    .input_state
                    .private_payload_matches_request_selection(&request, &payload);
                let shapes = self
                    .input_state
                    .private_payload_shapes_for_request(&request, payload);
                if !payload_matches_local {
                    self.input_state
                        .mark_selection_clipboard_superseded_for_generation(
                            request.local_selection_fallback_generation,
                        );
                }
                let pasted = shapes
                    .map(|shapes| {
                        self.input_state
                            .paste_clipboard_shapes_from_request(&request, shapes)
                    })
                    .unwrap_or(0);
                if pasted == 0 {
                    self.input_state
                        .set_ui_toast(UiToastKind::Warning, "No shapes pasted.");
                    self.input_state.trigger_blocked_feedback();
                }
            }
            ClipboardPasteResult::Image(image) => {
                self.input_state
                    .mark_selection_clipboard_superseded_for_generation(
                        request.local_selection_fallback_generation,
                    );
                if !self
                    .input_state
                    .paste_external_image_from_request(&request, image)
                {
                    self.input_state.trigger_blocked_feedback();
                }
            }
            ClipboardPasteResult::ClipboardEmpty => {
                self.input_state
                    .mark_selection_clipboard_superseded_for_generation(
                        request.local_selection_fallback_generation,
                    );
                self.input_state
                    .set_ui_toast(UiToastKind::Warning, "System clipboard is empty.");
                self.input_state.trigger_blocked_feedback();
            }
            ClipboardPasteResult::ClipboardUnavailable(err) => {
                log::warn!("Clipboard unavailable: {}", err);
                self.paste_local_fallback_or_warn(
                    &request,
                    "System clipboard unavailable.",
                    "System clipboard unavailable; pasted local copy.",
                );
            }
            ClipboardPasteResult::NoSupportedMime { offered } => {
                log::debug!("Unsupported clipboard MIME types: {:?}", offered);
                self.input_state
                    .mark_selection_clipboard_superseded_for_generation(
                        request.local_selection_fallback_generation,
                    );
                self.input_state.set_ui_toast(
                    UiToastKind::Warning,
                    "Clipboard content is not a supported image or Wayscriber selection.",
                );
                self.input_state.trigger_blocked_feedback();
            }
            ClipboardPasteResult::TooLarge { limit } => {
                self.input_state
                    .mark_selection_clipboard_superseded_for_generation(
                        request.local_selection_fallback_generation,
                    );
                self.input_state.set_ui_toast(
                    UiToastKind::Warning,
                    format!(
                        "Clipboard data is too large to paste (limit {} MB).",
                        limit / 1024 / 1024
                    ),
                );
                self.input_state.trigger_blocked_feedback();
            }
            ClipboardPasteResult::TooManyPixels {
                width,
                height,
                limit,
            } => {
                self.input_state
                    .mark_selection_clipboard_superseded_for_generation(
                        request.local_selection_fallback_generation,
                    );
                self.input_state.set_ui_toast(
                    UiToastKind::Warning,
                    format!(
                        "Clipboard image is too large ({}x{}, limit {} pixels).",
                        width, height, limit
                    ),
                );
                self.input_state.trigger_blocked_feedback();
            }
            ClipboardPasteResult::DecodeFailed(err) => {
                log::warn!("Clipboard image decode failed: {}", err);
                self.input_state
                    .mark_selection_clipboard_superseded_for_generation(
                        request.local_selection_fallback_generation,
                    );
                self.input_state.set_ui_toast(
                    UiToastKind::Warning,
                    "Clipboard image could not be decoded.",
                );
                self.input_state.trigger_blocked_feedback();
            }
            ClipboardPasteResult::ReadTimedOut => {
                self.paste_local_fallback_or_warn(
                    &request,
                    "Clipboard read timed out.",
                    "Clipboard read timed out; pasted local copy.",
                );
            }
            ClipboardPasteResult::ClipboardError(err) => {
                log::warn!("Clipboard paste error: {}", err);
                self.paste_local_fallback_or_warn(
                    &request,
                    "Failed to read clipboard.",
                    "Failed to read clipboard; pasted local copy.",
                );
            }
            ClipboardPasteResult::MalformedPrivateSelection(err) => {
                log::warn!("Malformed Wayscriber clipboard payload: {}", err);
                self.input_state
                    .mark_selection_clipboard_superseded_for_generation(
                        request.local_selection_fallback_generation,
                    );
                self.input_state.set_ui_toast(
                    UiToastKind::Warning,
                    "Wayscriber clipboard selection could not be read.",
                );
                self.input_state.trigger_blocked_feedback();
            }
        }

        self.input_state.finish_clipboard_paste_request(request.id);
    }

    fn paste_local_fallback_or_warn(
        &mut self,
        request: &ClipboardPasteRequest,
        warning: &'static str,
        fallback_message: &'static str,
    ) {
        if let Some(generation) = request.local_selection_fallback_generation
            && let Some(shapes) = self
                .input_state
                .local_selection_shapes_for_fallback(generation)
        {
            let pasted = self
                .input_state
                .paste_clipboard_shapes_from_request(request, shapes);
            if pasted > 0 {
                self.input_state
                    .set_ui_toast(UiToastKind::Info, fallback_message);
                return;
            }
        }

        self.input_state.set_ui_toast(UiToastKind::Warning, warning);
        self.input_state.trigger_blocked_feedback();
    }
}

fn resolve_selection_clipboard_publish(
    generation: u64,
    payload_json: String,
) -> ClipboardPublishCompletion {
    if payload_json.len() > MAX_CLIPBOARD_SELECTION_BYTES {
        return ClipboardPublishCompletion {
            generation,
            fingerprint: clipboard_fingerprint(),
            copied: false,
            warning: Some("Copied locally; selection is too large for system clipboard."),
        };
    }

    let copied = match std::panic::catch_unwind(|| publish_selection_clipboard(&payload_json)) {
        Ok(Ok(())) => true,
        Ok(Err(err)) => {
            log::warn!("Selection clipboard publish failed: {}", err);
            false
        }
        Err(_) => {
            log::error!("Selection clipboard publish panicked");
            false
        }
    };

    ClipboardPublishCompletion {
        generation,
        fingerprint: if copied {
            None
        } else {
            clipboard_fingerprint()
        },
        copied,
        warning: (!copied).then_some("Copied locally; system clipboard unavailable."),
    }
}

fn publish_selection_clipboard(payload_json: &str) -> Result<(), String> {
    publish_selection_via_command(payload_json)
}

fn publish_selection_via_command(payload_json: &str) -> Result<(), String> {
    let mut child = Command::new("wl-copy")
        .arg("--type")
        .arg(WAYSCRIBER_SELECTION_MIME)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
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

fn resolve_system_clipboard() -> ClipboardPasteResult {
    let offered = match list_mime_types() {
        Ok(types) if types.is_empty() => return ClipboardPasteResult::ClipboardEmpty,
        Ok(types) => types,
        Err(ClipboardReadError::Empty) => return ClipboardPasteResult::ClipboardEmpty,
        Err(ClipboardReadError::Unavailable(err)) => {
            return ClipboardPasteResult::ClipboardUnavailable(err);
        }
        Err(err) => return map_read_error(err),
    };

    let Some(mime_type) = choose_supported_mime(&offered) else {
        return ClipboardPasteResult::NoSupportedMime { offered };
    };
    let limit = if mime_type == WAYSCRIBER_SELECTION_MIME {
        MAX_CLIPBOARD_SELECTION_BYTES
    } else {
        MAX_CLIPBOARD_IMAGE_BYTES
    };

    let bytes = match read_clipboard_mime(&mime_type, limit, CLIPBOARD_READ_TIMEOUT) {
        Ok(bytes) if bytes.is_empty() => return ClipboardPasteResult::ClipboardEmpty,
        Ok(bytes) => bytes,
        Err(err) => return map_read_error(err),
    };

    if mime_type == WAYSCRIBER_SELECTION_MIME {
        return serde_json::from_slice::<WayscriberClipboardSelection>(&bytes)
            .map(ClipboardPasteResult::PrivateSelection)
            .unwrap_or_else(|err| {
                ClipboardPasteResult::MalformedPrivateSelection(err.to_string())
            });
    }

    decode_clipboard_image(&mime_type, bytes)
}

fn clipboard_fingerprint() -> Option<ClipboardFingerprint> {
    let offered = list_mime_types().ok()?;
    let selected_mime_type = choose_supported_mime(&offered).or_else(|| offered.first().cloned());
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

fn choose_supported_mime(offered: &[String]) -> Option<String> {
    [
        WAYSCRIBER_SELECTION_MIME,
        "image/png",
        "image/jpeg",
        "image/jpg",
    ]
    .into_iter()
    .find(|candidate| offered.iter().any(|mime| mime == candidate))
    .map(ToString::to_string)
}

fn decode_clipboard_image(mime_type: &str, bytes: Vec<u8>) -> ClipboardPasteResult {
    let Some(format) = format_from_mime_or_bytes(mime_type, &bytes) else {
        return ClipboardPasteResult::DecodeFailed(format!("unsupported MIME type {}", mime_type));
    };
    let dimensions = match image_dimensions(format, &bytes) {
        Ok(dimensions) => dimensions,
        Err(err) => return ClipboardPasteResult::DecodeFailed(err),
    };
    let pixels = dimensions.0 as u64 * dimensions.1 as u64;
    if pixels > MAX_CLIPBOARD_IMAGE_PIXELS {
        return ClipboardPasteResult::TooManyPixels {
            width: dimensions.0,
            height: dimensions.1,
            limit: MAX_CLIPBOARD_IMAGE_PIXELS,
        };
    }
    if let Err(err) = decode_rgba(format, &bytes) {
        return ClipboardPasteResult::DecodeFailed(err);
    }
    ClipboardPasteResult::Image(EmbeddedImage {
        mime_type: if mime_type == "image/jpg" {
            "image/jpeg".to_string()
        } else {
            mime_type.to_string()
        },
        width: dimensions.0,
        height: dimensions.1,
        bytes,
    })
}

fn list_mime_types() -> Result<Vec<String>, ClipboardReadError> {
    list_mime_types_via_command()
}

fn list_mime_types_via_command() -> Result<Vec<String>, ClipboardReadError> {
    let output = Command::new("wl-paste")
        .arg("--list-types")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| {
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

fn read_clipboard_mime(
    mime_type: &str,
    limit: usize,
    timeout: Duration,
) -> Result<Vec<u8>, ClipboardReadError> {
    read_clipboard_mime_via_command(mime_type, limit, timeout)
}

fn read_clipboard_mime_prefix(
    mime_type: &str,
    limit: usize,
    timeout: Duration,
) -> Result<ClipboardPrefixRead, ClipboardReadError> {
    read_clipboard_mime_prefix_via_command(mime_type, limit, timeout)
}

fn read_clipboard_mime_via_command(
    mime_type: &str,
    limit: usize,
    timeout: Duration,
) -> Result<Vec<u8>, ClipboardReadError> {
    let mut child = Command::new("wl-paste")
        .arg("--type")
        .arg(mime_type)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
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

fn read_clipboard_mime_prefix_via_command(
    mime_type: &str,
    limit: usize,
    timeout: Duration,
) -> Result<ClipboardPrefixRead, ClipboardReadError> {
    let mut child = Command::new("wl-paste")
        .arg("--type")
        .arg(mime_type)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
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

fn read_pipe_with_timeout<R>(
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

fn map_read_error(err: ClipboardReadError) -> ClipboardPasteResult {
    match err {
        ClipboardReadError::Empty => ClipboardPasteResult::ClipboardEmpty,
        ClipboardReadError::TooLarge { limit } => ClipboardPasteResult::TooLarge { limit },
        ClipboardReadError::TimedOut => ClipboardPasteResult::ReadTimedOut,
        ClipboardReadError::Unavailable(err) => ClipboardPasteResult::ClipboardUnavailable(err),
        ClipboardReadError::Other(err) => ClipboardPasteResult::ClipboardError(err),
    }
}

fn content_hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor, Read};

    struct ExactLimitThenError {
        bytes: Vec<u8>,
        read_once: bool,
    }

    impl Read for ExactLimitThenError {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.read_once {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "unexpected second read",
                ));
            }
            self.read_once = true;
            let len = self.bytes.len().min(buffer.len());
            buffer[..len].copy_from_slice(&self.bytes[..len]);
            Ok(len)
        }
    }

    #[test]
    fn strict_read_rejects_data_over_limit() {
        let err = read_limited(Cursor::new(vec![1, 2, 3, 4, 5]), 4).expect_err("over limit");

        assert!(matches!(err, ClipboardReadError::TooLarge { limit: 4 }));
    }

    #[test]
    fn prefix_read_returns_bounded_sample_for_data_over_limit() {
        let sample = read_prefix(Cursor::new(vec![1, 2, 3, 4, 5]), 4).expect("prefix sample");

        assert_eq!(sample.bytes, vec![1, 2, 3, 4]);
        assert!(sample.truncated);
    }

    #[test]
    fn prefix_read_returns_when_first_read_reaches_limit() {
        let reader = ExactLimitThenError {
            bytes: vec![1, 2, 3, 4],
            read_once: false,
        };
        let sample = read_prefix(reader, 4).expect("prefix sample");

        assert_eq!(sample.bytes, vec![1, 2, 3, 4]);
        assert!(sample.truncated);
    }

    #[test]
    fn prefix_read_marks_small_data_untruncated() {
        let sample = read_prefix(Cursor::new(vec![1, 2, 3]), 4).expect("prefix sample");

        assert_eq!(sample.bytes, vec![1, 2, 3]);
        assert!(!sample.truncated);
    }

    #[test]
    fn image_byte_cap_leaves_room_for_default_persisted_create_history() {
        let encoded_len = MAX_CLIPBOARD_IMAGE_BYTES.div_ceil(3) * 4;
        let duplicated_history_len = encoded_len * 2;
        let default_session_budget = 10 * 1024 * 1024;
        let json_margin = 512 * 1024;

        assert!(duplicated_history_len + json_margin < default_session_budget);
    }
}
