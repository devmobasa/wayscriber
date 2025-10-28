use super::options::{CompressionMode, SessionOptions};
use crate::draw::{Color, Frame};
use crate::input::{InputState, board_mode::BoardMode};
use anyhow::{Context, Result};
use chrono::Utc;
use flate2::{Compression, bufread::GzDecoder, write::GzEncoder};
use fs2::FileExt;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

const CURRENT_VERSION: u32 = 1;

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
        let empty_frame =
            |frame: &Option<Frame>| frame.as_ref().map_or(true, |data| data.shapes.is_empty());
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
            current_font_size: input.current_font_size,
            text_background_enabled: input.text_background_enabled,
            arrow_length: input.arrow_length,
            arrow_angle: input.arrow_angle,
            board_previous_color: input.board_previous_color,
            show_status_bar: input.show_status_bar,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionFile {
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
}

/// Capture a snapshot from the current input state if persistence is enabled.
pub fn snapshot_from_input(
    input: &InputState,
    options: &SessionOptions,
) -> Option<SessionSnapshot> {
    if !options.any_enabled() && !options.restore_tool_state {
        return None;
    }

    let mut snapshot = SessionSnapshot {
        active_mode: input.board_mode(),
        transparent: None,
        whiteboard: None,
        blackboard: None,
        tool_state: None,
    };

    if options.persist_transparent {
        if let Some(frame) = input.canvas_set.frame(BoardMode::Transparent) {
            if !frame.shapes.is_empty() {
                snapshot.transparent = Some(frame.clone());
            }
        }
    }

    if options.persist_whiteboard {
        if let Some(frame) = input.canvas_set.frame(BoardMode::Whiteboard) {
            if !frame.shapes.is_empty() {
                snapshot.whiteboard = Some(frame.clone());
            }
        }
    }

    if options.persist_blackboard {
        if let Some(frame) = input.canvas_set.frame(BoardMode::Blackboard) {
            if !frame.shapes.is_empty() {
                snapshot.blackboard = Some(frame.clone());
            }
        }
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
        .open(&lock_path)
        .with_context(|| format!("failed to open session lock file {}", lock_path.display()))?;
    lock_file
        .lock_exclusive()
        .with_context(|| format!("failed to lock session file {}", lock_path.display()))?;

    let result = save_snapshot_inner(snapshot, options);

    lock_file.unlock().unwrap_or_else(|err| {
        warn!(
            "failed to unlock session file {}: {}",
            lock_path.display(),
            err
        )
    });

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
        last_modified: Utc::now().to_rfc3339(),
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
        debug!("Persistence disabled for all boards; skipping load");
        return Ok(None);
    }

    let session_path = options.session_file_path();
    if !session_path.exists() {
        debug!(
            "No session file present at {}, skipping load",
            session_path.display()
        );
        return Ok(None);
    }

    let metadata = fs::metadata(&session_path)
        .with_context(|| format!("failed to stat session file {}", session_path.display()))?;
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
        .open(&lock_path)
        .with_context(|| format!("failed to open session lock file {}", lock_path.display()))?;
    lock_file
        .lock_shared()
        .with_context(|| format!("failed to acquire shared lock {}", lock_path.display()))?;

    let result = load_snapshot_inner(&session_path, options);

    lock_file.unlock().unwrap_or_else(|err| {
        warn!(
            "failed to unlock session file {}: {}",
            lock_path.display(),
            err
        )
    });

    match result? {
        Some(loaded) => Ok(Some(loaded.snapshot)),
        None => Ok(None),
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

    let session_file: SessionFile =
        serde_json::from_slice(&decompressed).context("failed to parse session json")?;

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
    }))
}

/// Apply a session snapshot to the live [`InputState`].
pub fn apply_snapshot(input: &mut InputState, snapshot: SessionSnapshot, options: &SessionOptions) {
    if options.persist_transparent {
        input
            .canvas_set
            .set_frame(BoardMode::Transparent, snapshot.transparent);
    }
    if options.persist_whiteboard {
        input
            .canvas_set
            .set_frame(BoardMode::Whiteboard, snapshot.whiteboard);
    }
    if options.persist_blackboard {
        input
            .canvas_set
            .set_frame(BoardMode::Blackboard, snapshot.blackboard);
    }

    input.canvas_set.switch_mode(snapshot.active_mode);

    if options.restore_tool_state {
        if let Some(tool_state) = snapshot.tool_state {
            input.current_color = tool_state.current_color;
            input.current_thickness = tool_state.current_thickness.clamp(1.0, 20.0);
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

fn enforce_shape_limits(snapshot: &mut SessionSnapshot, max_shapes: usize) {
    let truncate = |frame: &mut Option<Frame>, mode: &str| {
        if let Some(frame_data) = frame {
            if frame_data.shapes.len() > max_shapes {
                warn!(
                    "Session frame '{}' contains {} shapes which exceeds the limit of {}; truncating",
                    mode,
                    frame_data.shapes.len(),
                    max_shapes
                );
                frame_data.shapes.truncate(max_shapes);
            }
        }
    };

    truncate(&mut snapshot.transparent, "transparent");
    truncate(&mut snapshot.whiteboard, "whiteboard");
    truncate(&mut snapshot.blackboard, "blackboard");
}

fn board_mode_to_str(mode: BoardMode) -> &'static str {
    match mode {
        BoardMode::Transparent => "transparent",
        BoardMode::Whiteboard => "whiteboard",
        BoardMode::Blackboard => "blackboard",
    }
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
