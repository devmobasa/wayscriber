use anyhow::{Context, Result, anyhow};
use flate2::{Compression, bufread::GzDecoder, write::GzEncoder};
use log::warn;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use super::types::ClearToolStateOutcome;
use crate::durable_io::{AtomicWriteOptions, OverwriteMode, PermissionPolicy, SymlinkPolicy};
use crate::session::options::SessionOptions;
use crate::session::primary::session_artifact_metadata_if_exists;

const MAX_EXPANDED_SESSION_BYTES: u64 = 128 * 1024 * 1024;

pub fn clear_tool_state(options: &SessionOptions) -> Result<ClearToolStateOutcome> {
    if options.is_named_file() {
        crate::session::validate_named_session_file_for_clear(&options.session_file_path())?;
    }

    let mut found_session = false;
    let mut cleared = false;
    let mut preserved_board_data = false;

    for edit_options in tool_state_edit_targets(options) {
        match clear_tool_state_for_target(&edit_options)? {
            ClearToolStateOutcome::NoSession => {}
            ClearToolStateOutcome::NoToolState => found_session = true,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: target_preserved_board_data,
            } => {
                found_session = true;
                cleared = true;
                preserved_board_data |= target_preserved_board_data;
            }
        }
    }

    if cleared {
        Ok(ClearToolStateOutcome::Cleared {
            preserved_board_data,
        })
    } else if found_session {
        Ok(ClearToolStateOutcome::NoToolState)
    } else {
        Ok(ClearToolStateOutcome::NoSession)
    }
}

fn clear_tool_state_for_target(options: &SessionOptions) -> Result<ClearToolStateOutcome> {
    let Some(mut candidate) = load_tool_state_edit_candidate(options)? else {
        return Ok(ClearToolStateOutcome::NoSession);
    };
    let session_path = options.session_file_path();
    let board_data = session_value_has_board_data(&candidate.value);

    let Some(object) = candidate.value.as_object_mut() else {
        return Err(anyhow!(
            "session file {} does not contain a JSON object",
            session_path.display()
        ));
    };

    if !object.contains_key("tool_state") {
        return if board_data {
            Ok(ClearToolStateOutcome::NoToolState)
        } else {
            Ok(ClearToolStateOutcome::NoSession)
        };
    }

    object.remove("tool_state");
    let raw =
        serde_json::to_vec_pretty(&candidate.value).context("failed to encode session json")?;
    let output = if candidate.compressed {
        compress_bytes(&raw)?
    } else {
        raw
    };

    crate::durable_io::write_atomic(
        &session_path,
        &output,
        AtomicWriteOptions {
            overwrite: OverwriteMode::Replace,
            permissions: PermissionPolicy::PreserveExistingOrMode(0o600),
            symlink: SymlinkPolicy::Reject,
            sync_file: true,
            sync_parent: true,
        },
    )
    .map_err(|err| anyhow!(err))
    .with_context(|| format!("failed to write session file {}", session_path.display()))?;

    Ok(ClearToolStateOutcome::Cleared {
        preserved_board_data: board_data,
    })
}

struct ToolStateEditCandidate {
    value: Value,
    compressed: bool,
}

impl ToolStateEditCandidate {
    fn has_board_data(&self) -> bool {
        session_value_has_board_data(&self.value)
    }

    fn has_session_data(&self) -> bool {
        self.value.get("tool_state").is_some() || self.has_board_data()
    }
}

