use super::super::WaylandState;
use crate::config::{Action, Config};
use crate::draw::frame::UndoAction;
use crate::draw::{EmbeddedImage, Shape};
use crate::input::InputState;
use crate::input::boards::BoardState;
use crate::input::state::{ClipboardPasteRequest, UiToastKind};
use crate::{notification, session};
use std::path::PathBuf;
use std::time::Instant;

const SESSION_PASTE_WARNING_TOAST_MS: u64 = 20_000;
const SESSION_PASTE_NOTIFICATION_TIMEOUT_MS: i32 = 15_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PastePersistenceDecision {
    Allow {
        warning: Option<SessionPasteWarning>,
    },
    Block {
        warning: SessionPasteWarning,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionPasteWarning {
    pub(super) toast: String,
    pub(super) notification: Option<(String, String)>,
    pub(super) log_detail: String,
}

impl SessionPasteWarning {
    pub(super) fn toast_only(message: impl Into<String>) -> Self {
        let toast = message.into();
        Self {
            log_detail: toast.clone(),
            toast,
            notification: None,
        }
    }
}

impl WaylandState {
    pub(super) fn session_paste_preflight_message(
        &self,
        request: &ClipboardPasteRequest,
        image: &EmbeddedImage,
    ) -> Result<PastePersistenceDecision, anyhow::Error> {
        let Some(options) = self.session_options() else {
            return Ok(PastePersistenceDecision::Allow { warning: None });
        };
        if !session_persistence_enabled(options) {
            return Ok(PastePersistenceDecision::Allow { warning: None });
        }
        let preflight_started = Instant::now();
        let snapshot_started = Instant::now();
        let Some(snapshot) = self.snapshot_after_external_image_paste(request, image, options)
        else {
            return Ok(PastePersistenceDecision::Allow { warning: None });
        };
        let snapshot_elapsed = snapshot_started.elapsed();

        let visible_estimate_started = Instant::now();
        let visible_estimate =
            session::estimate_snapshot_without_history_payload(&snapshot, options)?;
        let visible_estimate_elapsed = visible_estimate_started.elapsed();
        if visible_estimate.limit_exceeded.is_some() {
            let estimate = session::SnapshotSaveEstimate {
                full: visible_estimate,
                visible_without_history: visible_estimate,
            };
            log::info!(
                "Session paste preflight for request {} completed in {:?}: snapshot={:?}, visible_estimate={:?}, full_estimate=skipped, visible_written={} bytes, max_file_size={} bytes, visible_limit={:?}",
                request.id,
                preflight_started.elapsed(),
                snapshot_elapsed,
                visible_estimate_elapsed,
                visible_estimate.written_size,
                options.max_file_size_bytes,
                visible_estimate.limit_exceeded
            );
            return Ok(paste_persistence_decision(&estimate, options));
        }

        let full_estimate_started = Instant::now();
        let full_estimate = session::estimate_snapshot_payload(&snapshot, options)?;
        let full_estimate_elapsed = full_estimate_started.elapsed();
        let estimate = session::SnapshotSaveEstimate {
            full: full_estimate,
            visible_without_history: visible_estimate,
        };
        log::info!(
            "Session paste preflight for request {} completed in {:?}: snapshot={:?}, visible_estimate={:?}, full_estimate={:?}, full_written={} bytes, visible_written={} bytes, max_file_size={} bytes, full_limit={:?}, visible_limit={:?}",
            request.id,
            preflight_started.elapsed(),
            snapshot_elapsed,
            visible_estimate_elapsed,
            full_estimate_elapsed,
            estimate.full.written_size,
            estimate.visible_without_history.written_size,
            options.max_file_size_bytes,
            estimate.full.limit_exceeded,
            estimate.visible_without_history.limit_exceeded
        );
        Ok(paste_persistence_decision(&estimate, options))
    }

    fn snapshot_after_external_image_paste(
        &self,
        request: &ClipboardPasteRequest,
        image: &EmbeddedImage,
        options: &session::SessionOptions,
    ) -> Option<session::SessionSnapshot> {
        snapshot_after_external_image_paste_from_input(&self.input_state, request, image, options)
    }

    pub(super) fn show_session_paste_warning(&mut self, warning: SessionPasteWarning) {
        self.input_state.set_ui_toast_with_action_and_duration(
            UiToastKind::Warning,
            warning.toast,
            "Settings",
            Action::OpenConfigurator,
            SESSION_PASTE_WARNING_TOAST_MS,
        );
        if let Some((summary, body)) = warning.notification {
            notification::send_notification_with_timeout_async(
                &self.tokio_handle,
                summary,
                body,
                Some("dialog-warning".to_string()),
                SESSION_PASTE_NOTIFICATION_TIMEOUT_MS,
            );
        }
    }
}

fn snapshot_after_external_image_paste_from_input(
    input: &InputState,
    request: &ClipboardPasteRequest,
    image: &EmbeddedImage,
    options: &session::SessionOptions,
) -> Option<session::SessionSnapshot> {
    let target_board = input
        .boards
        .board_states()
        .iter()
        .find(|board| board.spec.id == request.target_board_id)?;
    if target_board.pages.generation() != request.target_page_generation {
        log::info!(
            "Skipping session paste preflight for request {} because target board '{}' generation changed from {} to {}",
            request.id,
            request.target_board_id,
            request.target_page_generation,
            target_board.pages.generation()
        );
        return None;
    }

    let mut snapshot =
        session::snapshot_from_input(input, options).unwrap_or_else(|| session::SessionSnapshot {
            active_board_id: input.board_id().to_string(),
            boards: Vec::new(),
            tool_state: None,
        });

    if !snapshot
        .boards
        .iter()
        .any(|board| board.id == request.target_board_id)
    {
        if !board_should_persist(target_board, options) {
            return None;
        }
        snapshot.boards.push(session::BoardSnapshot {
            id: target_board.spec.id.clone(),
            pages: snapshot_pages_for_preflight(target_board, input, options),
        });
    }

    let board = snapshot
        .boards
        .iter_mut()
        .find(|board| board.id == request.target_board_id)?;
    let frame = board.pages.pages.get_mut(request.target_page_index)?;
    let shape = Shape::Image {
        x: 0,
        y: 0,
        w: i32::try_from(image.width).unwrap_or(i32::MAX).max(1),
        h: i32::try_from(image.height).unwrap_or(i32::MAX).max(1),
        data: image.clone(),
    };
    let id = frame.try_add_shape_with_id(shape, input.max_shapes_per_frame)?;
    let history_limit = options.effective_history_limit(input.undo_stack_limit);
    if history_limit > 0 {
        let (index, stored) = frame
            .find_index(id)
            .and_then(|index| frame.shape(id).map(|shape| (index, shape.clone())))?;
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(index, stored)],
            },
            history_limit,
        );
    }
    Some(snapshot)
}

