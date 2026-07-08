use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use wayscriber::env_vars::{NO_DETACH_ENV, WAYLAND_DISPLAY_ENV, XDG_CONFIG_HOME_ENV};
use wayscriber::runtime_capabilities::RUNTIME_CAPABILITIES_FLAG;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> std::io::Result<Self> {
        let base = std::env::temp_dir();
        let pid = std::process::id();

        for _ in 0..100 {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!("wayscriber-cli-test-{pid}-{id}"));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(err),
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "failed to create a unique temporary test directory",
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct CommandOutput {
    output: Output,
}

impl CommandOutput {
    fn success(self) -> Self {
        assert!(
            self.output.status.success(),
            "expected success\nstdout:\n{}\nstderr:\n{}",
            self.stdout_text(),
            self.stderr_text()
        );
        self
    }

    fn failure(self) -> Self {
        assert!(
            !self.output.status.success(),
            "expected failure\nstdout:\n{}\nstderr:\n{}",
            self.stdout_text(),
            self.stderr_text()
        );
        self
    }

    fn stdout_contains(self, needle: &str) -> Self {
        let stdout = self.stdout_text();
        assert!(
            stdout.contains(needle),
            "stdout did not contain {needle:?}\nstdout:\n{stdout}\nstderr:\n{}",
            self.stderr_text()
        );
        self
    }

    fn stdout_starts_with(self, prefix: &str) -> Self {
        let stdout = self.stdout_text();
        assert!(
            stdout.starts_with(prefix),
            "stdout did not start with {prefix:?}\nstdout:\n{stdout}\nstderr:\n{}",
            self.stderr_text()
        );
        self
    }

    fn stdout_eq(self, expected: &str) -> Self {
        let stdout = self.stdout_text();
        assert_eq!(stdout, expected, "stderr:\n{}", self.stderr_text());
        self
    }

    fn stderr_contains(self, needle: &str) -> Self {
        let stderr = self.stderr_text();
        assert!(
            stderr.contains(needle),
            "stderr did not contain {needle:?}\nstdout:\n{}\nstderr:\n{stderr}",
            self.stdout_text()
        );
        self
    }

    fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).into_owned()
    }

    fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).into_owned()
    }
}

fn run_command(command: &mut Command) -> CommandOutput {
    CommandOutput {
        output: command.output().expect("run wayscriber command"),
    }
}

fn write_session_config(temp: &TempDir, custom_dir: &Path) {
    let config_dir = temp.path().join("wayscriber");
    fs::create_dir_all(&config_dir).unwrap();
    let config_contents = format!(
        r#"
[session]
persist_transparent = true
persist_whiteboard = false
persist_blackboard = false
restore_tool_state = true
storage = "custom"
custom_directory = "{}"
max_shapes_per_frame = 100
max_file_size_mb = 5
compress = "off"
auto_compress_threshold_kb = 100
backup_retention = 1
"#,
        custom_dir.display()
    );
    fs::write(config_dir.join("config.toml"), config_contents).unwrap();
}

fn write_disabled_session_config(temp: &TempDir, custom_dir: &Path) {
    let config_dir = temp.path().join("wayscriber");
    fs::create_dir_all(&config_dir).unwrap();
    let config_contents = format!(
        r#"
[session]
persist_transparent = false
persist_whiteboard = false
persist_blackboard = false
persist_history = false
restore_tool_state = false
storage = "custom"
custom_directory = "{}"
max_shapes_per_frame = 100
max_file_size_mb = 5
compress = "off"
auto_compress_threshold_kb = 100
backup_retention = 1
"#,
        custom_dir.display()
    );
    fs::write(config_dir.join("config.toml"), config_contents).unwrap();
}

fn saved_tool_state() -> wayscriber::session::ToolStateSnapshot {
    wayscriber::session::ToolStateSnapshot {
        current_color: wayscriber::draw::Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
        current_thickness: 3.0,
        eraser_size: 12.0,
        eraser_kind: wayscriber::draw::EraserKind::Circle,
        eraser_mode: wayscriber::input::EraserMode::Brush,
        marker_opacity: Some(0.32),
        fill_enabled: Some(false),
        tool_override: None,
        current_font_size: 24.0,
        font_descriptor: Some(wayscriber::draw::FontDescriptor::default()),
        text_background_enabled: false,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        arrow_head_at_end: Some(false),
        arrow_label_enabled: Some(false),
        polygon_sides: wayscriber::draw::REGULAR_POLYGON_DEFAULT_SIDES,
        board_previous_color: None,
        show_status_bar: true,
        tool_settings: None,
    }
}

