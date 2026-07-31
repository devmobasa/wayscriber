use super::*;

pub(super) fn load_snapshot_opened_with_expanded_limit(
    session_path: &Path,
    options: &SessionOptions,
    mut file: fs::File,
    max_expanded_size: u64,
    max_encoded_size: Option<u64>,
    newer_version_action: NewerVersionAction,
) -> Result<Option<LoadedSnapshot>> {
    let mut file_bytes = Vec::new();
    if let Some(max_encoded_size) = max_encoded_size {
        file.by_ref()
            .take(max_encoded_size.saturating_add(1))
            .read_to_end(&mut file_bytes)
            .context("failed to read session file")?;
        if file_bytes.len() as u64 > max_encoded_size {
            return Err(anyhow!(
                "session file {} is larger than configured limit of {} bytes",
                session_path.display(),
                max_encoded_size
            ));
        }
    } else {
        file.read_to_end(&mut file_bytes)
            .context("failed to read session file")?;
    }

    // Keep the original encoded payload only when decompression would consume
    // it. A too-new session must be preserved byte-for-byte, including its gzip
    // wrapper; ordinary uncompressed loads retain the existing zero-copy path.
    let encoded_session = is_gzip(&file_bytes).then(|| file_bytes.clone());
    let (decompressed, compressed) = maybe_decompress_with_limit(file_bytes, max_expanded_size)?;

    let original_value: Value =
        serde_json::from_slice(&decompressed).context("failed to parse session json")?;

    // The version gate runs on the raw document, before schema
    // deserialization: a newer release may have changed an existing field
    // incompatibly, and a deserialization failure would send the file down
    // the corrupt-backup path — whose backup slot rotation eventually
    // replaces, exactly the loss preservation exists to prevent.
    if let Some(version) = original_value.get("version").and_then(Value::as_u64)
        && version > u64::from(CURRENT_VERSION)
    {
        warn!(
            "Session file {} was written by a newer wayscriber (version {}, supported {}); continuing with an empty session",
            session_path.display(),
            version,
            CURRENT_VERSION
        );
        if matches!(newer_version_action, NewerVersionAction::Preserve) {
            let preservation_bytes = encoded_session.as_deref().unwrap_or(&decompressed);
            let preserved_path =
                preserve_newer_version_session(session_path, preservation_bytes, version).map_err(
                    |err| NewerVersionPreservationFailed {
                        path: session_path.to_path_buf(),
                        details: format!("{err:#}"),
                    },
                )?;
            warn!(
                "Preserved the newer-version session at {}; a newer wayscriber can restore it",
                preserved_path.display()
            );
        }
        return Ok(None);
    }

    let max_depth = max_history_depth(&original_value);
    let mut working_value = original_value.clone();
    if max_depth > MAX_COMPOUND_DEPTH {
        warn!(
            "Session history depth {} exceeds limit {}; dropping history",
            max_depth, MAX_COMPOUND_DEPTH
        );
        strip_history_fields(&mut working_value);
    }

    let session_file: SessionFile = match serde_json::from_value(working_value.clone()) {
        Ok(file) => file,
        Err(err) => {
            warn!(
                "Failed to deserialize session ({}); retrying without history",
                err
            );
            let mut stripped = original_value.clone();
            strip_history_fields(&mut stripped);
            serde_json::from_value(stripped)
                .context("failed to parse session after stripping history")?
        }
    };

    let SessionFile {
        active_board_id,
        active_mode,
        boards,
        transparent,
        whiteboard,
        blackboard,
        transparent_pages,
        whiteboard_pages,
        blackboard_pages,
        transparent_active_page,
        whiteboard_active_page,
        blackboard_active_page,
        tool_state,
        ..
    } = session_file;

    let mut snapshot = if !boards.is_empty() || active_board_id.is_some() {
        let mut board_snaps = Vec::new();
        for BoardFile {
            id,
            pages,
            active_page,
        } in boards
        {
            board_snaps.push(BoardSnapshot {
                id,
                pages: normalized_board_pages_snapshot(pages, Some(active_page)),
            });
        }
        let active_board_id = resolved_active_board_id(active_board_id, &board_snaps);
        SessionSnapshot {
            active_board_id,
            boards: board_snaps,
            tool_state,
        }
    } else {
        let mut board_snaps = Vec::new();
        if let Some(pages) =
            board_pages_from_file(transparent_pages, transparent_active_page, transparent)
        {
            board_snaps.push(BoardSnapshot {
                id: "transparent".to_string(),
                pages,
            });
        }
        if let Some(pages) =
            board_pages_from_file(whiteboard_pages, whiteboard_active_page, whiteboard)
        {
            board_snaps.push(BoardSnapshot {
                id: "whiteboard".to_string(),
                pages,
            });
        }
        if let Some(pages) =
            board_pages_from_file(blackboard_pages, blackboard_active_page, blackboard)
        {
            board_snaps.push(BoardSnapshot {
                id: "blackboard".to_string(),
                pages,
            });
        }
        let active_board_id =
            resolved_active_board_id(active_mode.map(|mode| mode.to_lowercase()), &board_snaps);
        SessionSnapshot {
            active_board_id,
            boards: board_snaps,
            tool_state,
        }
    };

    enforce_shape_limits(&mut snapshot, options.max_shapes_per_frame);
    let disk_history_limit = if options.persist_history {
        options.max_persisted_undo_depth
    } else {
        Some(0)
    };
    for board in &mut snapshot.boards {
        apply_history_policies(&mut board.pages, &board.id, disk_history_limit);
    }

    if snapshot.is_empty() && snapshot.tool_state.is_none() {
        debug!(
            "Loaded session file at {} but it contained no data",
            session_path.display()
        );
        return Ok(None);
    }

    Ok(Some(LoadedSnapshot {
        snapshot,
        compressed,
        version: session_file.version,
    }))
}

fn board_pages_from_file(
    pages: Option<Vec<Frame>>,
    active: Option<usize>,
    legacy: Option<Frame>,
) -> Option<BoardPagesSnapshot> {
    if let Some(pages) = pages {
        return Some(normalized_board_pages_snapshot(pages, active));
    }
    legacy.map(|frame| BoardPagesSnapshot {
        pages: vec![frame],
        active: 0,
    })
}

fn normalized_board_pages_snapshot(
    mut pages: Vec<Frame>,
    active: Option<usize>,
) -> BoardPagesSnapshot {
    if pages.is_empty() {
        pages.push(Frame::new());
    }
    let active = active.unwrap_or(0).min(pages.len().saturating_sub(1));
    BoardPagesSnapshot { pages, active }
}

fn resolved_active_board_id(requested: Option<String>, boards: &[BoardSnapshot]) -> String {
    let Some(fallback_id) = boards.first().map(|board| board.id.clone()) else {
        return "transparent".to_string();
    };

    let requested = requested.unwrap_or_else(|| fallback_id.clone());
    if boards.iter().any(|board| board.id == requested) {
        requested
    } else {
        warn!(
            "Session active board '{}' missing from restored boards; falling back to '{}'",
            requested, fallback_id
        );
        fallback_id
    }
}
