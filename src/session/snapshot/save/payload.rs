use super::*;

const NEAR_LIMIT_PERCENT: u64 = 90;

pub(super) struct PayloadCandidate {
    pub(super) bytes: Vec<u8>,
    pub(super) raw_size: usize,
    pub(super) compressed: bool,
}

impl PayloadCandidate {
    pub(super) fn final_size(&self) -> usize {
        self.bytes.len()
    }

    pub(super) fn limit_exceeded(
        &self,
        options: &SessionOptions,
        max_expanded_size: u64,
    ) -> Option<SaveLimitExceeded> {
        if self.final_size() as u64 > options.max_file_size_bytes {
            return Some(SaveLimitExceeded::WrittenSize {
                written_size: self.final_size() as u64,
                max_file_size: options.max_file_size_bytes,
            });
        }
        if self.compressed && self.raw_size as u64 > max_expanded_size {
            return Some(SaveLimitExceeded::ExpandedSize {
                raw_size: self.raw_size as u64,
                max_expanded_size,
            });
        }
        None
    }

    pub(super) fn fits_limit(&self, options: &SessionOptions, max_expanded_size: u64) -> bool {
        self.limit_exceeded(options, max_expanded_size).is_none()
    }

    pub(super) fn expanded_limit_exceeded(
        &self,
        max_expanded_size: u64,
    ) -> Option<SaveLimitExceeded> {
        if self.raw_size as u64 > max_expanded_size {
            Some(SaveLimitExceeded::ExpandedSize {
                raw_size: self.raw_size as u64,
                max_expanded_size,
            })
        } else {
            None
        }
    }

    pub(super) fn fits_expanded_limit(&self, max_expanded_size: u64) -> bool {
        self.expanded_limit_exceeded(max_expanded_size).is_none()
    }
}

pub(super) struct PreparedPayload {
    pub(super) payload: Option<PayloadCandidate>,
    pub(super) outcome: SaveSnapshotOutcome,
    pub(super) raw_size: usize,
    pub(super) compressed: bool,
}

impl PreparedPayload {
    pub(super) fn write(payload: PayloadCandidate, outcome: SaveSnapshotOutcome) -> Self {
        Self {
            raw_size: payload.raw_size,
            compressed: payload.compressed,
            payload: Some(payload),
            outcome,
        }
    }

    pub(super) fn clear(raw_size: usize, compressed: bool) -> Self {
        Self {
            payload: None,
            outcome: SaveSnapshotOutcome::ClearedEmpty,
            raw_size,
            compressed,
        }
    }
}

