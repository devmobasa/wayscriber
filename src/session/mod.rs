//! Session persistence (save/restore) support.
//!
//! Converts in-memory drawing state into a serialised representation, writes it
//! to disk with locking, optional compression, and backup rotation, and restores
//! the state on startup when requested.

use crate::config::{SessionCompression, SessionConfig, SessionStorageMode};
use crate::draw::{Color, Frame};
use crate::input::{InputState, board_mode::BoardMode};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use flate2::{Compression, bufread::GzDecoder, write::GzEncoder};
use fs2::FileExt;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::SystemTime;

const CURRENT_VERSION: u32 = 1;
const DEFAULT_AUTO_COMPRESS_THRESHOLD_BYTES: u64 = 100 * 1024; // 100 KiB

/// Compression preference for session files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMode {
    /// Always write plain JSON.
    Off,
    /// Always write gzip-compressed JSON.
    On,
    /// Write gzip when payload exceeds the configured threshold.
    Auto,
}

/// Runtime options derived from configuration for session persistence.
#[derive(Debug, Clone)]
pub struct SessionOptions {
    pub base_dir: PathBuf,
    pub persist_transparent: bool,
    pub persist_whiteboard: bool,
    pub persist_blackboard: bool,
    pub restore_tool_state: bool,
    pub max_shapes_per_frame: usize,
    pub max_file_size_bytes: u64,
    pub compression: CompressionMode,
    pub auto_compress_threshold_bytes: u64,
    pub display_id: String,
    pub backup_retention: usize,
    pub output_identity: Option<String>,
    pub per_output: bool,
}

impl SessionOptions {
    /// Creates a basic options struct with sensible defaults. Intended mainly for tests.
    pub fn new(base_dir: PathBuf, display_id: impl Into<String>) -> Self {
        let raw_display = display_id.into();
        let display_id = sanitize_identifier(&raw_display);
        Self {
            base_dir,
            persist_transparent: false,
            persist_whiteboard: false,
            persist_blackboard: false,
            restore_tool_state: true,
            max_shapes_per_frame: 10_000,
            max_file_size_bytes: 10 * 1024 * 1024,
            compression: CompressionMode::Auto,
            auto_compress_threshold_bytes: DEFAULT_AUTO_COMPRESS_THRESHOLD_BYTES,
            display_id,
            backup_retention: 1,
            output_identity: None,
            per_output: true,
        }
    }

    pub fn any_enabled(&self) -> bool {
        self.persist_transparent || self.persist_whiteboard || self.persist_blackboard
    }

    pub fn session_file_path(&self) -> PathBuf {
        self.base_dir
            .join(format!("{}.json", self.session_file_stem()))
    }

    pub fn backup_file_path(&self) -> PathBuf {
        self.base_dir
            .join(format!("{}.json.bak", self.session_file_stem()))
    }

    pub fn lock_file_path(&self) -> PathBuf {
        self.base_dir
            .join(format!("{}.lock", self.session_file_stem()))
    }

    pub fn file_prefix(&self) -> String {
        format!("session-{}", self.display_id)
    }

    fn session_file_stem(&self) -> String {
        if self.per_output {
            match &self.output_identity {
                Some(identity) => format!("{}-{}", self.file_prefix(), identity),
                None => self.file_prefix(),
            }
        } else {
            self.file_prefix()
        }
    }

    pub fn set_output_identity(&mut self, identity: Option<&str>) -> bool {
        if !self.per_output {
            self.output_identity = None;
            return false;
        }
        let sanitized = identity.map(|s| sanitize_identifier(s));
        if self.output_identity == sanitized {
            false
        } else {
            self.output_identity = sanitized;
            true
        }
    }

    pub fn output_identity(&self) -> Option<&str> {
        self.output_identity.as_deref()
    }
}

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