fn load_tool_state_edit_candidate(
    options: &SessionOptions,
) -> Result<Option<ToolStateEditCandidate>> {
    let session_path = options.session_file_path();
    let session_metadata =
        session_artifact_metadata_if_exists(&session_path, options.is_named_file())?;
    let clear_marker_metadata = clear_marker_metadata(options);
    let backup_recovery_marker_metadata =
        marker_metadata_if_exists(&options.backup_recovery_marker_file_path(), options);
    let recovery_recoverable_marker_metadata =
        marker_metadata_if_exists(&options.recovery_recoverable_marker_file_path(), options);

    let recovery_path = options.recovery_file_path();
    let recovery_metadata =
        fallback_artifact_metadata_if_exists("session recovery", &recovery_path);
    let mut tried_recovery = false;
    if let Some(recovery_metadata) = recovery_metadata.as_ref()
        && should_prefer_recovery(recovery_metadata, session_metadata.as_ref())
        && !clear_marker_suppresses_artifact(recovery_metadata, clear_marker_metadata.as_ref())
    {
        tried_recovery = true;
        if let Some(candidate) =
            read_fallback_tool_state_edit_candidate("session recovery", &recovery_path)
            && candidate.has_session_data()
        {
            return Ok(Some(candidate));
        }
    }

    if let Some(session_metadata) = session_metadata.as_ref() {
        let primary_suppressed =
            clear_marker_suppresses_artifact(session_metadata, clear_marker_metadata.as_ref());
        if primary_suppressed {
            if let Some(candidate) =
                read_suppressed_primary_tool_state_edit_candidate(&session_path)
                && candidate.has_session_data()
                && !candidate.has_board_data()
            {
                return Ok(Some(candidate));
            }
        } else if let Some(candidate) = read_tool_state_edit_candidate(&session_path)?
            && candidate.has_session_data()
        {
            if candidate.has_board_data() {
                return Ok(Some(candidate));
            }
            if let Some(backup) = load_contentful_backup_candidate(
                options,
                Some(session_metadata),
                clear_marker_metadata.as_ref(),
                backup_recovery_marker_metadata.as_ref(),
            )? {
                return Ok(Some(backup));
            }
            if let Some(recovery) = load_contentful_recovery_candidate(
                options,
                clear_marker_metadata.as_ref(),
                recovery_recoverable_marker_metadata.as_ref(),
            )? {
                return Ok(Some(recovery));
            }
            return Ok(Some(candidate));
        }
    }

    if let Some(candidate) = load_contentful_backup_candidate(
        options,
        None,
        clear_marker_metadata.as_ref(),
        backup_recovery_marker_metadata.as_ref(),
    )? {
        return Ok(Some(candidate));
    }

    if !tried_recovery
        && let Some(recovery_metadata) = recovery_metadata.as_ref()
        && !clear_marker_suppresses_artifact(recovery_metadata, clear_marker_metadata.as_ref())
        && let Some(candidate) =
            read_fallback_tool_state_edit_candidate("session recovery", &recovery_path)
        && candidate.has_session_data()
    {
        return Ok(Some(candidate));
    }

    Ok(None)
}

fn load_contentful_backup_candidate(
    options: &SessionOptions,
    primary_metadata: Option<&fs::Metadata>,
    clear_marker_metadata: Option<&fs::Metadata>,
    backup_recovery_marker_metadata: Option<&fs::Metadata>,
) -> Result<Option<ToolStateEditCandidate>> {
    let backup_path = options.backup_file_path();
    let Some(backup_metadata) =
        fallback_artifact_metadata_if_exists("session backup", &backup_path)
    else {
        return Ok(None);
    };
    if clear_marker_suppresses_artifact(&backup_metadata, clear_marker_metadata) {
        return Ok(None);
    }
    if let Some(primary_metadata) = primary_metadata
        && backup_recovery_marker_metadata.is_none()
        && !backup_is_newer_than_primary(&backup_metadata, primary_metadata)
    {
        return Ok(None);
    }
    let Some(candidate) = read_fallback_tool_state_edit_candidate("session backup", &backup_path)
    else {
        return Ok(None);
    };
    Ok(candidate.has_board_data().then_some(candidate))
}

fn load_contentful_recovery_candidate(
    options: &SessionOptions,
    clear_marker_metadata: Option<&fs::Metadata>,
    recovery_recoverable_marker_metadata: Option<&fs::Metadata>,
) -> Result<Option<ToolStateEditCandidate>> {
    if recovery_recoverable_marker_metadata.is_none() {
        return Ok(None);
    }
    let recovery_path = options.recovery_file_path();
    let Some(recovery_metadata) =
        fallback_artifact_metadata_if_exists("session recovery", &recovery_path)
    else {
        return Ok(None);
    };
    if clear_marker_suppresses_artifact(&recovery_metadata, clear_marker_metadata) {
        return Ok(None);
    }
    let Some(candidate) =
        read_fallback_tool_state_edit_candidate("session recovery", &recovery_path)
    else {
        return Ok(None);
    };
    Ok(candidate.has_board_data().then_some(candidate))
}