pub(super) fn payload_within_limit(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    last_modified: &str,
    max_expanded_size: u64,
    history_fallback: HistoryFallbackStrategy,
) -> Result<PreparedPayload> {
    if snapshot.is_empty() && snapshot.tool_state.is_none() {
        return Ok(PreparedPayload::clear(0, false));
    }

    let full_started = Instant::now();
    let full_payload = payload_candidate(snapshot, options, last_modified)?;
    log_payload_candidate("full", &full_payload, full_started.elapsed());
    if full_payload.fits_limit(options, max_expanded_size) {
        return Ok(PreparedPayload::write(
            full_payload,
            SaveSnapshotOutcome::Full,
        ));
    }

    let full_raw_size = full_payload.raw_size;
    let full_final_size = full_payload.final_size();
    let full_limit = full_payload
        .limit_exceeded(options, max_expanded_size)
        .expect("full payload should exceed a save/load limit");
    let visible_only = snapshot_without_history(snapshot);
    if visible_only.is_empty() && visible_only.tool_state.is_none() {
        warn!(
            "Full session payload cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); dropping undo/redo history leaves no visible session data, clearing saved session",
            full_limit.description(),
            full_final_size,
            full_raw_size,
            full_payload.compressed
        );
        return Ok(PreparedPayload::clear(
            full_raw_size,
            full_payload.compressed,
        ));
    }

    let visible_started = Instant::now();
    let visible_payload = payload_candidate(&visible_only, options, last_modified)?;
    log_payload_candidate("visible-only", &visible_payload, visible_started.elapsed());
    if !visible_payload.fits_limit(options, max_expanded_size) {
        let visible_limit = visible_payload
            .limit_exceeded(options, max_expanded_size)
            .expect("visible payload should exceed a save/load limit");
        return Err(SavePayloadTooLarge {
            limit: visible_limit,
            written_size: visible_payload.final_size(),
            raw_size: visible_payload.raw_size,
            compressed: visible_payload.compressed,
        }
        .into());
    }

    let history_depth = max_history_depth(snapshot);
    if history_depth > 0 {
        let visible_near_limit = is_near_limit(
            visible_payload.final_size() as u64,
            options.max_file_size_bytes,
        );
        let depth_one_started = Instant::now();
        let depth_one_candidate = snapshot_with_history_depth(snapshot, 1);
        let depth_one_payload = payload_candidate(&depth_one_candidate, options, last_modified)?;
        log_payload_candidate(
            "history-depth 1",
            &depth_one_payload,
            depth_one_started.elapsed(),
        );

        if depth_one_payload.fits_limit(options, max_expanded_size) {
            let fitting_history = if history_depth == 1 || visible_near_limit {
                if visible_near_limit && history_depth > 1 {
                    warn!(
                        "Visible-only session payload is already near the configured limit ({} of {} bytes); keeping one history entry and skipping deeper history-depth scan",
                        visible_payload.final_size(),
                        options.max_file_size_bytes
                    );
                }
                Some((1, depth_one_payload))
            } else {
                fitting_history_payload(
                    snapshot,
                    history_depth,
                    options,
                    last_modified,
                    max_expanded_size,
                    history_fallback,
                )?
                .or(Some((1, depth_one_payload)))
            };

            if let Some((depth, payload)) = fitting_history {
                warn!(
                    "Full session payload cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); saving recent {} undo/redo history entries per stack ({} bytes written from {} raw bytes, compression={})",
                    full_limit.description(),
                    full_final_size,
                    full_raw_size,
                    full_payload.compressed,
                    depth,
                    payload.final_size(),
                    payload.raw_size,
                    payload.compressed
                );
                return Ok(PreparedPayload::write(
                    payload,
                    SaveSnapshotOutcome::TrimmedHistory { depth },
                ));
            }
        } else {
            let depth_one_limit = depth_one_payload
                .limit_exceeded(options, max_expanded_size)
                .expect("depth-one payload should exceed a save/load limit");
            warn!(
                "Even one persisted history entry cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); skipping history-depth scan and saving visible data only",
                depth_one_limit.description(),
                depth_one_payload.final_size(),
                depth_one_payload.raw_size,
                depth_one_payload.compressed
            );
        }
    }

    warn!(
        "Full session payload cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); saving visible data without undo/redo history ({} bytes written from {} raw bytes, compression={})",
        full_limit.description(),
        full_final_size,
        full_raw_size,
        full_payload.compressed,
        visible_payload.final_size(),
        visible_payload.raw_size,
        visible_payload.compressed
    );
    Ok(PreparedPayload::write(
        visible_payload,
        SaveSnapshotOutcome::VisibleOnly,
    ))
}

fn fitting_history_payload(
    snapshot: &SessionSnapshot,
    history_depth: usize,
    options: &SessionOptions,
    last_modified: &str,
    max_expanded_size: u64,
    history_fallback: HistoryFallbackStrategy,
) -> Result<Option<(usize, PayloadCandidate)>> {
    match history_fallback {
        HistoryFallbackStrategy::LargestFitting => largest_fitting_history_payload(
            snapshot,
            2,
            history_depth,
            options,
            last_modified,
            max_expanded_size,
        ),
        HistoryFallbackStrategy::Bounded { max_depth } => {
            let max_depth = max_depth.min(history_depth);
            if max_depth < 2 {
                debug!(
                    "Autosave history fallback capped at depth {}; skipping deeper history-depth scan",
                    max_depth
                );
                return Ok(None);
            }

            largest_fitting_history_payload(
                snapshot,
                2,
                max_depth,
                options,
                last_modified,
                max_expanded_size,
            )
        }
    }
}

fn largest_fitting_history_payload(
    snapshot: &SessionSnapshot,
    min_depth: usize,
    max_depth: usize,
    options: &SessionOptions,
    last_modified: &str,
    max_expanded_size: u64,
) -> Result<Option<(usize, PayloadCandidate)>> {
    if min_depth > max_depth {
        return Ok(None);
    }

    let scan_started = Instant::now();
    let mut attempts = 0usize;
    for depth in (min_depth..=max_depth).rev() {
        attempts += 1;
        let candidate_started = Instant::now();
        let candidate = snapshot_with_history_depth(snapshot, depth);
        let payload = payload_candidate(&candidate, options, last_modified)?;
        debug!(
            "Prepared history-depth session payload candidate depth={} in {:?}: written={} bytes, raw={} bytes, compression={}",
            depth,
            candidate_started.elapsed(),
            payload.final_size(),
            payload.raw_size,
            payload.compressed
        );
        if payload.fits_limit(options, max_expanded_size) {
            info!(
                "History trim scan found fitting session payload at depth {} after {} candidate(s) in {:?}: written={} bytes, raw={} bytes, compression={}",
                depth,
                attempts,
                scan_started.elapsed(),
                payload.final_size(),
                payload.raw_size,
                payload.compressed
            );
            return Ok(Some((depth, payload)));
        }
    }

    warn!(
        "History trim scan found no fitting session payload after {} candidate(s) in {:?}",
        attempts,
        scan_started.elapsed()
    );
    Ok(None)
}

