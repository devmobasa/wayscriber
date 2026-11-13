//! All integration-test launches of the application belong here.
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use tempfile::TempDir;

pub fn wayscriber_cmd(temp: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wayscriber"));
    isolate_directories(&mut command, temp);
    command
}

/// Re-executes a test alone so its in-process library calls and environment
/// changes cannot reach the runner's directories or race other tests.
pub fn in_isolated_process(test_name: &str) -> bool {
    const CHILD: &str = "WAYSCRIBER_TEST_ISOLATED_PROCESS";
    if env::var(CHILD).is_ok_and(|name| name == test_name) {
        return true;
    }

    let temp = TempDir::new().expect("isolated library test directories");
    let mut command = Command::new(env::current_exe().expect("integration test executable"));
    isolate_directories(&mut command, &temp);
    let output = command
        .args([test_name, "--exact", "--test-threads=1"])
        .env(CHILD, test_name)
        .output()
        .expect("run isolated library test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "isolated test failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("1 passed;"),
        "the isolated child must run one test:\n{stdout}"
    );
    false
}

fn isolate_directories(command: &mut Command, temp: &TempDir) {
    for (key, directory) in [
        ("HOME", "home"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_RUNTIME_DIR", "runtime"),
    ] {
        let path = temp.path().join(directory);
        fs::create_dir_all(&path).expect("create isolated application directory");
        #[cfg(unix)]
        if key == "XDG_RUNTIME_DIR" {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("make the isolated runtime directory private");
        }
        command.env(key, path);
    }

    for key in [
        "WAYLAND_DISPLAY",
        "DISPLAY",
        "HYPRLAND_INSTANCE_SIGNATURE",
        "SWAYSOCK",
    ] {
        command.env_remove(key);
    }
}