fn saved_line_snapshot(with_tool_state: bool) -> wayscriber::session::SessionSnapshot {
    let mut frame = wayscriber::draw::Frame::new();
    frame.add_shape(wayscriber::draw::Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: wayscriber::draw::Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 2.0,
    });

    wayscriber::session::SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: vec![wayscriber::session::BoardSnapshot {
            id: "transparent".to_string(),
            pages: wayscriber::session::BoardPagesSnapshot {
                pages: vec![frame],
                active: 0,
            },
        }],
        tool_state: with_tool_state.then(saved_tool_state),
    }
}

fn wayscriber_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wayscriber"))
}

#[test]
fn wayscriber_help_prints_usage() {
    run_command(wayscriber_cmd().arg("--help"))
        .success()
        .stdout_contains("Screen annotation tool for Wayland compositors")
        .stdout_contains("--light-toggle            Toggle light passthrough mode")
        .stdout_contains("--light-draw-toggle")
        .stdout_contains("--light-draw-on")
        .stdout_contains("--light-draw-off");
}

#[test]
fn wayscriber_version_prints_binary_name() {
    for arg in ["--version", "-V"] {
        run_command(wayscriber_cmd().arg(arg))
            .success()
            .stdout_starts_with("wayscriber ")
            .stdout_contains(wayscriber::build_info::version());
    }
}

#[test]
fn wayscriber_runtime_capabilities_reports_portal_feature() {
    let expected = format!(
        "portal={}\n",
        wayscriber::shortcut_hint::portal_runtime_supported()
    );
    run_command(wayscriber_cmd().arg(RUNTIME_CAPABILITIES_FLAG))
        .success()
        .stdout_eq(&expected);
}

#[test]
fn bare_usage_mentions_freeze_on_show() {
    run_command(&mut wayscriber_cmd())
        .success()
        .stdout_contains("--freeze-on-show")
        .stdout_contains("--daemon-toggle");
}

#[test]
fn active_mode_requires_wayland_env() {
    run_command(
        wayscriber_cmd()
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--active"),
    )
    .failure()
    .stderr_contains(&format!("{WAYLAND_DISPLAY_ENV} not set"));
}

#[test]
fn session_clear_command_succeeds_without_files() {
    let temp = TempDir::new().unwrap();
    let session_dir = temp.path().join("sessions");
    write_session_config(&temp, &session_dir);

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--clear-session"),
    )
    .success()
    .stdout_contains("Session file:")
    .stdout_contains("No session file present");
}

#[test]
fn named_session_info_missing_parent_reports_not_found() {
    let temp = TempDir::new().unwrap();
    let named_path = temp
        .path()
        .join("missing")
        .join("lecture-04.wayscriber-session");

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--session-info")
            .arg("--session-file")
            .arg(&named_path),
    )
    .success()
    .stdout_contains(&format!("Session file       : {}", named_path.display()))
    .stdout_contains("(not found)");
}

#[test]
fn named_session_info_forces_persistence_despite_disabled_config() {
    let temp = TempDir::new().unwrap();
    let session_dir = temp.path().join("sessions");
    let named_path = temp.path().join("lecture-04.wayscriber-session");
    write_disabled_session_config(&temp, &session_dir);

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--session-info")
            .arg("--session-file")
            .arg(&named_path),
    )
    .success()
    .stdout_contains("Persist transparent: true")
    .stdout_contains("Persist whiteboard : true")
    .stdout_contains("Persist blackboard : true")
    .stdout_contains("Persist history    : true")
    .stdout_contains("Restore tool state : true");
}

#[test]
fn named_session_clear_missing_parent_reports_no_artifacts() {
    let temp = TempDir::new().unwrap();
    let named_path = temp
        .path()
        .join("missing")
        .join("lecture-04.wayscriber-session");

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--clear-session")
            .arg("--session-file")
            .arg(&named_path),
    )
    .success()
    .stdout_contains(&format!("Session file: {}", named_path.display()))
    .stdout_contains("No session artefacts found");
}