fn board_should_persist(board: &BoardState, options: &session::SessionOptions) -> bool {
    if board.spec.background.is_transparent() {
        options.persist_transparent
    } else {
        (options.persist_whiteboard || options.persist_blackboard) && board.spec.persist
    }
}

fn snapshot_pages_for_preflight(
    board: &BoardState,
    input: &InputState,
    options: &session::SessionOptions,
) -> session::BoardPagesSnapshot {
    let history_limit = options.effective_history_limit(input.undo_stack_limit);
    let mut pages = board.pages.pages().to_vec();
    for page in &mut pages {
        if history_limit == 0 {
            page.clamp_history_depth(0);
        } else if history_limit < usize::MAX {
            page.clamp_history_depth(history_limit);
        }
    }
    session::BoardPagesSnapshot {
        pages,
        active: board.pages.active_index(),
    }
}

fn paste_persistence_decision(
    estimate: &session::SnapshotSaveEstimate,
    options: &session::SessionOptions,
) -> PastePersistenceDecision {
    let limit = format_bytes(options.max_file_size_bytes);
    if let Some(limit_exceeded) = estimate.visible_without_history.limit_exceeded {
        let warning = visible_limit_warning(
            limit_exceeded,
            &estimate.visible_without_history,
            &limit,
            options,
        );
        return PastePersistenceDecision::Block { warning };
    }

    if let Some(limit_exceeded) = estimate.full.limit_exceeded {
        return PastePersistenceDecision::Allow {
            warning: Some(full_limit_warning(limit_exceeded, &estimate.full, options)),
        };
    }

    if estimate.full.is_near_limit() {
        let written = format_bytes(estimate.full.written_size as u64);
        let suggested_limit_mb = suggested_limit_mb(
            estimate.full.written_size as u64,
            options.max_file_size_bytes,
        );
        return PastePersistenceDecision::Allow {
            warning: Some(SessionPasteWarning::toast_only(format!(
                "Image pasted. Session near limit: {written}/{limit}. Consider {suggested_limit_mb} MiB."
            ))),
        };
    }

    PastePersistenceDecision::Allow { warning: None }
}

