use super::options::{CompressionMode, SessionOptions};
use crate::draw::frame::{MAX_COMPOUND_DEPTH, ShapeId};
use crate::draw::{Color, EraserKind, Frame};
use crate::input::{
    InputState,
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

const CURRENT_VERSION: u32 = 3;

/// Captured state suitable for serialisation or restoration.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub active_mode: BoardMode,
    pub transparent: Option<Frame>,
    pub whiteboard: Option<Frame>,
    pub blackboard: Option<Frame>,
    pub tool_state: Option<ToolStateSnapshot>,
}

impl SessionSnapshot {
    fn is_empty(&self) -> bool {
        let empty_frame = |frame: &Option<Frame>| {
            frame
                .as_ref()
                .is_none_or(|data| !data.has_persistable_data())
        };
        empty_frame(&self.transparent)
            && empty_frame(&self.whiteboard)
            && empty_frame(&self.blackboard)
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
    pub current_font_size: f64,
    pub text_background_enabled: bool,
    pub arrow_length: f64,
    pub arrow_angle: f64,
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
            current_font_size: input.current_font_size,
            text_background_enabled: input.text_background_enabled,
            arrow_length: input.arrow_length,
            arrow_angle: input.arrow_angle,
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

#[derive(Debug, Serialize, Deserialize)]
struct SessionFile {
    #[serde(default = "default_file_version")]
    version: u32,
    last_modified: String,
    active_mode: String,
    #[serde(default)]
    transparent: Option<Frame>,
    #[serde(default)]
    whiteboard: Option<Frame>,
    #[serde(default)]
    blackboard: Option<Frame>,
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

    let capture_frame = |mode: BoardMode| -> Option<Frame> {
        let frame = input.canvas_set.frame(mode)?;
        let mut cloned = frame.clone();
        if history_limit == 0 {
            cloned.clamp_history_depth(0);
        } else if history_limit < usize::MAX {
            cloned.clamp_history_depth(history_limit);
        }
        if cloned.has_persistable_data() {
            Some(cloned)
        } else {
            None
        }
    };

    if options.persist_transparent {
        snapshot.transparent = capture_frame(BoardMode::Transparent);
    }

    if options.persist_whiteboard {
        snapshot.whiteboard = capture_frame(BoardMode::Whiteboard);
    }

    if options.persist_blackboard {
        snapshot.blackboard = capture_frame(BoardMode::Blackboard);
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

    let file_payload = SessionFile {
        version: CURRENT_VERSION,
        last_modified: now_rfc3339(),
        active_mode: board_mode_to_str(snapshot.active_mode).to_string(),
        transparent: snapshot.transparent.clone(),
        whiteboard: snapshot.whiteboard.clone(),
        blackboard: snapshot.blackboard.clone(),
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

    let mut snapshot = SessionSnapshot {
        active_mode,
        transparent: session_file.transparent,
        whiteboard: session_file.whiteboard,
        blackboard: session_file.blackboard,
        tool_state: session_file.tool_state,
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
        input
            .canvas_set
            .set_frame(BoardMode::Transparent, snapshot.transparent);
        clamp_runtime_history(
            &mut input.canvas_set,
            BoardMode::Transparent,
            runtime_history_limit,
        );
    }
    if options.persist_whiteboard {
        input
            .canvas_set
            .set_frame(BoardMode::Whiteboard, snapshot.whiteboard);
        clamp_runtime_history(
            &mut input.canvas_set,
            BoardMode::Whiteboard,
            runtime_history_limit,
        );
    }
    if options.persist_blackboard {
        input
            .canvas_set
            .set_frame(BoardMode::Blackboard, snapshot.blackboard);
        clamp_runtime_history(
            &mut input.canvas_set,
            BoardMode::Blackboard,
            runtime_history_limit,
        );
    }

    input.canvas_set.switch_mode(snapshot.active_mode);

    if options.restore_tool_state {
        if let Some(tool_state) = snapshot.tool_state {
            input.current_color = tool_state.current_color;
            input.current_thickness = tool_state
                .current_thickness
                .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
            input.eraser_size = tool_state
                .eraser_size
                .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
            input.eraser_kind = tool_state.eraser_kind;
            input.current_font_size = tool_state.current_font_size.clamp(8.0, 72.0);
            input.text_background_enabled = tool_state.text_background_enabled;
            input.arrow_length = tool_state.arrow_length.clamp(5.0, 50.0);
            input.arrow_angle = tool_state.arrow_angle.clamp(15.0, 60.0);
            input.board_previous_color = tool_state.board_previous_color;
            input.show_status_bar = tool_state.show_status_bar;
        }
    }

    input.needs_redraw = true;
}

fn clamp_runtime_history(canvas: &mut crate::draw::CanvasSet, mode: BoardMode, limit: usize) {
    if let Some(frame) = canvas.frame_mut(mode) {
        frame.clamp_history_depth(limit);
    }
}

fn enforce_shape_limits(snapshot: &mut SessionSnapshot, max_shapes: usize) {
    if max_shapes == 0 {
        return;
    }

    let truncate = |frame: &mut Option<Frame>, mode: &str| {
        if let Some(frame_data) = frame {
            if frame_data.shapes.len() > max_shapes {
                let removed: Vec<_> = frame_data.shapes.drain(max_shapes..).collect();
                warn!(
                    "Session frame '{}' contains {} shapes which exceeds the limit of {}; truncating",
                    mode,
                    frame_data.shapes.len() + removed.len(),
                    max_shapes
                );
                let removed_ids: HashSet<ShapeId> =
                    removed.into_iter().map(|shape| shape.id).collect();
                if !removed_ids.is_empty() {
                    let stats = frame_data.prune_history_for_removed_ids(&removed_ids);
                    if !stats.is_empty() {
                        warn!(
                            "Dropped {} undo and {} redo actions referencing trimmed shapes in '{}' history",
                            stats.undo_removed, stats.redo_removed, mode
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

fn apply_history_policies(frame: &mut Option<Frame>, mode: &str, depth_limit: Option<usize>) {
    if let Some(frame_data) = frame {
        let depth_trim = frame_data.validate_history(MAX_COMPOUND_DEPTH);
        if !depth_trim.is_empty() {
            warn!(
                "Removed {} undo and {} redo actions with invalid structure in '{}' history",
                depth_trim.undo_removed, depth_trim.redo_removed, mode
            );
        }
        let shape_trim = frame_data.prune_history_against_shapes();
        if !shape_trim.is_empty() {
            warn!(
                "Removed {} undo and {} redo actions referencing missing shapes in '{}' history",
                shape_trim.undo_removed, shape_trim.redo_removed, mode
            );
        }
        if let Some(limit) = depth_limit {
            let trimmed = frame_data.clamp_history_depth(limit);
            if !trimmed.is_empty() {
                debug!(
                    "Clamped '{}' history to {} entries (dropped {} undo / {} redo)",
                    mode, limit, trimmed.undo_removed, trimmed.redo_removed
                );
            }
        }
    }
}

fn max_history_depth(doc: &Value) -> usize {
    let mut max_depth = 0;
    for key in ["transparent", "whiteboard", "blackboard"] {
        if let Some(frame) = doc.get(key) {
            if let Some(obj) = frame.as_object() {
                for stack_key in ["undo_stack", "redo_stack"] {
                    if let Some(Value::Array(arr)) = obj.get(stack_key) {
                        max_depth = max_depth.max(depth_array(arr));
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
        for key in ["transparent", "whiteboard", "blackboard"] {
            if let Some(Value::Object(frame)) = obj.get_mut(key) {
                frame.remove("undo_stack");
                frame.remove("redo_stack");
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