#[test]
fn clear_tool_state_command_succeeds_without_files() {
    let temp = TempDir::new().unwrap();
    let session_dir = temp.path().join("sessions");
    write_session_config(&temp, &session_dir);

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--clear-tool-state"),
    )
    .success()
    .stdout_contains("Session file:")
    .stdout_contains("No session file present");
}

#[test]
fn clear_tool_state_named_session_targets_only_requested_file() {
    let temp = TempDir::new().unwrap();
    let selected = temp.path().join("lecture-04.wayscriber-session");
    let sibling = temp.path().join("lecture-05.wayscriber-session");

    let mut selected_options =
        wayscriber::session::SessionOptions::new(temp.path().to_path_buf(), "display");
    selected_options.set_named_file_target(selected.clone());
    selected_options.persist_transparent = true;
    selected_options.restore_tool_state = true;

    let mut sibling_options =
        wayscriber::session::SessionOptions::new(temp.path().to_path_buf(), "display");
    sibling_options.set_named_file_target(sibling);
    sibling_options.persist_transparent = true;
    sibling_options.restore_tool_state = true;

    wayscriber::session::save_snapshot(&saved_line_snapshot(true), &selected_options).unwrap();
    wayscriber::session::save_snapshot(&saved_line_snapshot(true), &sibling_options).unwrap();

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--clear-tool-state")
            .arg("--session-file")
            .arg(&selected),
    )
    .success()
    .stdout_contains(&format!("Session file: {}", selected.display()))
    .stdout_contains("Cleared saved tool state")
    .stdout_contains("Preserved saved boards and history");

    let selected_loaded = wayscriber::session::load_snapshot(&selected_options)
        .unwrap()
        .expect("selected session should remain");
    let sibling_loaded = wayscriber::session::load_snapshot(&sibling_options)
        .unwrap()
        .expect("sibling session should remain");

    assert!(selected_loaded.tool_state.is_none());
    assert!(sibling_loaded.tool_state.is_some());
}

#[cfg(unix)]
#[test]
fn named_session_info_rejects_symlink_primary() {
    let temp = TempDir::new().unwrap();
    let target = temp.path().join("real-session.wayscriber-session");
    let link = temp.path().join("linked-session.wayscriber-session");
    fs::write(&target, b"{}").unwrap();
    symlink(&target, &link).unwrap();

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--session-info")
            .arg("--session-file")
            .arg(&link),
    )
    .failure()
    .stderr_contains("not a symlink");
}

#[cfg(unix)]
#[test]
fn named_session_clear_rejects_symlink_primary_without_removing_artifacts() {
    let temp = TempDir::new().unwrap();
    let target = temp.path().join("real-session.wayscriber-session");
    let link = temp.path().join("linked-session.wayscriber-session");
    let mut backup_raw = std::ffi::OsString::from(link.as_os_str());
    backup_raw.push(".bak");
    let backup = PathBuf::from(backup_raw);
    fs::write(&target, b"target").unwrap();
    fs::write(&backup, b"backup").unwrap();
    symlink(&target, &link).unwrap();

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--clear-session")
            .arg("--session-file")
            .arg(&link),
    )
    .failure()
    .stderr_contains("not a symlink");

    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&target).unwrap(), b"target");
    assert_eq!(fs::read(&backup).unwrap(), b"backup");
}

#[cfg(unix)]
#[test]
fn active_named_session_rejects_symlink_primary_before_wayland_preflight() {
    let temp = TempDir::new().unwrap();
    let target = temp.path().join("real-session.wayscriber-session");
    let link = temp.path().join("linked-session.wayscriber-session");
    fs::write(&target, b"{}").unwrap();
    symlink(&target, &link).unwrap();

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env(NO_DETACH_ENV, "1")
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--active")
            .arg("--session-file")
            .arg(&link),
    )
    .failure()
    .stderr_contains("not a symlink");
}

#[cfg(unix)]
#[test]
fn freeze_named_session_rejects_symlink_primary_before_wayland_preflight() {
    let temp = TempDir::new().unwrap();
    let target = temp.path().join("real-session.wayscriber-session");
    let link = temp.path().join("linked-session.wayscriber-session");
    fs::write(&target, b"{}").unwrap();
    symlink(&target, &link).unwrap();

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env(NO_DETACH_ENV, "1")
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--freeze")
            .arg("--session-file")
            .arg(&link),
    )
    .failure()
    .stderr_contains("not a symlink");
}

