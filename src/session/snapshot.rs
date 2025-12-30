use super::options::{CompressionMode, SessionOptions};
use crate::draw::frame::{MAX_COMPOUND_DEPTH, ShapeId};
use crate::draw::{BoardPages, Color, EraserKind, Frame};
use crate::input::{
    EraserMode, InputState, Tool,
    board_mode::BoardMode,
    state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS},
};
use crate::session::lock::{lock_exclusive, lock_shared, unlock};
use crate::time_utils::now_rfc3339;
use anyhow::{Context, Result};
use flate2::{Compression, bufread::GzDecoder, write::GzEncoder};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

const CURRENT_VERSION: u32 = 4;

/// Captured state suitable for serialisation or restoration.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub active_mode: BoardMode,
    pub transparent: Option<BoardPagesSnapshot>,
    pub whiteboard: Option<BoardPagesSnapshot>,
    pub blackboard: Option<BoardPagesSnapshot>,
    pub tool_state: Option<ToolStateSnapshot>,
}

#[derive(Debug, Clone)]
pub struct BoardPagesSnapshot {
    pub pages: Vec<Frame>,
    pub active: usize,
}

impl BoardPagesSnapshot {
    fn has_persistable_data(&self) -> bool {
        if self.pages.len() > 1 || self.active > 0 {
            return true;
        }
        self.pages.iter().any(|page| page.has_persistable_data())
    }
}

impl SessionSnapshot {
    fn is_empty(&self) -> bool {
        let empty_pages = |pages: &Option<BoardPagesSnapshot>| {
            pages
                .as_ref()
                .is_none_or(|data| !data.has_persistable_data())
        };
        empty_pages(&self.transparent)
            && empty_pages(&self.whiteboard)
            && empty_pages(&self.blackboard)
    }
}

/// Subset of [`InputState`] we persist to disk to restore tool context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStateSnapshot {
    pub current_color: Color,
    pub current_thickness: f64,
    #[serde(default = "default_eraser_size_for_snapshot")]
    pub eraser_size: f64,
    #[serde(default = "default_eraser_kind_for_snapshot")]
    pub eraser_kind: EraserKind,
    #[serde(default = "default_eraser_mode_for_snapshot")]
    pub eraser_mode: EraserMode,
    #[serde(default)]
    pub marker_opacity: Option<f64>,
    #[serde(default)]
    pub fill_enabled: Option<bool>,
    #[serde(default)]
    pub tool_override: Option<Tool>,
    pub current_font_size: f64,
    pub text_background_enabled: bool,
    pub arrow_length: f64,
    pub arrow_angle: f64,
    #[serde(default)]
    pub arrow_head_at_end: Option<bool>,
    pub board_previous_color: Option<Color>,
    pub show_status_bar: bool,
}

impl ToolStateSnapshot {
    fn from_input_state(input: &InputState) -> Self {
        Self {
            current_color: input.current_color,
            current_thickness: input.current_thickness,
            eraser_size: input.eraser_size,
            eraser_kind: input.eraser_kind,
            eraser_mode: input.eraser_mode,
            marker_opacity: Some(input.marker_opacity),
            fill_enabled: Some(input.fill_enabled),
            tool_override: input.tool_override(),
            current_font_size: input.current_font_size,
            text_background_enabled: input.text_background_enabled,
            arrow_length: input.arrow_length,
            arrow_angle: input.arrow_angle,
            arrow_head_at_end: Some(input.arrow_head_at_end),
            board_previous_color: input.board_previous_color,
            show_status_bar: input.show_status_bar,
        }
    }
}

fn default_eraser_size_for_snapshot() -> f64 {
    12.0
}

fn default_eraser_kind_for_snapshot() -> EraserKind {
    EraserKind::Circle
}