fn read_tool_state_edit_candidate(path: &Path) -> Result<Option<ToolStateEditCandidate>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to read session file {}", path.display()));
        }
    };
    let (expanded, compressed) = maybe_decompress_with_limit(bytes)?;
    let value: Value = serde_json::from_slice(&expanded)
        .with_context(|| format!("failed to parse session json {}", path.display()))?;
    Ok(Some(ToolStateEditCandidate { value, compressed }))
}

fn read_suppressed_primary_tool_state_edit_candidate(
    path: &Path,
) -> Option<ToolStateEditCandidate> {
    match read_tool_state_edit_candidate(path) {
        Ok(candidate) => candidate,
        Err(err) => {
            warn!(
                "Preserving unreadable clear-marker-suppressed primary session {} while clearing saved tool state: {:#}",
                path.display(),
                err
            );
            None
        }
    }
}

fn read_fallback_tool_state_edit_candidate(
    label: &str,
    path: &Path,
) -> Option<ToolStateEditCandidate> {
    match read_tool_state_edit_candidate(path) {
        Ok(candidate) => candidate,
        Err(err) => {
            warn!(
                "Ignoring unreadable fallback {} {} while clearing saved tool state: {:#}",
                label,
                path.display(),
                err
            );
            None
        }
    }
}

fn fallback_artifact_metadata_if_exists(label: &str, path: &Path) -> Option<fs::Metadata> {
    match session_artifact_metadata_if_exists(path, false) {
        Ok(metadata) => metadata,
        Err(err) => {
            warn!(
                "Ignoring unreadable fallback {} {} while clearing saved tool state: {:#}",
                label,
                path.display(),
                err
            );
            None
        }
    }
}
fn clear_marker_metadata(options: &SessionOptions) -> Option<fs::Metadata> {
    session_artifact_metadata_if_exists(&options.clear_marker_file_path(), options.is_named_file())
        .ok()
        .flatten()
}

fn marker_metadata_if_exists(path: &Path, options: &SessionOptions) -> Option<fs::Metadata> {
    session_artifact_metadata_if_exists(path, options.is_named_file())
        .ok()
        .flatten()
}

fn clear_marker_suppresses_artifact(
    artifact_metadata: &fs::Metadata,
    clear_marker_metadata: Option<&fs::Metadata>,
) -> bool {
    let Some(clear_marker_metadata) = clear_marker_metadata else {
        return false;
    };
    match (
        artifact_metadata.modified(),
        clear_marker_metadata.modified(),
    ) {
        (Ok(artifact_modified), Ok(marker_modified)) => artifact_modified <= marker_modified,
        _ => true,
    }
}

fn should_prefer_recovery(
    recovery_metadata: &fs::Metadata,
    session_metadata: Option<&fs::Metadata>,
) -> bool {
    let Some(session_metadata) = session_metadata else {
        return true;
    };
    match (recovery_metadata.modified(), session_metadata.modified()) {
        (Ok(recovery_modified), Ok(session_modified)) => recovery_modified >= session_modified,
        _ => true,
    }
}

fn backup_is_newer_than_primary(backup: &fs::Metadata, primary: &fs::Metadata) -> bool {
    match (backup.modified(), primary.modified()) {
        (Ok(backup_modified), Ok(primary_modified)) => backup_modified > primary_modified,
        _ => false,
    }
}

fn tool_state_edit_targets(options: &SessionOptions) -> Vec<SessionOptions> {
    let mut identities = BTreeSet::new();
    if !options.is_named_file()
        && let Some(identity) = options.output_identity()
    {
        identities.insert(Some(identity.to_string()));
    } else {
        identities.insert(None);
    }

    if !options.is_named_file() && options.per_output && options.output_identity().is_none() {
        let prefix = options.file_prefix();
        if let Ok(entries) = fs::read_dir(&options.base_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if let Some(identity) = per_output_identity_from_snapshot_artifact(name, &prefix) {
                    identities.insert(identity);
                }
            }
        }
    }

    identities
        .into_iter()
        .map(|identity| {
            let mut target = options.clone();
            target.force_resume_persistence();
            if !target.is_named_file() {
                target.set_output_identity(identity.as_deref());
            }
            target
        })
        .collect()
}