fn visible_limit_warning(
    limit_exceeded: session::SaveLimitExceeded,
    estimate: &session::SnapshotPayloadEstimate,
    formatted_file_limit: &str,
    options: &session::SessionOptions,
) -> SessionPasteWarning {
    match limit_exceeded {
        session::SaveLimitExceeded::WrittenSize { .. } => {
            let suggested_limit_mb =
                suggested_limit_mb(estimate.written_size as u64, options.max_file_size_bytes);
            let written = format_bytes(estimate.written_size as u64);
            let toast = format!(
                "Image blocked: {written}/{formatted_file_limit}. Set {suggested_limit_mb} MiB."
            );
            let body = format!(
                "Paste would save as {written}, over the {formatted_file_limit} cap. Open Settings > Session > Max file size and set {suggested_limit_mb} MiB, or edit {}.",
                config_path_display()
            );
            SessionPasteWarning {
                toast,
                notification: Some(("Image Paste Blocked".to_string(), body)),
                log_detail: format!(
                    "visible payload {}, suggested max_file_size_mb={suggested_limit_mb}",
                    describe_limit(limit_exceeded)
                ),
            }
        }
        session::SaveLimitExceeded::ExpandedSize {
            raw_size,
            max_expanded_size,
        } => {
            let raw = format_bytes(raw_size);
            let expanded_limit = format_bytes(max_expanded_size);
            SessionPasteWarning {
                toast: format!("Image blocked: restore safety {raw}/{expanded_limit}."),
                notification: Some((
                    "Image Paste Blocked".to_string(),
                    format!(
                        "Paste would expand to {raw}, over the {expanded_limit} restore safety limit. Reduce images or history; max_file_size_mb will not help."
                    ),
                )),
                log_detail: format!("visible payload {}", describe_limit(limit_exceeded)),
            }
        }
    }
}

fn full_limit_warning(
    limit_exceeded: session::SaveLimitExceeded,
    estimate: &session::SnapshotPayloadEstimate,
    options: &session::SessionOptions,
) -> SessionPasteWarning {
    match limit_exceeded {
        session::SaveLimitExceeded::WrittenSize { .. } => {
            let suggested_limit_mb =
                suggested_limit_mb(estimate.written_size as u64, options.max_file_size_bytes);
            let toast =
                format!("Image pasted. Undo history may be dropped. Set {suggested_limit_mb} MiB.");
            let body = format!(
                "Drawing data fits, but undo history may be dropped on save. Set Session > Max file size to {suggested_limit_mb} MiB, or edit {}.",
                config_path_display()
            );
            SessionPasteWarning {
                toast,
                notification: Some(("Session Undo History at Risk".to_string(), body)),
                log_detail: format!(
                    "full payload {}, suggested max_file_size_mb={suggested_limit_mb}",
                    describe_limit(limit_exceeded)
                ),
            }
        }
        session::SaveLimitExceeded::ExpandedSize {
            raw_size,
            max_expanded_size,
        } => {
            let raw = format_bytes(raw_size);
            let expanded_limit = format_bytes(max_expanded_size);
            SessionPasteWarning {
                toast: "Image pasted. Undo history may be dropped for restore safety.".to_string(),
                notification: Some((
                    "Session Undo History at Risk".to_string(),
                    format!(
                        "Full session would expand to {raw}, over the {expanded_limit} restore safety limit. Visible drawing data can still be saved."
                    ),
                )),
                log_detail: format!("full payload {}", describe_limit(limit_exceeded)),
            }
        }
    }
}

fn session_persistence_enabled(options: &session::SessionOptions) -> bool {
    options.any_enabled() || options.restore_tool_state || options.persist_history
}

fn suggested_limit_mb(projected_written_size: u64, current_limit_bytes: u64) -> u64 {
    const MIB: u64 = 1024 * 1024;
    let projected_mb = projected_written_size.div_ceil(MIB);
    let current_mb = current_limit_bytes.div_ceil(MIB);
    projected_mb
        .saturating_mul(3)
        .div_ceil(2)
        .max(current_mb.saturating_mul(3).div_ceil(2))
        .clamp(1, 1024)
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} bytes")
    }
}

fn describe_limit(limit: session::SaveLimitExceeded) -> String {
    match limit {
        session::SaveLimitExceeded::WrittenSize {
            written_size,
            max_file_size,
        } => format!(
            "writes as {} and exceeds configured cap {}",
            format_bytes(written_size),
            format_bytes(max_file_size)
        ),
        session::SaveLimitExceeded::ExpandedSize {
            raw_size,
            max_expanded_size,
        } => format!(
            "expands to {} and exceeds restore safety cap {}",
            format_bytes(raw_size),
            format_bytes(max_expanded_size)
        ),
    }
}

fn config_path_display() -> String {
    Config::get_config_path()
        .map(compact_path)
        .unwrap_or_else(|_| "~/.config/wayscriber/config.toml".to_string())
}

fn compact_path(path: PathBuf) -> String {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return path.display().to_string();
    };
    match path.strip_prefix(&home) {
        Ok(stripped) if !stripped.as_os_str().is_empty() => {
            format!("~/{}", stripped.display())
        }
        Ok(_) => "~".to_string(),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests;