fn default_eraser_mode_for_snapshot() -> EraserMode {
    EraserMode::Brush
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionFile {
    #[serde(default = "default_file_version")]
    version: u32,
    last_modified: String,
    active_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transparent: Option<Frame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    whiteboard: Option<Frame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blackboard: Option<Frame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transparent_pages: Option<Vec<Frame>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    whiteboard_pages: Option<Vec<Frame>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blackboard_pages: Option<Vec<Frame>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transparent_active_page: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    whiteboard_active_page: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blackboard_active_page: Option<usize>,
    #[serde(default)]
    tool_state: Option<ToolStateSnapshot>,
}

pub struct LoadedSnapshot {
    pub snapshot: SessionSnapshot,
    pub compressed: bool,
    pub version: u32,
}

/// Capture a snapshot from the current input state if persistence is enabled.
pub fn snapshot_from_input(
    input: &InputState,
    options: &SessionOptions,
) -> Option<SessionSnapshot> {
    if !options.any_enabled() && !options.restore_tool_state && !options.persist_history {
        return None;
    }

    let mut snapshot = SessionSnapshot {
        active_mode: input.board_mode(),
        transparent: None,
        whiteboard: None,
        blackboard: None,
        tool_state: None,
    };

    let history_limit = options.effective_history_limit(input.undo_stack_limit);

    let capture_pages = |mode: BoardMode| -> Option<BoardPagesSnapshot> {
        let pages = input.canvas_set.pages(mode)?;
        let mut cloned_pages: Vec<Frame> = pages.pages().to_vec();
        for page in &mut cloned_pages {
            if history_limit == 0 {
                page.clamp_history_depth(0);
            } else if history_limit < usize::MAX {
                page.clamp_history_depth(history_limit);
            }
        }
        let snapshot = BoardPagesSnapshot {
            pages: cloned_pages,
            active: pages.active_index(),
        };
        snapshot.has_persistable_data().then_some(snapshot)
    };

    if options.persist_transparent {
        snapshot.transparent = capture_pages(BoardMode::Transparent);
    }

    if options.persist_whiteboard {
        snapshot.whiteboard = capture_pages(BoardMode::Whiteboard);
    }

    if options.persist_blackboard {
        snapshot.blackboard = capture_pages(BoardMode::Blackboard);
    }

    if options.restore_tool_state {
        snapshot.tool_state = Some(ToolStateSnapshot::from_input_state(input));
    }

    if snapshot.is_empty() && snapshot.tool_state.is_none() {
        None
    } else {
        Some(snapshot)
    }
}

/// Persist the provided snapshot to disk according to the configured options.
pub fn save_snapshot(snapshot: &SessionSnapshot, options: &SessionOptions) -> Result<()> {
    if !options.any_enabled() && snapshot.tool_state.is_none() {
        debug!("Session persistence disabled for all boards; skipping save");
        return Ok(());
    }

    fs::create_dir_all(&options.base_dir).with_context(|| {
        format!(
            "failed to create session directory {}",
            options.base_dir.display()
        )
    })?;

    let lock_path = options.lock_file_path();
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open session lock file {}", lock_path.display()))?;
    lock_exclusive(&lock_file)
        .with_context(|| format!("failed to lock session file {}", lock_path.display()))?;

    let result = save_snapshot_inner(snapshot, options);

    if let Err(err) = unlock(&lock_file) {
        warn!(
            "failed to unlock session file {}: {}",
            lock_path.display(),
            err
        );
    }

    result
}

fn save_snapshot_inner(snapshot: &SessionSnapshot, options: &SessionOptions) -> Result<()> {
    let session_path = options.session_file_path();
    let backup_path = options.backup_file_path();

    if snapshot.is_empty() && snapshot.tool_state.is_none() {
        if session_path.exists() {
            debug!(
                "Removing session file {} because snapshot is empty",
                session_path.display()
            );
            fs::remove_file(&session_path).with_context(|| {
                format!(
                    "failed to remove empty session file {}",
                    session_path.display()
                )
            })?;
        }
        return Ok(());
    }

    let transparent = snapshot.transparent.clone();
    let whiteboard = snapshot.whiteboard.clone();
    let blackboard = snapshot.blackboard.clone();

    let file_payload = SessionFile {
        version: CURRENT_VERSION,
        last_modified: now_rfc3339(),
        active_mode: board_mode_to_str(snapshot.active_mode).to_string(),
        transparent: None,
        whiteboard: None,
        blackboard: None,
        transparent_pages: transparent.as_ref().map(|pages| pages.pages.clone()),
        whiteboard_pages: whiteboard.as_ref().map(|pages| pages.pages.clone()),
        blackboard_pages: blackboard.as_ref().map(|pages| pages.pages.clone()),
        transparent_active_page: transparent.as_ref().map(|pages| pages.active),
        whiteboard_active_page: whiteboard.as_ref().map(|pages| pages.active),
        blackboard_active_page: blackboard.as_ref().map(|pages| pages.active),
        tool_state: snapshot.tool_state.clone(),
    };

    let mut json_bytes =
        serde_json::to_vec_pretty(&file_payload).context("failed to serialise session payload")?;

    if json_bytes.len() as u64 > options.max_file_size_bytes {
        warn!(
            "Session data size {} bytes exceeds the configured limit of {} bytes; skipping save",
            json_bytes.len(),
            options.max_file_size_bytes
        );
        return Ok(());
    }

    let should_compress = match options.compression {
        CompressionMode::Off => false,
        CompressionMode::On => true,
        CompressionMode::Auto => (json_bytes.len() as u64) >= options.auto_compress_threshold_bytes,
    };

    if should_compress {
        json_bytes = compress_bytes(&json_bytes)?;
    }

    let tmp_path = temp_path(&session_path)?;
    {
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .with_context(|| {
                format!(
                    "failed to open temporary session file {}",
                    tmp_path.display()
                )
            })?;
        tmp_file
            .write_all(&json_bytes)
            .context("failed to write session payload")?;
        tmp_file
            .sync_all()
            .context("failed to sync temporary session file")?;
    }

    if session_path.exists() {
        if options.backup_retention > 0 {
            if backup_path.exists() {
                fs::remove_file(&backup_path).ok();
            }
            fs::rename(&session_path, &backup_path).with_context(|| {
                format!(
                    "failed to rotate previous session file {} -> {}",
                    session_path.display(),
                    backup_path.display()
                )
            })?;
        } else {
            fs::remove_file(&session_path).ok();
        }
    }

    fs::rename(&tmp_path, &session_path).with_context(|| {
        format!(
            "failed to move temporary session file {} -> {}",
            tmp_path.display(),
            session_path.display()
        )
    })?;

    info!(
        "Session saved to {} ({} bytes, compression={})",
        session_path.display(),
        json_bytes.len(),
        should_compress
    );

    Ok(())
}

/// Attempt to load a previously saved session.
pub fn load_snapshot(options: &SessionOptions) -> Result<Option<SessionSnapshot>> {
    if !options.any_enabled() && !options.restore_tool_state {
        info!(
            "Session load skipped: persistence disabled (base_dir={}, file={})",
            options.base_dir.display(),
            options.session_file_path().display()
        );
        return Ok(None);
    }

    let session_path = options.session_file_path();
    if !session_path.exists() {
        info!(
            "Session file not found at {}; skipping load",
            session_path.display()
        );
        return Ok(None);
    }

    let metadata = fs::metadata(&session_path)
        .with_context(|| format!("failed to stat session file {}", session_path.display()))?;
    info!(
        "Session file present at {} ({} bytes, per_output={}, output_identity={:?})",
        session_path.display(),
        metadata.len(),
        options.per_output,
        options.output_identity()
    );
    if metadata.len() > options.max_file_size_bytes {
        warn!(
            "Session file {} is {} bytes which exceeds the configured limit ({} bytes); refusing to load",
            session_path.display(),
            metadata.len(),
            options.max_file_size_bytes
        );
        return Ok(None);
    }

    let lock_path = options.lock_file_path();
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open session lock file {}", lock_path.display()))?;
    lock_shared(&lock_file)
        .with_context(|| format!("failed to acquire shared lock {}", lock_path.display()))?;

    let result = load_snapshot_inner(&session_path, options);

    if let Err(err) = unlock(&lock_file) {
        warn!(
            "failed to unlock session file {}: {}",
            lock_path.display(),
            err
        );
    }

    match result {
        Ok(Some(loaded)) => {
            let boards = (
                loaded.snapshot.transparent.is_some(),
                loaded.snapshot.whiteboard.is_some(),
                loaded.snapshot.blackboard.is_some(),
            );
            let tool_state = loaded.snapshot.tool_state.is_some();
            info!(
                "Loaded session from {} (version {}, compressed={}, boards[T/W/B]={}/{}/{}, tool_state={})",
                session_path.display(),
                loaded.version,
                loaded.compressed,
                boards.0,
                boards.1,
                boards.2,
                tool_state
            );
            Ok(Some(loaded.snapshot))
        }
        Ok(None) => {
            info!(
                "Session file {} contained no usable data; continuing with defaults",
                session_path.display()
            );
            Ok(None)
        }
        Err(err) => {
            warn!(
                "Failed to load session {}; backing up and continuing with defaults: {}",
                session_path.display(),
                err
            );
            if let Err(backup_err) = backup_corrupt_session(&session_path, options) {
                warn!(
                    "Failed to back up corrupt session {}: {}",
                    session_path.display(),
                    backup_err
                );
            }
            Ok(None)
        }
    }
}

pub(crate) fn load_snapshot_inner(
    session_path: &Path,
    options: &SessionOptions,
) -> Result<Option<LoadedSnapshot>> {
    let mut file_bytes = Vec::new();
    {
        let mut file = File::open(session_path)
            .with_context(|| format!("failed to open session file {}", session_path.display()))?;
        file.read_to_end(&mut file_bytes)
            .context("failed to read session file")?;
    }

    let compressed = is_gzip(&file_bytes);
    let decompressed = if compressed {
        let mut decoder = GzDecoder::new(&file_bytes[..]);
        let mut out = Vec::new();
        decoder
            .read_to_end(&mut out)
            .context("failed to decompress session file")?;
        out
    } else {
        file_bytes
    };

    let original_value: serde_json::Value =
        serde_json::from_slice(&decompressed).context("failed to parse session json")?;

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

    if session_file.version > CURRENT_VERSION {
        warn!(
            "Session file version {} is newer than supported version {}; skipping load",
            session_file.version, CURRENT_VERSION
        );
        return Ok(None);
    }

    let active_mode =
        BoardMode::from_str(&session_file.active_mode).unwrap_or(BoardMode::Transparent);

    let SessionFile {
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

    let pages_from_file = |pages: Option<Vec<Frame>>,
                           active: Option<usize>,
                           legacy: Option<Frame>|
     -> Option<BoardPagesSnapshot> {
        if let Some(mut pages) = pages {
            if pages.is_empty() {
                pages.push(Frame::new());
            }
            let active = active.unwrap_or(0).min(pages.len() - 1);
            return Some(BoardPagesSnapshot { pages, active });
        }
        legacy.map(|frame| BoardPagesSnapshot {
            pages: vec![frame],
            active: 0,
        })
    };

    let mut snapshot = SessionSnapshot {
        active_mode,
        transparent: pages_from_file(transparent_pages, transparent_active_page, transparent),
        whiteboard: pages_from_file(whiteboard_pages, whiteboard_active_page, whiteboard),
        blackboard: pages_from_file(blackboard_pages, blackboard_active_page, blackboard),
        tool_state,
    };

    enforce_shape_limits(&mut snapshot, options.max_shapes_per_frame);
    let disk_history_limit = if options.persist_history {
        options.max_persisted_undo_depth
    } else {
        Some(0)
    };
    apply_history_policies(&mut snapshot.transparent, "transparent", disk_history_limit);
    apply_history_policies(&mut snapshot.whiteboard, "whiteboard", disk_history_limit);
    apply_history_policies(&mut snapshot.blackboard, "blackboard", disk_history_limit);

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

fn backup_corrupt_session(session_path: &Path, options: &SessionOptions) -> Result<()> {
    let bytes = fs::read(session_path)
        .with_context(|| format!("failed to read corrupt session {}", session_path.display()))?;
    let backup_path = options.backup_file_path();
    fs::write(&backup_path, &bytes)
        .with_context(|| format!("failed to write session backup {}", backup_path.display()))?;
    fs::remove_file(session_path).with_context(|| {
        format!(
            "failed to remove corrupt session {}",
            session_path.display()
        )
    })?;
    Ok(())
}

/// Apply a session snapshot to the live [`InputState`].
pub fn apply_snapshot(input: &mut InputState, snapshot: SessionSnapshot, options: &SessionOptions) {
    let runtime_history_limit = options.effective_history_limit(input.undo_stack_limit);

    if options.persist_transparent {
        input.canvas_set.set_pages(
            BoardMode::Transparent,
            snapshot.transparent.map(snapshot_to_board_pages),
        );
        clamp_runtime_history(
            &mut input.canvas_set,
            BoardMode::Transparent,
            runtime_history_limit,
        );
    }
    if options.persist_whiteboard {
        input.canvas_set.set_pages(
            BoardMode::Whiteboard,
            snapshot.whiteboard.map(snapshot_to_board_pages),
        );
        clamp_runtime_history(
            &mut input.canvas_set,
            BoardMode::Whiteboard,
            runtime_history_limit,
        );
    }
    if options.persist_blackboard {
        input.canvas_set.set_pages(
            BoardMode::Blackboard,
            snapshot.blackboard.map(snapshot_to_board_pages),
        );
        clamp_runtime_history(
            &mut input.canvas_set,
            BoardMode::Blackboard,
            runtime_history_limit,
        );
    }

    input.canvas_set.switch_mode(snapshot.active_mode);

    if options.restore_tool_state {
        if let Some(tool_state) = snapshot.tool_state {
            let marker_opacity = tool_state.marker_opacity.unwrap_or(input.marker_opacity);
            let fill_enabled = tool_state.fill_enabled.unwrap_or(input.fill_enabled);
            log::info!(
                "Restoring tool state: color={:?}, thickness={:.2}, eraser[size={:.2}, kind={:?}, mode={:?}], marker_opacity={:.2}, fill_enabled={}, tool_override={:?}, font_size={:.1}, text_bg={}, arrow[length={:.1}, angle={:.1}], status_bar={}, prev_color={:?}",
                tool_state.current_color,
                tool_state.current_thickness,
                tool_state.eraser_size,
                tool_state.eraser_kind,
                tool_state.eraser_mode,
                marker_opacity,
                fill_enabled,
                tool_state.tool_override,
                tool_state.current_font_size,
                tool_state.text_background_enabled,
                tool_state.arrow_length,
                tool_state.arrow_angle,
                tool_state.show_status_bar,
                tool_state.board_previous_color
            );
            let _ = input.set_color(tool_state.current_color);
            let _ = input.set_thickness(
                tool_state
                    .current_thickness
                    .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS),
            );
            let _ = input.set_eraser_size(
                tool_state
                    .eraser_size
                    .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS),
            );
            input.eraser_kind = tool_state.eraser_kind;
            let _ = input.set_eraser_mode(tool_state.eraser_mode);
            if let Some(opacity) = tool_state.marker_opacity {
                let _ = input.set_marker_opacity(opacity);
            }
            if let Some(fill_enabled) = tool_state.fill_enabled {
                let _ = input.set_fill_enabled(fill_enabled);
            }
            let _ = input.set_tool_override(tool_state.tool_override);
            let _ = input.set_font_size(tool_state.current_font_size.clamp(8.0, 72.0));
            input.text_background_enabled = tool_state.text_background_enabled;
            input.arrow_length = tool_state.arrow_length.clamp(5.0, 50.0);
            input.arrow_angle = tool_state.arrow_angle.clamp(15.0, 60.0);
            if let Some(head_at_end) = tool_state.arrow_head_at_end {
                input.arrow_head_at_end = head_at_end;
            }
            input.board_previous_color = tool_state.board_previous_color;
            input.show_status_bar = tool_state.show_status_bar;
        } else {
            log::info!("No tool state found in session; skipping tool restore");
        }
    }

    input.needs_redraw = true;
}

fn snapshot_to_board_pages(pages: BoardPagesSnapshot) -> BoardPages {
    BoardPages::from_pages(pages.pages, pages.active)
}

fn clamp_runtime_history(canvas: &mut crate::draw::CanvasSet, mode: BoardMode, limit: usize) {
    if let Some(pages) = canvas.pages_mut(mode) {
        for page in pages.pages_mut() {
            page.clamp_history_depth(limit);
        }
    }
}

fn enforce_shape_limits(snapshot: &mut SessionSnapshot, max_shapes: usize) {
    if max_shapes == 0 {
        return;
    }

    let truncate = |pages: &mut Option<BoardPagesSnapshot>, mode: &str| {
        if let Some(pages) = pages {
            for (idx, frame_data) in pages.pages.iter_mut().enumerate() {
                if frame_data.shapes.len() <= max_shapes {
                    continue;
                }
                let removed: Vec<_> = frame_data.shapes.drain(max_shapes..).collect();
                warn!(
                    "Session page '{}' (#{}) contains {} shapes which exceeds the limit of {}; truncating",
                    mode,
                    idx + 1,
                    frame_data.shapes.len() + removed.len(),
                    max_shapes
                );
                let removed_ids: HashSet<ShapeId> =
                    removed.into_iter().map(|shape| shape.id).collect();
                if !removed_ids.is_empty() {
                    let stats = frame_data.prune_history_for_removed_ids(&removed_ids);
                    if !stats.is_empty() {
                        warn!(
                            "Dropped {} undo and {} redo actions referencing trimmed shapes in '{}' page #{} history",
                            stats.undo_removed,
                            stats.redo_removed,
                            mode,
                            idx + 1
                        );
                    }
                }
            }
        }
    };

    truncate(&mut snapshot.transparent, "transparent");
    truncate(&mut snapshot.whiteboard, "whiteboard");
    truncate(&mut snapshot.blackboard, "blackboard");
}

fn apply_history_policies(
    pages: &mut Option<BoardPagesSnapshot>,
    mode: &str,
    depth_limit: Option<usize>,
) {
    if let Some(pages) = pages {
        for (idx, frame_data) in pages.pages.iter_mut().enumerate() {
            let depth_trim = frame_data.validate_history(MAX_COMPOUND_DEPTH);
            if !depth_trim.is_empty() {
                warn!(
                    "Removed {} undo and {} redo actions with invalid structure in '{}' page #{} history",
                    depth_trim.undo_removed,
                    depth_trim.redo_removed,
                    mode,
                    idx + 1
                );
            }
            let shape_trim = frame_data.prune_history_against_shapes();
            if !shape_trim.is_empty() {
                warn!(
                    "Removed {} undo and {} redo actions referencing missing shapes in '{}' page #{} history",
                    shape_trim.undo_removed,
                    shape_trim.redo_removed,
                    mode,
                    idx + 1
                );
            }
            if let Some(limit) = depth_limit {
                let trimmed = frame_data.clamp_history_depth(limit);
                if !trimmed.is_empty() {
                    debug!(
                        "Clamped '{}' page #{} history to {} entries (dropped {} undo / {} redo)",
                        mode,
                        idx + 1,
                        limit,
                        trimmed.undo_removed,
                        trimmed.redo_removed
                    );
                }
            }
        }
    }
}

fn max_history_depth(doc: &Value) -> usize {
    let mut max_depth = 0;
    for key in [
        "transparent",
        "whiteboard",
        "blackboard",
        "transparent_pages",
        "whiteboard_pages",
        "blackboard_pages",
    ] {
        if let Some(Value::Object(obj)) = doc.get(key) {
            for stack_key in ["undo_stack", "redo_stack"] {
                if let Some(Value::Array(arr)) = obj.get(stack_key) {
                    max_depth = max_depth.max(depth_array(arr));
                }
            }
        } else if let Some(Value::Array(pages)) = doc.get(key) {
            for page in pages {
                if let Some(obj) = page.as_object() {
                    for stack_key in ["undo_stack", "redo_stack"] {
                        if let Some(Value::Array(arr)) = obj.get(stack_key) {
                            max_depth = max_depth.max(depth_array(arr));
                        }
                    }
                }
            }
        }
    }
    max_depth
}

fn depth_array(arr: &[Value]) -> usize {
    arr.iter().map(depth_action).max().unwrap_or(1)
}

fn depth_action(action: &Value) -> usize {
    if let Some(obj) = action.as_object() {
        let is_compound = obj.get("kind").and_then(|v| v.as_str()) == Some("compound");
        if is_compound {
            let mut child_max = 0;
            for value in obj.values() {
                if let Value::Array(arr) = value {
                    child_max = child_max.max(depth_array(arr));
                }
            }
            return 1 + child_max;
        }
    }
    1
}

fn strip_history_fields(doc: &mut Value) {
    if let Some(obj) = doc.as_object_mut() {
        for key in [
            "transparent",
            "whiteboard",
            "blackboard",
            "transparent_pages",
            "whiteboard_pages",
            "blackboard_pages",
        ] {
            if let Some(Value::Object(frame)) = obj.get_mut(key) {
                frame.remove("undo_stack");
                frame.remove("redo_stack");
            } else if let Some(Value::Array(pages)) = obj.get_mut(key) {
                for page in pages {
                    if let Some(frame) = page.as_object_mut() {
                        frame.remove("undo_stack");
                        frame.remove("redo_stack");
                    }
                }
            }
        }
    }
}

fn board_mode_to_str(mode: BoardMode) -> &'static str {
    match mode {
        BoardMode::Transparent => "transparent",
        BoardMode::Whiteboard => "whiteboard",
        BoardMode::Blackboard => "blackboard",
    }
}