pub(super) fn log_payload_candidate(label: &str, payload: &PayloadCandidate, elapsed: Duration) {
    info!(
        "Prepared {} session payload candidate in {:?}: written={} bytes, raw={} bytes, compression={}",
        label,
        elapsed,
        payload.final_size(),
        payload.raw_size,
        payload.compressed
    );
}

pub(super) fn payload_candidate(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    last_modified: &str,
) -> Result<PayloadCandidate> {
    let raw_bytes = serialize_payload(snapshot, last_modified)?;
    let raw_size = raw_bytes.len();
    let compressed = should_compress_payload(raw_size, options);
    let bytes = if compressed {
        compress_bytes(&raw_bytes)?
    } else {
        raw_bytes
    };

    Ok(PayloadCandidate {
        bytes,
        raw_size,
        compressed,
    })
}

fn should_compress_payload(raw_size: usize, options: &SessionOptions) -> bool {
    match options.compression {
        CompressionMode::Off => false,
        CompressionMode::On => true,
        CompressionMode::Auto => raw_size as u64 >= options.auto_compress_threshold_bytes,
    }
}

fn serialize_payload(snapshot: &SessionSnapshot, last_modified: &str) -> Result<Vec<u8>> {
    let file_payload = SessionFile {
        version: CURRENT_VERSION,
        last_modified: last_modified.to_string(),
        active_board_id: Some(snapshot.active_board_id.clone()),
        active_mode: None,
        boards: snapshot
            .boards
            .iter()
            .map(|board| BoardFile {
                id: board.id.clone(),
                pages: board.pages.pages.clone(),
                active_page: board.pages.active,
            })
            .collect(),
        transparent: None,
        whiteboard: None,
        blackboard: None,
        transparent_pages: None,
        whiteboard_pages: None,
        blackboard_pages: None,
        transparent_active_page: None,
        whiteboard_active_page: None,
        blackboard_active_page: None,
        tool_state: snapshot.tool_state.clone(),
    };

    serde_json::to_vec_pretty(&file_payload).context("failed to serialise session payload")
}

#[allow(dead_code)]
pub(super) fn estimate_from_candidate(
    candidate: &PayloadCandidate,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> SnapshotPayloadEstimate {
    SnapshotPayloadEstimate {
        raw_size: candidate.raw_size,
        written_size: candidate.final_size(),
        max_file_size_bytes: options.max_file_size_bytes,
        compressed: candidate.compressed,
        limit_exceeded: candidate.limit_exceeded(options, max_expanded_size),
    }
}

pub(super) fn is_near_limit(written_size: u64, max_file_size_bytes: u64) -> bool {
    if written_size == 0 {
        return false;
    }
    let threshold = ((max_file_size_bytes as u128) * (NEAR_LIMIT_PERCENT as u128)).div_ceil(100);
    (written_size as u128) >= threshold
}

fn max_history_depth(snapshot: &SessionSnapshot) -> usize {
    snapshot
        .boards
        .iter()
        .flat_map(|board| board.pages.pages.iter())
        .map(|page| page.undo_stack_len().max(page.redo_stack_len()))
        .max()
        .unwrap_or(0)
}

fn snapshot_with_history_depth(snapshot: &SessionSnapshot, depth: usize) -> SessionSnapshot {
    let mut candidate = snapshot.clone();
    for board in &mut candidate.boards {
        for page in &mut board.pages.pages {
            page.clamp_history_depth(depth);
        }
    }
    candidate
}

pub(super) fn snapshot_without_history(snapshot: &SessionSnapshot) -> SessionSnapshot {
    let mut boards = Vec::with_capacity(snapshot.boards.len());
    for board in &snapshot.boards {
        let pages = BoardPagesSnapshot {
            pages: board
                .pages
                .pages
                .iter()
                .map(|page| page.clone_without_history())
                .collect(),
            active: board.pages.active,
        };
        if pages.has_persistable_data() {
            boards.push(BoardSnapshot {
                id: board.id.clone(),
                pages,
            });
        }
    }

    SessionSnapshot {
        active_board_id: snapshot.active_board_id.clone(),
        boards,
        tool_state: snapshot.tool_state.clone(),
    }
}
