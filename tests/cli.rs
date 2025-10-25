use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn wayscriber_cmd() -> Command {
    Command::cargo_bin("wayscriber").expect("binary exists")
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
fn dry_run_requires_migrate_flag() {
    wayscriber_cmd()
        .arg("--dry-run")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "required arguments were not provided",
        ));
}

#[test]
fn migrate_dry_run_uses_temp_config_home() {
    let temp = TempDir::new().unwrap();
    let hypr_dir = temp.path().join("hyprmarker");
    std::fs::create_dir_all(&hypr_dir).unwrap();
    std::fs::write(hypr_dir.join("config.toml"), "legacy = true").unwrap();

    wayscriber_cmd()
        .env_remove("WAYLAND_DISPLAY")
        .env("XDG_CONFIG_HOME", temp.path())
        .args(["--migrate-config", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry-run: would copy 1 file"));
}