fn default_file_version() -> u32 {
    1
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

fn is_gzip(bytes: &[u8]) -> bool {
    bytes.len() > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

fn temp_path(target: &Path) -> Result<PathBuf> {
    let mut candidate = target.with_extension("json.tmp");
    let mut counter = 0u32;
    while candidate.exists() {
        counter += 1;
        candidate = target.with_extension(format!("json.tmp{}", counter));
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{Color, Frame, Shape};
    use tempfile::tempdir;

    fn sample_snapshot() -> SessionSnapshot {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        });

        SessionSnapshot {
            active_mode: BoardMode::Transparent,
            transparent: Some(BoardPagesSnapshot {
                pages: vec![frame],
                active: 0,
            }),
            whiteboard: None,
            blackboard: None,
            tool_state: None,
        }
    }

    #[test]
    fn save_snapshot_respects_auto_compression_threshold() {
        let temp = tempdir().unwrap();
        let snapshot = sample_snapshot();

        let mut plain = SessionOptions::new(temp.path().join("plain"), "plain");
        plain.persist_transparent = true;
        plain.compression = CompressionMode::Auto;
        plain.auto_compress_threshold_bytes = u64::MAX;
        save_snapshot(&snapshot, &plain).expect("save_snapshot should succeed");
        let plain_bytes = std::fs::read(plain.session_file_path()).unwrap();
        assert!(
            !is_gzip(&plain_bytes),
            "expected uncompressed session payload"
        );

        let mut compressed = SessionOptions::new(temp.path().join("compressed"), "compressed");
        compressed.persist_transparent = true;
        compressed.compression = CompressionMode::Auto;
        compressed.auto_compress_threshold_bytes = 1;
        save_snapshot(&snapshot, &compressed).expect("save_snapshot should succeed");
        let compressed_bytes = std::fs::read(compressed.session_file_path()).unwrap();
        assert!(is_gzip(&compressed_bytes), "expected gzip payload");
    }

    #[test]
    fn load_snapshot_inner_reports_compression_and_version() {
        let temp = tempdir().unwrap();
        let snapshot = sample_snapshot();

        let mut options = SessionOptions::new(temp.path().to_path_buf(), "compressed");
        options.persist_transparent = true;
        options.compression = CompressionMode::On;
        save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

        let loaded = load_snapshot_inner(&options.session_file_path(), &options)
            .expect("load_snapshot_inner should succeed")
            .expect("snapshot should be present");
        assert!(loaded.compressed);
        assert_eq!(loaded.version, CURRENT_VERSION);
        assert!(loaded.snapshot.transparent.is_some());
    }

    #[test]
    fn load_snapshot_inner_skips_newer_versions() {
        let temp = tempdir().unwrap();
        let session_path = temp.path().join("session.json");

        let file = SessionFile {
            version: CURRENT_VERSION + 1,
            last_modified: now_rfc3339(),
            active_mode: "transparent".to_string(),
            transparent: None,
            whiteboard: None,
            blackboard: None,
            transparent_pages: None,
            whiteboard_pages: None,
            blackboard_pages: None,
            transparent_active_page: None,
            whiteboard_active_page: None,
            blackboard_active_page: None,
            tool_state: None,
        };
        let bytes = serde_json::to_vec_pretty(&file).unwrap();
        std::fs::write(&session_path, bytes).unwrap();

        let options = SessionOptions::new(temp.path().to_path_buf(), "skip");
        let loaded =
            load_snapshot_inner(&session_path, &options).expect("load_snapshot_inner should work");
        assert!(loaded.is_none());
    }

    #[test]
    fn save_snapshot_preserves_multiple_pages() {
        let temp = tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "multi");
        options.persist_transparent = true;

        let mut first = Frame::new();
        first.add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        });

        let mut second = Frame::new();
        second.add_shape(Shape::Rect {
            x: 5,
            y: 5,
            w: 8,
            h: 8,
            fill: false,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 1.0,
        });

        let snapshot = SessionSnapshot {
            active_mode: BoardMode::Transparent,
            transparent: Some(BoardPagesSnapshot {
                pages: vec![first, second],
                active: 1,
            }),
            whiteboard: None,
            blackboard: None,
            tool_state: None,
        };

        save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

        let loaded = load_snapshot(&options)
            .expect("load_snapshot should succeed")
            .expect("snapshot should be present");
        let pages = loaded
            .transparent
            .expect("transparent pages should be present");
        assert_eq!(pages.pages.len(), 2);
        assert_eq!(pages.active, 1);
        assert_eq!(pages.pages[0].shapes.len(), 1);
        assert_eq!(pages.pages[1].shapes.len(), 1);
    }

    #[test]
    fn save_snapshot_keeps_empty_pages() {
        let temp = tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "empty-pages");
        options.persist_transparent = true;

        let snapshot = SessionSnapshot {
            active_mode: BoardMode::Transparent,
            transparent: Some(BoardPagesSnapshot {
                pages: vec![Frame::new(), Frame::new(), Frame::new()],
                active: 2,
            }),
            whiteboard: None,
            blackboard: None,
            tool_state: None,
        };

        save_snapshot(&snapshot, &options).expect("save_snapshot should succeed");

        let loaded = load_snapshot(&options)
            .expect("load_snapshot should succeed")
            .expect("snapshot should be present");
        let pages = loaded
            .transparent
            .expect("transparent pages should be present");
        assert_eq!(pages.pages.len(), 3);
        assert_eq!(pages.active, 2);
    }

    #[test]
    fn load_snapshot_inner_migrates_legacy_frame_to_pages() {
        let temp = tempdir().unwrap();
        let session_path = temp.path().join("session.json");

        let mut frame = Frame::new();
        frame.add_shape(Shape::Line {
            x1: 1,
            y1: 2,
            x2: 3,
            y2: 4,
            color: Color {
                r: 0.2,
                g: 0.3,
                b: 0.4,
                a: 1.0,
            },
            thick: 1.0,
        });

        let file = SessionFile {
            version: CURRENT_VERSION - 1,
            last_modified: now_rfc3339(),
            active_mode: "transparent".to_string(),
            transparent: Some(frame),
            whiteboard: None,
            blackboard: None,
            transparent_pages: None,
            whiteboard_pages: None,
            blackboard_pages: None,
            transparent_active_page: None,
            whiteboard_active_page: None,
            blackboard_active_page: None,
            tool_state: None,
        };
        let bytes = serde_json::to_vec_pretty(&file).unwrap();
        std::fs::write(&session_path, bytes).unwrap();

        let options = SessionOptions::new(temp.path().to_path_buf(), "legacy");
        let loaded = load_snapshot_inner(&session_path, &options)
            .expect("load_snapshot_inner should succeed")
            .expect("snapshot should be present");
        let pages = loaded
            .snapshot
            .transparent
            .expect("transparent pages should be present");
        assert_eq!(pages.pages.len(), 1);
        assert_eq!(pages.active, 0);
        assert_eq!(pages.pages[0].shapes.len(), 1);
    }
}
