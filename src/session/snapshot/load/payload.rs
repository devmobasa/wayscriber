use super::*;

pub(super) fn load_snapshot_opened_with_expanded_limit(
    session_path: &Path,
    options: &SessionOptions,
    file: fs::File,
    max_expanded_size: u64,
    max_encoded_size: Option<u64>,
    newer_version_action: NewerVersionAction,
) -> Result<Option<LoadedSnapshot>> {
    let file_bytes = read_session_bytes(
        session_path,
        options,
        file,
        max_expanded_size,
        max_encoded_size,
    )?;

    // Keep the original encoded payload only when decompression would consume
    // it. A too-new session must be preserved byte-for-byte, including its gzip
    // wrapper; ordinary uncompressed loads retain the existing zero-copy path.
    let encoded_session = is_gzip(&file_bytes).then(|| file_bytes.clone());
    let (decompressed, compressed) = maybe_decompress_with_limit(file_bytes, max_expanded_size)?;

    let mut value: Value =
        serde_json::from_slice(&decompressed).context("failed to parse session json")?;

    // The version gate runs on the raw document, before schema
    // deserialization: a newer release may have changed an existing field
    // incompatibly, and a deserialization failure would send the file down
    // the corrupt-backup path — whose backup slot rotation eventually
    // replaces, exactly the loss preservation exists to prevent.
    if let Some(version) = value.get("version").and_then(Value::as_u64)
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
    drop(encoded_session);

    let max_depth = max_history_depth(&value);
    if max_depth > MAX_COMPOUND_DEPTH {
        warn!(
            "Session history depth {} exceeds limit {}; dropping history",
            max_depth, MAX_COMPOUND_DEPTH
        );
        strip_history_fields(&mut value);
    }

    // Deserialization consumes the document, so a retry parses the retained
    // bytes again instead of holding a second copy of every string up front.
    let session_file: SessionFile = match serde_json::from_value(value) {
        Ok(file) => file,
        Err(err) => {
            warn!(
                "Failed to deserialize session ({}); retrying without history",
                err
            );
            let mut stripped: Value =
                serde_json::from_slice(&decompressed).context("failed to parse session json")?;
            strip_history_fields(&mut stripped);
            serde_json::from_value(stripped)
                .context("failed to parse session after stripping history")?
        }
    };
    drop(decompressed);

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
            appearance,
            id,
            pages,
            active_page,
        } in boards
        {
            board_snaps.push(BoardSnapshot {
                appearance,
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
                appearance: None,
                id: "transparent".to_string(),
                pages,
            });
        }
        if let Some(pages) =
            board_pages_from_file(whiteboard_pages, whiteboard_active_page, whiteboard)
        {
            board_snaps.push(BoardSnapshot {
                appearance: None,
                id: "whiteboard".to_string(),
                pages,
            });
        }
        if let Some(pages) =
            board_pages_from_file(blackboard_pages, blackboard_active_page, blackboard)
        {
            board_snaps.push(BoardSnapshot {
                appearance: None,
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

    snapshot
        .boards
        .retain(|board| board.appearance.is_none() || board.has_recoverable_user_data());

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

/// Reads the whole session file, but never more than a limit.
///
/// A caller-supplied `max_encoded_size` is the configured file-size check.
/// Otherwise the limit is the larger of the expanded-size cap and the
/// configured file size: uncompressed saves may use the whole configured size,
/// and a plain file expands to exactly its encoded size.
fn read_session_bytes(
    session_path: &Path,
    options: &SessionOptions,
    file: fs::File,
    max_expanded_size: u64,
    max_encoded_size: Option<u64>,
) -> Result<Vec<u8>> {
    let read_limit =
        max_encoded_size.unwrap_or_else(|| max_expanded_size.max(options.max_file_size_bytes));

    let mut file_bytes = Vec::new();
    file.take(read_limit.saturating_add(1))
        .read_to_end(&mut file_bytes)
        .context("failed to read session file")?;

    let read_size = file_bytes.len() as u64;
    if read_size <= read_limit {
        return Ok(file_bytes);
    }
    if max_encoded_size.is_some() {
        return Err(anyhow!(
            "session file {} is larger than configured limit of {} bytes",
            session_path.display(),
            read_limit
        ));
    }
    Err(ExpandedSessionTooLarge {
        expanded_size: read_size,
        max_expanded_size: read_limit,
    }
    .into())
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