fn per_output_identity_from_snapshot_artifact(name: &str, prefix: &str) -> Option<Option<String>> {
    let primary = name
        .strip_suffix(".bak")
        .or_else(|| name.strip_suffix(".recovery"))
        .unwrap_or(name);
    let rest = primary.strip_prefix(prefix)?;
    if rest == ".json" {
        return Some(None);
    }
    let identity = rest.strip_prefix('-')?.strip_suffix(".json")?;
    (!identity.is_empty()).then(|| Some(identity.to_string()))
}

fn maybe_decompress_with_limit(bytes: Vec<u8>) -> Result<(Vec<u8>, bool)> {
    if !is_gzip(&bytes) {
        return Ok((bytes, false));
    }

    let mut decoder = GzDecoder::new(&bytes[..]);
    let mut out = Vec::new();
    decoder
        .by_ref()
        .take(MAX_EXPANDED_SESSION_BYTES.saturating_add(1))
        .read_to_end(&mut out)
        .context("failed to decompress session file")?;
    if out.len() as u64 > MAX_EXPANDED_SESSION_BYTES {
        return Err(anyhow!(
            "session expands beyond the safety limit of {} bytes",
            MAX_EXPANDED_SESSION_BYTES
        ));
    }
    Ok((out, true))
}

fn is_gzip(bytes: &[u8]) -> bool {
    bytes.len() > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

fn compress_bytes(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .context("failed to compress session payload")?;
    encoder
        .finish()
        .context("failed to finalise compressed session payload")
}

fn session_value_has_board_data(value: &Value) -> bool {
    has_non_empty_boards(value)
        || frame_has_data(value.get("transparent"))
        || frame_has_data(value.get("whiteboard"))
        || frame_has_data(value.get("blackboard"))
        || pages_have_data(value.get("transparent_pages"))
        || pages_have_data(value.get("whiteboard_pages"))
        || pages_have_data(value.get("blackboard_pages"))
}

fn has_non_empty_boards(value: &Value) -> bool {
    value
        .get("boards")
        .and_then(Value::as_array)
        .is_some_and(|boards| {
            boards.iter().any(|board| {
                board
                    .get("pages")
                    .and_then(Value::as_array)
                    .is_some_and(|pages| pages.iter().any(|page| frame_has_data(Some(page))))
            })
        })
}

fn pages_have_data(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_array)
        .is_some_and(|pages| pages.iter().any(|page| frame_has_data(Some(page))))
}

fn frame_has_data(value: Option<&Value>) -> bool {
    let Some(value) = value else {
        return false;
    };
    value
        .get("shapes")
        .and_then(Value::as_array)
        .is_some_and(|shapes| !shapes.is_empty())
        || value.get("page_name").is_some_and(|name| !name.is_null())
        || tuple_is_non_origin(value.get("view_offset"))
        || value
            .get("undo_stack")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
        || value
            .get("redo_stack")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
}

