use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
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
            .env_remove("WAYLAND_DISPLAY")
            .arg("--active"),
    )
    .failure()
    .stderr_contains("WAYLAND_DISPLAY not set");
}

#[test]
fn session_clear_command_succeeds_without_files() {
    let temp = TempDir::new().unwrap();
    let session_dir = temp.path().join("sessions");
    write_session_config(&temp, &session_dir);

    run_command(
        wayscriber_cmd()
            .env("XDG_CONFIG_HOME", temp.path())
            .env_remove("WAYLAND_DISPLAY")
            .arg("--clear-session"),
    )
    .success()
    .stdout_contains("Session file:")
    .stdout_contains("No session file present");
}

#[test]
fn session_info_reports_saved_snapshot() {
    let temp = TempDir::new().unwrap();
    let session_dir = temp.path().join("sessions");
    write_session_config(&temp, &session_dir);

    let display = "test-session";
    let original_config = std::env::var_os("XDG_CONFIG_HOME");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", temp.path());
    }
    let original_display = std::env::var_os("WAYLAND_DISPLAY");
    unsafe {
        std::env::set_var("WAYLAND_DISPLAY", display);
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
        Some(value) => unsafe { std::env::set_var("XDG_CONFIG_HOME", value) },
        None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
    }

    match original_display {
        Some(value) => unsafe { std::env::set_var("WAYLAND_DISPLAY", value) },
        None => unsafe { std::env::remove_var("WAYLAND_DISPLAY") },
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
            .env("XDG_CONFIG_HOME", temp.path())
            .env("WAYLAND_DISPLAY", display)
            .arg("--session-info"),
    )
    .success()
    .stdout_contains("Per-output persistence: true")
    .stdout_contains("Session file       :")
    .stdout_contains("Output identity: DP_1")
    .stdout_contains("transparent 1")
    .stdout_contains("Tool state stored: false");
}