/// Build runtime session options from configuration values.
pub fn options_from_config(
    session_cfg: &SessionConfig,
    config_dir: &Path,
    display_id: Option<&str>,
) -> Result<SessionOptions> {
    let base_dir = match session_cfg.storage {
        SessionStorageMode::Auto => {
            let root = dirs::data_dir().unwrap_or_else(|| config_dir.to_path_buf());
            root.join("wayscriber")
        }
        SessionStorageMode::Config => config_dir.to_path_buf(),
        SessionStorageMode::Custom => {
            let raw = session_cfg.custom_directory.as_ref().ok_or_else(|| {
                anyhow!("session.custom_directory must be set when storage = \"custom\"")
            })?;
            let expanded = expand_tilde(raw);
            if expanded.as_os_str().is_empty() {
                return Err(anyhow!(
                    "session.custom_directory resolved to an empty path"
                ));
            }
            expanded
        }
    };

    let mut options = SessionOptions::new(base_dir, resolve_display_id(display_id));
    options.persist_transparent = session_cfg.persist_transparent;
    options.persist_whiteboard = session_cfg.persist_whiteboard;
    options.persist_blackboard = session_cfg.persist_blackboard;
    options.restore_tool_state = session_cfg.restore_tool_state;
    options.max_shapes_per_frame = session_cfg.max_shapes_per_frame;
    options.max_file_size_bytes = session_cfg
        .max_file_size_mb
        .saturating_mul(1024 * 1024)
        .max(1);
    options.auto_compress_threshold_bytes = session_cfg
        .auto_compress_threshold_kb
        .saturating_mul(1024)
        .max(1);
    options.compression = match session_cfg.compress {
        SessionCompression::Auto => CompressionMode::Auto,
        SessionCompression::On => CompressionMode::On,
        SessionCompression::Off => CompressionMode::Off,
    };
    options.backup_retention = session_cfg.backup_retention;
    options.per_output = session_cfg.per_output;

    Ok(options)
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

struct LoadedSnapshot {
    snapshot: SessionSnapshot,
    compressed: bool,
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

fn load_snapshot_inner(
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

fn sanitize_identifier(raw: &str) -> String {
    if raw.is_empty() {
        return "default".to_string();
    }

    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn resolve_display_id(display_id: Option<&str>) -> String {
    if let Some(id) = display_id {
        return sanitize_identifier(id);
    }

    match env::var("WAYLAND_DISPLAY") {
        Ok(value) => sanitize_identifier(&value),
        Err(_) => "default".to_string(),
    }
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(path)
}

/// Result of clearing on-disk session data.
#[derive(Debug, Clone, Copy)]
pub struct ClearOutcome {
    pub removed_session: bool,
    pub removed_backup: bool,
    pub removed_lock: bool,
}

/// Summary information about the current session file(s).
#[derive(Debug, Clone)]
pub struct SessionInspection {
    pub session_path: PathBuf,
    pub exists: bool,
    pub size_bytes: Option<u64>,
    pub modified: Option<SystemTime>,
    pub backup_path: PathBuf,
    pub backup_exists: bool,
    pub backup_size_bytes: Option<u64>,
    pub active_identity: Option<String>,
    pub per_output: bool,
    pub persist_transparent: bool,
    pub persist_whiteboard: bool,
    pub persist_blackboard: bool,
    pub restore_tool_state: bool,
    pub frame_counts: Option<FrameCounts>,
    pub tool_state_present: bool,
    pub compressed: bool,
}

/// Frame counts for each board stored in the session.
#[derive(Debug, Clone, Copy)]
pub struct FrameCounts {
    pub transparent: usize,
    pub whiteboard: usize,
    pub blackboard: usize,
}

/// Remove persisted session files (session, backup, and lock).
pub fn clear_session(options: &SessionOptions) -> Result<ClearOutcome> {
    let session_path = options.session_file_path();
    let backup_path = options.backup_file_path();
    let lock_path = options.lock_file_path();

    let mut removed_session = remove_file_if_exists(&session_path)?;
    let mut removed_backup = remove_file_if_exists(&backup_path)?;
    let mut removed_lock = remove_file_if_exists(&lock_path)?;

    if options.per_output && options.output_identity().is_none() {
        let prefix = options.file_prefix();
        let base_dir = &options.base_dir;

        if !removed_session {
            removed_session = remove_matching_files(base_dir, &prefix, ".json")? || removed_session;
        }

        if !removed_backup {
            removed_backup =
                remove_matching_files(base_dir, &prefix, ".json.bak")? || removed_backup;
        }

        if !removed_lock {
            removed_lock = remove_matching_files(base_dir, &prefix, ".lock")? || removed_lock;
        }
    }

    Ok(ClearOutcome {
        removed_session,
        removed_backup,
        removed_lock,
    })
}

fn remove_file_if_exists(path: &Path) -> Result<bool> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn remove_matching_files(dir: &Path, prefix: &str, suffix: &str) -> Result<bool> {
    let mut removed = false;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with(prefix) && name.ends_with(suffix) {
                    fs::remove_file(&path)
                        .with_context(|| format!("failed to remove {}", path.display()))?;
                    removed = true;
                }
            }
        }
    }
    Ok(removed)
}

fn find_existing_variant(
    dir: &Path,
    prefix: &str,
    suffix: &str,
) -> Option<(PathBuf, Option<String>)> {
    let entries = fs::read_dir(dir).ok()?;
    let mut matches: Vec<(PathBuf, Option<String>)> = Vec::new();

    for entry in entries {
        let entry = entry.ok()?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
        {
            if name.starts_with(prefix) && name.ends_with(suffix) {
                matches.push((path, extract_identity(&name, prefix, suffix)));
            }
        }
    }

    matches.sort_by(|a, b| {
        let a_name = a.0.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let b_name = b.0.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        a_name.cmp(b_name)
    });

    matches.into_iter().next()
}

fn extract_identity(name: &str, prefix: &str, suffix: &str) -> Option<String> {
    if !name.starts_with(prefix) || !name.ends_with(suffix) {
        return None;
    }

    let start = prefix.len();
    let end = name.len() - suffix.len();
    if start >= end {
        return None;
    }

    let trimmed = name[start..end].trim_start_matches('-');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Inspect the current session file for CLI reporting.
pub fn inspect_session(options: &SessionOptions) -> Result<SessionInspection> {
    let prefix = options.file_prefix();
    let mut session_path = options.session_file_path();
    let mut session_identity = options.output_identity().map(|s| s.to_string());
    let mut metadata = fs::metadata(&session_path).ok();

    if metadata.is_none() && options.per_output && options.output_identity().is_none() {
        if let Some((path, identity)) = find_existing_variant(&options.base_dir, &prefix, ".json") {
            metadata = fs::metadata(&path).ok();
            session_path = path;
            session_identity = identity;
        }
    }

    let exists = metadata.is_some();
    let size_bytes = metadata.as_ref().map(|m| m.len());
    let modified = metadata.as_ref().and_then(|m| m.modified().ok());

    let mut backup_path = options.backup_file_path();
    let mut backup_meta = fs::metadata(&backup_path).ok();
    if backup_meta.is_none() && options.per_output && options.output_identity().is_none() {
        if let Some((path, _)) = find_existing_variant(&options.base_dir, &prefix, ".json.bak") {
            backup_meta = fs::metadata(&path).ok();
            backup_path = path;
        }
    }

    let backup_exists = backup_meta.is_some();
    let backup_size = backup_meta.as_ref().map(|m| m.len());

    let mut frame_counts = None;
    let mut tool_state_present = false;
    let mut compressed = false;

    if exists {
        let lock_path = session_path.with_extension("lock");
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)
            .with_context(|| format!("failed to open session lock file {}", lock_path.display()))?;
        lock_file
            .lock_shared()
            .with_context(|| format!("failed to acquire shared lock {}", lock_path.display()))?;

        let loaded = load_snapshot_inner(&session_path, options);

        lock_file.unlock().unwrap_or_else(|err| {
            warn!(
                "failed to unlock session file {}: {}",
                lock_path.display(),
                err
            )
        });

        if let Some(loaded) = loaded? {
            frame_counts = Some(FrameCounts {
                transparent: loaded
                    .snapshot
                    .transparent
                    .as_ref()
                    .map_or(0, |f| f.shapes.len()),
                whiteboard: loaded
                    .snapshot
                    .whiteboard
                    .as_ref()
                    .map_or(0, |f| f.shapes.len()),
                blackboard: loaded
                    .snapshot
                    .blackboard
                    .as_ref()
                    .map_or(0, |f| f.shapes.len()),
            });
            tool_state_present = loaded.snapshot.tool_state.is_some();
            compressed = loaded.compressed;
        }
    }

    Ok(SessionInspection {
        session_path,
        exists,
        size_bytes,
        modified,
        backup_path,
        backup_exists,
        backup_size_bytes: backup_size,
        active_identity: session_identity,
        per_output: options.per_output,
        persist_transparent: options.persist_transparent,
        persist_whiteboard: options.persist_whiteboard,
        persist_blackboard: options.persist_blackboard,
        restore_tool_state: options.restore_tool_state,
        frame_counts,
        tool_state_present,
        compressed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardConfig, SessionConfig, SessionStorageMode};
    use crate::draw::FontDescriptor;
    use crate::draw::Shape;
    use crate::draw::color::Color;
    use crate::input::InputState;
    use std::collections::HashMap;

    fn dummy_input_state() -> InputState {
        use crate::config::{Action, KeyBinding};
        use crate::draw::Color as DrawColor;

        let mut action_map = HashMap::new();
        action_map.insert(KeyBinding::parse("Escape").unwrap(), Action::Exit);
        InputState::with_defaults(
            DrawColor {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            3.0,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            true,
            BoardConfig::default(),
            action_map,
        )
    }

    #[test]
    fn snapshot_skips_when_empty_and_no_tool_state() {
        let options = SessionOptions {
            base_dir: PathBuf::from("/tmp"),
            persist_transparent: true,
            persist_whiteboard: false,
            persist_blackboard: false,
            restore_tool_state: false,
            max_shapes_per_frame: 100,
            max_file_size_bytes: 1024 * 1024,
            compression: CompressionMode::Off,
            auto_compress_threshold_bytes: DEFAULT_AUTO_COMPRESS_THRESHOLD_BYTES,
            display_id: "test".into(),
            backup_retention: 1,
            output_identity: None,
            per_output: true,
        };

        let input = dummy_input_state();
        assert!(snapshot_from_input(&input, &options).is_none());
    }

    #[test]
    fn snapshot_includes_frames_and_tool_state() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
        options.persist_transparent = true;

        let mut input = dummy_input_state();
        input.canvas_set.active_frame_mut().add_shape(Shape::Line {
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

        let snapshot = snapshot_from_input(&input, &options).expect("snapshot present");
        assert!(snapshot.transparent.is_some());
        assert!(snapshot.tool_state.is_some());
    }

    #[test]
    fn options_from_config_custom_storage() {
        let temp = tempfile::tempdir().unwrap();
        let custom_dir = temp.path().join("sessions");

        let mut cfg = SessionConfig::default();
        cfg.persist_transparent = true;
        cfg.storage = SessionStorageMode::Custom;
        cfg.custom_directory = Some(custom_dir.to_string_lossy().to_string());

        let mut options = options_from_config(&cfg, temp.path(), Some("display-1")).unwrap();
        assert_eq!(options.base_dir, custom_dir);
        assert!(options.persist_transparent);
        options.set_output_identity(Some("DP-1"));
        assert_eq!(
            options
                .session_file_path()
                .file_name()
                .unwrap()
                .to_string_lossy(),
            "session-display_1-DP_1.json"
        );
    }

    #[test]
    fn options_from_config_config_storage_uses_config_dir() {
        let temp = tempfile::tempdir().unwrap();

        let mut cfg = SessionConfig::default();
        cfg.persist_whiteboard = true;
        cfg.storage = SessionStorageMode::Config;

        let original_display = std::env::var_os("WAYLAND_DISPLAY");
        unsafe {
            std::env::remove_var("WAYLAND_DISPLAY");
        }

        let mut options = options_from_config(&cfg, temp.path(), None).unwrap();
        match original_display {
            Some(value) => unsafe { std::env::set_var("WAYLAND_DISPLAY", value) },
            None => {}
        };

        assert_eq!(options.base_dir, temp.path());
        assert!(options.persist_whiteboard);
        assert_eq!(
            options
                .session_file_path()
                .file_name()
                .unwrap()
                .to_string_lossy(),
            "session-default.json"
        );
        options.set_output_identity(Some("Monitor-Primary"));
        assert_eq!(
            options
                .session_file_path()
                .file_name()
                .unwrap()
                .to_string_lossy(),
            "session-default-Monitor_Primary.json"
        );
    }

    #[test]
    fn session_file_without_per_output_suffix_when_disabled() {
        let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
        options.per_output = false;
        let original = options.session_file_path();
        options.set_output_identity(Some("DP-1"));
        assert_eq!(options.session_file_path(), original);
        assert!(options.output_identity().is_none());
    }
}
