//! Size estimates for a save that has not happened yet.
use super::*;

pub(crate) fn estimate_snapshot_save(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
) -> Result<SnapshotSaveEstimate> {
    estimate_snapshot_save_with_expanded_limit(
        snapshot,
        options,
        DEFAULT_MAX_EXPANDED_SESSION_BYTES,
    )
}

fn estimate_snapshot_save_with_expanded_limit(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<SnapshotSaveEstimate> {
    let last_modified = now_rfc3339();
    let full = payload_candidate(snapshot, options, PayloadStamp::widest(&last_modified))?;
    let visible_only = snapshot_without_history(snapshot);
    let visible_without_history =
        payload_candidate(&visible_only, options, PayloadStamp::widest(&last_modified))?;

    Ok(SnapshotSaveEstimate {
        full: estimate_from_candidate(&full, options, max_expanded_size),
        visible_without_history: estimate_from_candidate(
            &visible_without_history,
            options,
            max_expanded_size,
        ),
    })
}

pub(crate) fn estimate_snapshot_payload(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
) -> Result<SnapshotPayloadEstimate> {
    estimate_snapshot_payload_with_expanded_limit(
        snapshot,
        options,
        DEFAULT_MAX_EXPANDED_SESSION_BYTES,
    )
}

pub(crate) fn estimate_snapshot_without_history_payload(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
) -> Result<SnapshotPayloadEstimate> {
    let visible_only = snapshot_without_history(snapshot);
    estimate_snapshot_payload(&visible_only, options)
}

fn estimate_snapshot_payload_with_expanded_limit(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<SnapshotPayloadEstimate> {
    let last_modified = now_rfc3339();
    let candidate = payload_candidate(snapshot, options, PayloadStamp::widest(&last_modified))?;
    Ok(estimate_from_candidate(
        &candidate,
        options,
        max_expanded_size,
    ))
}
