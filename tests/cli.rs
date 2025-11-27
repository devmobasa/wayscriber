use assert_cmd::{Command, cargo::cargo_bin_cmd};
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn write_session_config(temp: &TempDir, custom_dir: &std::path::Path) {
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
    cargo_bin_cmd!("wayscriber")
}

#[test]
fn wayscriber_help_prints_usage() {
    wayscriber_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Screen annotation tool for Wayland compositors",
        ));
}

#[test]
fn active_mode_requires_wayland_env() {
    wayscriber_cmd()
        .env_remove("WAYLAND_DISPLAY")
        .arg("--active")
        .assert()
        .failure()
        .stderr(predicate::str::contains("WAYLAND_DISPLAY not set"));
}

#[test]
fn session_clear_command_succeeds_without_files() {
    let temp = TempDir::new().unwrap();
    let session_dir = temp.path().join("sessions");
    write_session_config(&temp, &session_dir);

    wayscriber_cmd()
        .env("XDG_CONFIG_HOME", temp.path())
        .env_remove("WAYLAND_DISPLAY")
        .arg("--clear-session")
        .assert()
        .success()
        .stdout(predicate::str::contains("Session file:"))
        .stdout(predicate::str::contains("No session file present"));
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
        active_mode: wayscriber::input::BoardMode::Transparent,
        transparent: Some(frame),
        whiteboard: None,
        blackboard: None,
        tool_state: None,
    };

    wayscriber::session::save_snapshot(&snapshot, &options).unwrap();

    wayscriber_cmd()
        .env("XDG_CONFIG_HOME", temp.path())
        .env("WAYLAND_DISPLAY", display)
        .arg("--session-info")
        .assert()
        .success()
        .stdout(predicate::str::contains("Per-output persistence: true"))
        .stdout(predicate::str::contains("Session file       :"))
        .stdout(predicate::str::contains("Output identity: DP_1"))
        .stdout(predicate::str::contains("transparent 1"))
        .stdout(predicate::str::contains("Tool state stored: false"));
}