#[test]
fn active_named_session_missing_parent_fails_before_wayland_preflight() {
    let temp = TempDir::new().unwrap();
    let named_path = temp
        .path()
        .join("missing")
        .join("lecture-04.wayscriber-session");

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--active")
            .arg("--session-file")
            .arg(&named_path),
    )
    .failure()
    .stderr_contains("named session parent directory does not exist")
    .stderr_contains(&named_path.parent().unwrap().display().to_string());
}

#[test]
fn active_named_session_directory_path_fails_before_wayland_preflight() {
    let temp = TempDir::new().unwrap();

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--active")
            .arg("--session-file")
            .arg(temp.path()),
    )
    .failure()
    .stderr_contains("--session-file must name a session file, not a directory");
}

#[test]
fn freeze_named_session_missing_parent_fails_before_wayland_preflight() {
    let temp = TempDir::new().unwrap();
    let named_path = temp
        .path()
        .join("missing")
        .join("lecture-04.wayscriber-session");

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--freeze")
            .arg("--session-file")
            .arg(&named_path),
    )
    .failure()
    .stderr_contains("named session parent directory does not exist");
}

#[test]
fn freeze_named_session_missing_wayland_fails_before_detach() {
    let temp = TempDir::new().unwrap();
    let named_path = temp.path().join("lecture-04.wayscriber-session");

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env_remove(WAYLAND_DISPLAY_ENV)
            .arg("--freeze")
            .arg("--session-file")
            .arg(&named_path),
    )
    .failure()
    .stderr_contains(&format!("{WAYLAND_DISPLAY_ENV} not set"));
}

#[test]
fn session_info_reports_saved_snapshot() {
    let temp = TempDir::new().unwrap();
    let session_dir = temp.path().join("sessions");
    write_session_config(&temp, &session_dir);

    let display = "test-session";
    let original_config = std::env::var_os(XDG_CONFIG_HOME_ENV);
    unsafe {
        std::env::set_var(XDG_CONFIG_HOME_ENV, temp.path());
    }
    let original_display = std::env::var_os(WAYLAND_DISPLAY_ENV);
    unsafe {
        std::env::set_var(WAYLAND_DISPLAY_ENV, display);
    }

    let loaded = wayscriber::config::Config::load().unwrap();
    let config_dir =
        wayscriber::config::Config::config_directory_from_source(&loaded.source).unwrap();
    let mut options = wayscriber::session::options_from_config(
        &loaded.config.session,
        &config_dir,
        Some(display),
    )
    .unwrap();
    options.set_output_identity(Some("DP-1"));

    match original_config {
        Some(value) => unsafe { std::env::set_var(XDG_CONFIG_HOME_ENV, value) },
        None => unsafe { std::env::remove_var(XDG_CONFIG_HOME_ENV) },
    }

    match original_display {
        Some(value) => unsafe { std::env::set_var(WAYLAND_DISPLAY_ENV, value) },
        None => unsafe { std::env::remove_var(WAYLAND_DISPLAY_ENV) },
    }

    let mut frame = wayscriber::draw::Frame::new();
    frame.add_shape(wayscriber::draw::Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: wayscriber::draw::Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 2.0,
    });

    let snapshot = wayscriber::session::SessionSnapshot {
        active_board_id: "transparent".to_string(),
        boards: vec![wayscriber::session::BoardSnapshot {
            id: "transparent".to_string(),
            pages: wayscriber::session::BoardPagesSnapshot {
                pages: vec![frame],
                active: 0,
            },
        }],
        tool_state: None,
    };

    wayscriber::session::save_snapshot(&snapshot, &options).unwrap();

    run_command(
        wayscriber_cmd()
            .env(XDG_CONFIG_HOME_ENV, temp.path())
            .env(WAYLAND_DISPLAY_ENV, display)
            .arg("--session-info"),
    )
    .success()
    .stdout_contains("Per-output persistence: true")
    .stdout_contains("Session file       :")
    .stdout_contains("Output identity: DP_1")
    .stdout_contains("transparent 1")
    .stdout_contains("Tool state stored: false");
}
