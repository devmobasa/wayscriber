//! Exercise the broker crate without `cfg(test)`: the driver is copied beside
//! the real companion and starts it through the production clone/exec path.

use std::ffi::OsStr;
use std::fs;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use wayscriber_process_broker::{
    BROKER_COHORT, HelperKind, HelperLifetime, current, start_for_runtime,
};

const CASE_ENV: &str = "WAYSCRIBER_BROKER_EXEC_TEST_CASE";
const ROOT_ENV: &str = "WAYSCRIBER_BROKER_EXEC_TEST_ROOT";
const COMPANION: &str = env!("CARGO_BIN_EXE_wayscriber-broker");

fn child_pids() -> Vec<i32> {
    // Raw-clone children belong to the test harness worker that called start.
    let tid = unsafe { libc::syscall(libc::SYS_gettid) };
    fs::read_to_string(format!("/proc/self/task/{tid}/children"))
        .expect("read owned children")
        .split_whitespace()
        .map(|pid| pid.parse().expect("child PID"))
        .collect()
}

fn expect_start_failure(fragment: &str) {
    let error = start_for_runtime().expect_err("broker start must fail");
    assert!(format!("{error:#}").contains(fragment), "{error:#}");
    assert!(child_pids().is_empty(), "failed start left a broker child");
}

fn executable(path: &Path, contents: &[u8]) {
    fs::write(path, contents).expect("write helper");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("mark helper executable");
}

fn run_production_case(case: &str, root: &Path) {
    match case {
        "missing" => expect_start_failure("missing broker companion"),
        "symlink" | "writable" | "nonexec" => {
            expect_start_failure("broker companion must be an executable")
        }
        "wrong" => expect_start_failure("process broker handshake failed"),
        "mismatch" => expect_start_failure("broker companion mismatch"),
        "success" => {
            let sentinel = root.join("sentinel");
            let file = fs::File::create(&sentinel).expect("create sentinel");
            // SAFETY: fcntl duplicates an owned file descriptor for this process.
            let descriptor = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD, 30) };
            assert!(descriptor >= 30, "stage inheritable descriptor");
            // SAFETY: the successful fcntl result is a fresh owned descriptor.
            let _inherited = unsafe { OwnedFd::from_raw_fd(descriptor) };

            let guard = start_for_runtime().expect("real broker handshake");
            let broker = current().expect("runtime broker is active");
            let [broker_pid] = child_pids().try_into().expect("one owned broker child");
            let broker_fds = fs::read_dir(format!("/proc/{broker_pid}/fd"))
                .expect("inspect broker descriptors")
                .map(|entry| fs::read_link(entry.expect("descriptor").path()))
                .collect::<Result<Vec<_>, _>>()
                .expect("read broker descriptors");
            assert!(
                !broker_fds.contains(&sentinel),
                "inherited descriptor leaked into broker"
            );

            let app = std::env::current_exe().expect("staged driver path");
            let about = broker
                .run(
                    HelperKind::About,
                    app.as_os_str(),
                    [OsStr::new("--help")],
                    Vec::new(),
                    Duration::from_secs(2),
                    64 * 1024,
                )
                .expect("parent executable admitted");
            assert_eq!(about.status, 0);

            let wrong_app = root.join("other/wayscriber");
            fs::create_dir(root.join("other")).expect("create alternate executable directory");
            fs::copy(&app, &wrong_app).expect("copy alternate executable");
            let error = broker
                .run(
                    HelperKind::About,
                    wrong_app.as_os_str(),
                    [OsStr::new("--help")],
                    Vec::new(),
                    Duration::from_secs(2),
                    64 * 1024,
                )
                .expect_err("different executable must be denied");
            assert!(format!("{error:#}").contains("not allowed"), "{error:#}");

            let helper = root.join("grim");
            executable(&helper, b"#!/bin/sh\nexit 17\n");
            let failure = broker
                .run(
                    HelperKind::CapabilityProbe,
                    helper.as_os_str(),
                    std::iter::empty::<&OsStr>(),
                    Vec::new(),
                    Duration::from_secs(2),
                    1024,
                )
                .expect("failed helper returns status");
            assert_eq!(failure.status, 17);

            executable(&helper, b"#!/bin/sh\nexec /bin/sleep 30\n");
            let owned = broker
                .spawn(
                    HelperKind::CapabilityProbe,
                    HelperLifetime::OwnedChild,
                    helper.as_os_str(),
                    std::iter::empty::<&OsStr>(),
                    Vec::new(),
                )
                .expect("spawn owned helper");
            let helper_pid = owned.id();
            assert!(Path::new(&format!("/proc/{helper_pid}")).exists());

            drop(owned);
            drop(broker);
            drop(guard);
            assert!(child_pids().is_empty(), "broker was not reaped on shutdown");
            assert!(current().is_err(), "runtime broker remained published");
            assert!(
                !Path::new(&format!("/proc/{helper_pid}")).exists(),
                "owned helper survived broker shutdown"
            );
        }
        _ => panic!("unknown production broker test case: {case}"),
    }
}

#[test]
fn production_exec_child() {
    let Ok(case) = std::env::var(CASE_ENV) else {
        return;
    };
    let root = std::env::var(ROOT_ENV).expect("staged test root");

    run_production_case(&case, Path::new(&root));
}

#[test]
fn production_exec_cases() {
    for case in [
        "success", "missing", "symlink", "writable", "nonexec", "wrong", "mismatch",
    ] {
        let staged = tempfile::tempdir().expect("stage companion cohort");
        let app = staged.path().join("wayscriber");
        let companion = staged.path().join("wayscriber-broker");
        fs::copy(std::env::current_exe().expect("test driver path"), &app)
            .expect("copy test driver into cohort");
        fs::copy(COMPANION, &companion).expect("copy real broker into cohort");

        match case {
            "missing" => fs::remove_file(&companion).expect("remove companion"),
            "symlink" => {
                fs::remove_file(&companion).expect("remove companion");
                symlink(COMPANION, &companion).expect("stage symlink companion");
            }
            "writable" => fs::set_permissions(&companion, fs::Permissions::from_mode(0o775))
                .expect("make companion writable"),
            "nonexec" => fs::set_permissions(&companion, fs::Permissions::from_mode(0o644))
                .expect("remove companion executable bit"),
            "wrong" => executable(&companion, b"#!/bin/sh\nexit 0\n"),
            "mismatch" => {
                let mut bytes = fs::read(&companion).expect("read companion");
                let cohort = BROKER_COHORT.as_bytes();
                let mut changed = 0;
                for offset in 0..=bytes.len() - cohort.len() {
                    if &bytes[offset..offset + cohort.len()] == cohort {
                        bytes[offset] = if bytes[offset] == b'0' { b'1' } else { b'0' };
                        changed += 1;
                    }
                }
                assert!(changed > 0, "broker executable lacks its cohort marker");
                fs::write(&companion, bytes).expect("stage mismatched companion");
            }
            "success" => {}
            _ => unreachable!(),
        }

        let mut child = Command::new(&app)
            .args(["--exact", "production_exec_child", "--nocapture"])
            .env(CASE_ENV, case)
            .env(ROOT_ENV, staged.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("launch staged production driver");
        let deadline = Instant::now() + Duration::from_secs(10);
        while child.try_wait().expect("poll staged driver").is_none() {
            if Instant::now() >= deadline {
                child.kill().expect("stop stalled driver");
                panic!("production broker case {case} timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = child
            .wait_with_output()
            .expect("collect staged driver result");
        assert!(
            output.status.success(),
            "case {case} failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