fn tuple_is_non_origin(value: Option<&Value>) -> bool {
    let Some(items) = value.and_then(Value::as_array) else {
        return false;
    };
    match items.as_slice() {
        [x, y] => x.as_i64().unwrap_or_default() != 0 || y.as_i64().unwrap_or_default() != 0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{ClearToolStateOutcome, SessionOptions};
    use serde_json::{Value, json};
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    fn session_payload(include_tool_state: bool, include_board_data: bool) -> Value {
        let mut payload = json!({
            "version": 6,
            "last_modified": "2026-07-09T00:00:00Z",
            "active_board_id": "transparent",
            "boards": [{
                "id": "transparent",
                "pages": [frame_payload(include_board_data)],
                "active_page": 0
            }]
        });
        if include_tool_state {
            payload["tool_state"] = json!({
                "current_color": { "r": 0.0, "g": 1.0, "b": 0.0, "a": 1.0 },
                "current_thickness": 3.0,
                "current_font_size": 24.0,
                "text_background_enabled": false,
                "arrow_length": 20.0,
                "arrow_angle": 30.0,
                "board_previous_color": null,
                "show_status_bar": true
            });
        }
        payload
    }

    fn frame_payload(include_board_data: bool) -> Value {
        if include_board_data {
            json!({
                "shapes": [{
                    "id": 1,
                    "shape": {
                        "Line": {
                            "x1": 0,
                            "y1": 0,
                            "x2": 10,
                            "y2": 10,
                            "color": { "r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0 },
                            "thick": 2.0
                        }
                    },
                    "created_at": 0,
                    "locked": false
                }]
            })
        } else {
            json!({ "shapes": [] })
        }
    }

    fn write_json(path: &Path, value: &Value) {
        std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    }

    fn write_compressed_json(path: &Path, value: &Value) {
        let raw = serde_json::to_vec_pretty(value).unwrap();
        std::fs::write(path, compress_bytes(&raw).unwrap()).unwrap();
    }

    fn read_json(path: &Path) -> Value {
        let bytes = std::fs::read(path).unwrap();
        let (expanded, _) = maybe_decompress_with_limit(bytes).unwrap();
        serde_json::from_slice(&expanded).unwrap()
    }

    fn set_modified(path: &Path, modified: SystemTime) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .expect("open file for timestamp update")
            .set_modified(modified)
            .expect("set file modified timestamp");
    }

    #[test]
    fn clear_tool_state_preserves_board_data() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-clear-tool");
        options.persist_transparent = true;
        options.restore_tool_state = true;
        let path = options.session_file_path();
        write_json(&path, &session_payload(true, true));

        let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");

        assert_eq!(
            outcome,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: true
            }
        );
        let saved = read_json(&path);
        assert!(saved.get("tool_state").is_none());
        assert!(
            saved["boards"][0]["pages"][0]["shapes"]
                .as_array()
                .is_some_and(|shapes| shapes.len() == 1)
        );
    }

    #[test]
    fn clear_tool_state_rewrites_compressed_session() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-compressed");
        options.persist_transparent = true;
        options.restore_tool_state = true;
        let path = options.session_file_path();
        write_compressed_json(&path, &session_payload(true, true));

        let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");

        assert_eq!(
            outcome,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: true
            }
        );
        let bytes = std::fs::read(&path).unwrap();
        assert!(is_gzip(&bytes));
        let saved = read_json(&path);
        assert!(saved.get("tool_state").is_none());
    }

    #[test]
    fn clear_tool_state_skips_corrupt_newer_recovery_and_edits_primary() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-recovery-bad");
        options.persist_transparent = true;
        options.restore_tool_state = true;
        let path = options.session_file_path();

        write_json(&path, &session_payload(true, true));
        std::fs::write(options.recovery_file_path(), b"{").unwrap();

        let outcome = clear_tool_state(&options).expect("corrupt recovery should be ignored");

        assert_eq!(
            outcome,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: true
            }
        );
        let saved = read_json(&path);
        assert!(saved.get("tool_state").is_none());
        assert!(
            saved["boards"][0]["pages"][0]["shapes"]
                .as_array()
                .is_some_and(|shapes| shapes.len() == 1)
        );
    }

    #[test]
    fn clear_tool_state_skips_corrupt_backup_candidate_and_edits_primary() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-backup-bad");
        options.persist_transparent = true;
        options.restore_tool_state = true;
        let path = options.session_file_path();

        write_json(&path, &session_payload(true, false));
        std::fs::write(options.backup_recovery_marker_file_path(), b"recoverable").unwrap();
        std::fs::write(options.backup_file_path(), b"{").unwrap();

        let outcome = clear_tool_state(&options).expect("corrupt backup should be ignored");

        assert_eq!(
            outcome,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: false
            }
        );
        let saved = read_json(&path);
        assert!(saved.get("tool_state").is_none());
        assert!(
            saved["boards"][0]["pages"][0]["shapes"]
                .as_array()
                .is_some_and(Vec::is_empty)
        );
    }

    #[test]
    fn clear_tool_state_restores_backup_when_suppressed_primary_is_corrupt() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-primary-bad");
        options.persist_transparent = true;
        options.restore_tool_state = true;
        let primary_path = options.session_file_path();
        let backup_path = options.backup_file_path();
        let clear_marker_path = options.clear_marker_file_path();

        write_json(&backup_path, &session_payload(true, true));
        std::fs::write(&primary_path, b"{not valid json").unwrap();
        std::fs::write(&clear_marker_path, b"cleared").unwrap();
        set_modified(
            &primary_path,
            SystemTime::UNIX_EPOCH + Duration::from_secs(10),
        );
        set_modified(
            &clear_marker_path,
            SystemTime::UNIX_EPOCH + Duration::from_secs(20),
        );
        set_modified(
            &backup_path,
            SystemTime::UNIX_EPOCH + Duration::from_secs(30),
        );

        let outcome = clear_tool_state(&options)
            .expect("suppressed corrupt primary should fall back to backup");

        assert_eq!(
            outcome,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: true
            }
        );
        let saved = read_json(&primary_path);
        assert!(saved.get("tool_state").is_none());
        assert!(
            saved["boards"][0]["pages"][0]["shapes"]
                .as_array()
                .is_some_and(|shapes| shapes.len() == 1),
            "recoverable backup board data should replace the suppressed corrupt primary"
        );
    }

    #[test]
    fn clear_named_tool_state_recovers_from_backup_when_primary_is_missing() {
        let temp = crate::test_temp::tempdir().unwrap();
        let session = temp.path().join("lecture-backup.wayscriber-session");
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "display");
        options.set_named_file_target(session);
        options.persist_transparent = true;
        options.restore_tool_state = true;

        write_json(&options.backup_file_path(), &session_payload(true, true));

        let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");

        assert_eq!(
            outcome,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: true
            }
        );
        let saved = read_json(&options.session_file_path());
        assert!(saved.get("tool_state").is_none());
        assert!(
            saved["boards"][0]["pages"][0]["shapes"]
                .as_array()
                .is_some_and(|shapes| shapes.len() == 1)
        );
    }

    #[test]
    fn clear_per_output_tool_state_recovers_from_recovery_when_primary_is_missing() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-1");
        options.persist_transparent = true;
        options.restore_tool_state = true;
        options.per_output = true;

        let mut output_options = options.clone();
        output_options.set_output_identity(Some("DP-1"));
        write_json(
            &output_options.recovery_file_path(),
            &session_payload(true, true),
        );

        let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");

        assert_eq!(
            outcome,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: true
            }
        );
        let saved = read_json(&output_options.session_file_path());
        assert!(saved.get("tool_state").is_none());
        assert!(
            saved["boards"][0]["pages"][0]["shapes"]
                .as_array()
                .is_some_and(|shapes| shapes.len() == 1)
        );
    }

    #[test]
    fn clear_tool_state_preserves_recoverable_backup_over_tool_state_only_primary() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-backup");
        options.persist_transparent = true;
        options.restore_tool_state = true;

        write_json(&options.backup_file_path(), &session_payload(true, true));
        std::fs::write(options.backup_recovery_marker_file_path(), b"recoverable").unwrap();
        write_json(&options.session_file_path(), &session_payload(true, false));

        let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");

        assert_eq!(
            outcome,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: true
            }
        );
        let saved = read_json(&options.session_file_path());
        assert!(saved.get("tool_state").is_none());
        assert!(
            saved["boards"][0]["pages"][0]["shapes"]
                .as_array()
                .is_some_and(|shapes| shapes.len() == 1),
            "recoverable backup board data should be restored into the primary"
        );
    }

    #[test]
    fn clear_tool_state_preserves_recoverable_recovery_over_tool_state_only_primary() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-recovery");
        options.persist_transparent = true;
        options.restore_tool_state = true;

        write_json(&options.recovery_file_path(), &session_payload(true, true));
        std::fs::write(
            options.recovery_recoverable_marker_file_path(),
            b"recoverable",
        )
        .unwrap();
        write_json(&options.session_file_path(), &session_payload(true, false));

        let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");

        assert_eq!(
            outcome,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: true
            }
        );
        let saved = read_json(&options.session_file_path());
        assert!(saved.get("tool_state").is_none());
        assert!(
            saved["boards"][0]["pages"][0]["shapes"]
                .as_array()
                .is_some_and(|shapes| shapes.len() == 1),
            "recoverable recovery board data should be restored into the primary"
        );
    }

    #[test]
    fn clear_tool_state_accepts_clear_marker_suppressed_tool_state_only_primary() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-cleared");
        options.persist_transparent = true;
        options.restore_tool_state = true;

        write_json(&options.session_file_path(), &session_payload(true, false));
        std::fs::write(options.clear_marker_file_path(), b"cleared").unwrap();

        let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");

        assert_eq!(
            outcome,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: false
            }
        );
        let saved = read_json(&options.session_file_path());
        assert!(saved.get("tool_state").is_none());
        assert!(
            saved["boards"][0]["pages"][0]["shapes"]
                .as_array()
                .is_some_and(Vec::is_empty),
            "contentless clear-boundary primary should be edited in place"
        );
    }

    #[test]
    fn clear_tool_state_without_output_identity_clears_all_per_output_variants() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-1");
        options.persist_transparent = true;
        options.restore_tool_state = true;
        options.per_output = true;

        let mut first_output = options.clone();
        first_output.set_output_identity(Some("DP-1"));
        write_json(
            &first_output.session_file_path(),
            &session_payload(true, true),
        );

        let mut second_output = options.clone();
        second_output.set_output_identity(Some("HDMI-A-1"));
        write_json(
            &second_output.session_file_path(),
            &session_payload(true, true),
        );

        let mut neighbor = SessionOptions::new(temp.path().to_path_buf(), "display-10");
        neighbor.persist_transparent = true;
        neighbor.restore_tool_state = true;
        neighbor.set_output_identity(Some("DP-1"));
        write_json(&neighbor.session_file_path(), &session_payload(true, true));

        let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");

        assert_eq!(
            outcome,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: true
            }
        );
        assert!(
            read_json(&first_output.session_file_path())
                .get("tool_state")
                .is_none()
        );
        assert!(
            read_json(&second_output.session_file_path())
                .get("tool_state")
                .is_none()
        );
        assert!(
            read_json(&neighbor.session_file_path())
                .get("tool_state")
                .is_some(),
            "neighboring display prefix should be preserved"
        );
    }

    #[test]
    fn clear_tool_state_missing_session_is_nonfatal() {
        let temp = crate::test_temp::tempdir().unwrap();
        let options = SessionOptions::new(temp.path().to_path_buf(), "display-missing");

        let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");

        assert_eq!(outcome, ClearToolStateOutcome::NoSession);
    }

    #[test]
    fn clear_tool_state_without_tool_state_is_nonfatal_and_preserves_boards() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "display-no-tool");
        options.persist_transparent = true;
        options.restore_tool_state = true;
        let path = options.session_file_path();
        write_json(&path, &session_payload(false, true));

        let outcome = clear_tool_state(&options).expect("clear_tool_state should succeed");

        assert_eq!(outcome, ClearToolStateOutcome::NoToolState);
        let saved = read_json(&path);
        assert!(saved.get("tool_state").is_none());
        assert!(
            saved["boards"][0]["pages"][0]["shapes"]
                .as_array()
                .is_some_and(|shapes| shapes.len() == 1)
        );
    }

    #[test]
    fn clear_named_tool_state_targets_only_selected_file() {
        let temp = crate::test_temp::tempdir().unwrap();
        let selected = temp.path().join("lecture-04.wayscriber-session");
        let sibling = temp.path().join("lecture-05.wayscriber-session");

        let mut selected_options = SessionOptions::new(temp.path().to_path_buf(), "display");
        selected_options.set_named_file_target(selected);
        selected_options.persist_transparent = true;
        selected_options.restore_tool_state = true;

        let mut sibling_options = SessionOptions::new(temp.path().to_path_buf(), "display");
        sibling_options.set_named_file_target(sibling);
        sibling_options.persist_transparent = true;
        sibling_options.restore_tool_state = true;

        write_json(
            &selected_options.session_file_path(),
            &session_payload(true, true),
        );
        write_json(
            &sibling_options.session_file_path(),
            &session_payload(true, true),
        );

        let outcome = clear_tool_state(&selected_options).expect("clear_tool_state should succeed");

        assert_eq!(
            outcome,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: true
            }
        );
        assert!(
            read_json(&selected_options.session_file_path())
                .get("tool_state")
                .is_none()
        );
        assert!(
            read_json(&sibling_options.session_file_path())
                .get("tool_state")
                .is_some(),
            "sibling tool state should be preserved"
        );
    }
}
