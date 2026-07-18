use std::ffi::OsStr;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::time::{Duration, Instant};

use super::*;

fn release_test_provider(
    release_path: &std::path::Path,
    proof_path: &std::path::Path,
    provider_pid: i32,
) {
    std::fs::write(release_path, b"release").unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if std::fs::read(proof_path).is_ok_and(|bytes| bytes == b"survived") {
            return;
        }
        if Instant::now() >= deadline {
            // SAFETY: cleanup keeps a failing regression test from leaking its provider.
            unsafe {
                libc::kill(provider_pid, libc::SIGKILL);
            }
            panic!("successful provider could not act after normal broker shutdown");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn configurator_manifest_preserves_arbitrary_explicit_override_name() {
    let _guard = crate::test_env::lock();
    let variable = crate::env_vars::CONFIGURATOR_ENV;
    let previous = std::env::var_os(variable);
    let temp = crate::test_temp::tempdir().unwrap();
    let configured = temp.path().join("open-wayscriber-settings");
    std::fs::write(&configured, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = std::fs::metadata(&configured).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o700);
    std::fs::set_permissions(&configured, permissions).unwrap();
    // SAFETY: access to the process environment is serialized by test_env.
    unsafe { std::env::set_var(variable, &configured) };

    let program = super::wire::OsWire::from_os(configured.as_os_str()).unwrap();
    let result = super::manifest::validate(HelperKind::Configurator, &program, &[], &[], &[]);
    let broker_result = (|| -> anyhow::Result<()> {
        let guard = start_for_runtime()?;
        guard.broker().spawn(
            HelperKind::Configurator,
            HelperLifetime::DetachedAfterExec,
            configured.as_os_str(),
            std::iter::empty::<&OsStr>(),
            Vec::new(),
        )?;
        Ok(())
    })();
    let unexpected = super::wire::OsWire::from_os(OsStr::new("/tmp/unrelated-program")).unwrap();
    let unexpected_result =
        super::manifest::validate(HelperKind::Configurator, &unexpected, &[], &[], &[]);

    if let Some(previous) = previous {
        // SAFETY: access to the process environment is serialized by test_env.
        unsafe { std::env::set_var(variable, previous) };
    } else {
        // SAFETY: access to the process environment is serialized by test_env.
        unsafe { std::env::remove_var(variable) };
    }
    result.unwrap();
    broker_result.unwrap();
    assert!(unexpected_result.is_err());
}

#[test]
fn prelock_broker_runs_bounded_helpers_and_owns_reaping() {
    let guard = start_for_runtime().unwrap();
    let output = guard
        .broker()
        .run(
            HelperKind::TestSleep,
            OsStr::new("sleep"),
            [OsStr::new("0")],
            Vec::new(),
            Duration::from_secs(1),
            1024,
        )
        .unwrap();
    assert_eq!(output.status, 0);
    assert!(!output.timed_out);

    let child = guard
        .broker()
        .spawn(
            HelperKind::TestSleep,
            HelperLifetime::OwnedChild,
            OsStr::new("sleep"),
            [OsStr::new("30")],
            Vec::new(),
        )
        .unwrap();
    assert!(child.try_wait().unwrap().is_none());
    child.signal(libc::SIGTERM).unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    while child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn process_group_guard_cleans_up_before_ownership_transfer() {
    let mut command = std::process::Command::new("sleep");
    command.arg("30").process_group(0);
    let child = super::execution::ProcessGroupChild::new(command.spawn().unwrap());
    let pid = i32::try_from(child.id()).unwrap();

    drop(child);

    // SAFETY: signal zero only probes the test-owned helper PID after guard cleanup.
    assert_ne!(unsafe { libc::kill(pid, 0) }, 0);
}

#[test]
fn broker_rejects_cross_kind_programs_and_enforces_timeout() {
    let guard = start_for_runtime().unwrap();
    assert!(
        guard
            .broker()
            .run(
                HelperKind::Grim,
                OsStr::new("sleep"),
                [OsStr::new("0")],
                Vec::new(),
                Duration::from_secs(1),
                1024,
            )
            .is_err()
    );
    let timed = guard
        .broker()
        .run(
            HelperKind::TestSleep,
            OsStr::new("sleep"),
            [OsStr::new("30")],
            Vec::new(),
            Duration::from_millis(20),
            1024,
        )
        .unwrap();
    assert!(timed.timed_out);
}

#[test]
fn broker_transfers_large_input_and_output_through_sealed_memfds() {
    let guard = start_for_runtime().unwrap();
    let input = (0..(128 * 1024))
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let output = guard
        .broker()
        .run(
            HelperKind::TestCat,
            OsStr::new("cat"),
            std::iter::empty::<&OsStr>(),
            input.clone(),
            Duration::from_secs(2),
            input.len(),
        )
        .unwrap();
    assert_eq!(output.status, 0);
    assert!(!output.timed_out);
    assert_eq!(output.stdout, input);
    assert!(output.stderr.is_empty());
}

#[test]
fn broker_transfers_capture_output_beyond_the_legacy_limit() {
    const CAPTURE_BYTES: usize = 16 * 1024 * 1024 + 1;
    let guard = start_for_runtime().unwrap();
    let output = guard
        .broker()
        .run(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [OsStr::new("-c"), OsStr::new("head -c 16777217 /dev/zero")],
            Vec::new(),
            Duration::from_secs(5),
            CAPTURE_BYTES,
        )
        .unwrap();
    assert_eq!(output.status, 0);
    assert_eq!(output.stdout.len(), CAPTURE_BYTES);
}

#[test]
fn broker_rejects_output_that_exceeds_the_requested_cap() {
    let guard = start_for_runtime().unwrap();
    let result = guard.broker().run(
        HelperKind::TestShell,
        OsStr::new("sh"),
        [OsStr::new("-c"), OsStr::new("printf 12345")],
        Vec::new(),
        Duration::from_secs(1),
        4,
    );
    assert!(result.is_err(), "broker returned silently truncated output");
}

#[test]
fn broker_prefix_read_returns_the_requested_prefix_without_weakening_strict_runs() {
    let guard = start_for_runtime().unwrap();
    let output = guard
        .broker()
        .run_prefix(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [OsStr::new("-c"), OsStr::new("printf 123456789")],
            Vec::new(),
            Duration::from_secs(1),
            5,
        )
        .unwrap();

    assert_eq!(output.stdout, b"12345");
    assert!(output.stdout_limit_reached);
    assert!(!output.timed_out);
    assert!(
        guard
            .broker()
            .run_prefix(
                HelperKind::TestCat,
                OsStr::new("cat"),
                std::iter::empty::<&OsStr>(),
                Vec::new(),
                Duration::from_secs(1),
                5,
            )
            .is_err(),
        "prefix output must stay restricted to wl-paste"
    );
}

#[test]
fn broker_guard_preempts_an_active_operation_and_kills_its_group() {
    let guard = start_for_runtime().unwrap();
    let broker = guard.broker().clone();
    let temp = crate::test_temp::tempdir().unwrap();
    let pid_path = temp.path().join("helper.pid");
    let run = std::thread::spawn({
        let pid_path = pid_path.clone();
        move || {
            broker.run(
                HelperKind::TestShell,
                OsStr::new("sh"),
                [
                    OsStr::new("-c"),
                    OsStr::new("echo $$ > \"$1\"; sleep 30"),
                    OsStr::new("sh"),
                    pid_path.as_os_str(),
                ],
                Vec::new(),
                Duration::from_secs(2),
                1024,
            )
        }
    });
    let deadline = Instant::now() + Duration::from_secs(1);
    while !pid_path.exists() {
        assert!(Instant::now() < deadline, "helper did not start");
        std::thread::sleep(Duration::from_millis(5));
    }
    let helper_pid = std::fs::read_to_string(&pid_path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();

    let started = Instant::now();
    drop(guard);
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "broker shutdown waited for the active operation"
    );
    assert!(run.join().unwrap().is_err());
    // SAFETY: signal zero only probes the test-owned helper PID.
    assert_ne!(unsafe { libc::kill(helper_pid, 0) }, 0);
}

#[test]
fn owned_child_inherits_daemon_pidfd_without_leaking_broker_copy() {
    let guard = start_for_runtime().unwrap();
    let watchdog = crate::daemon::protocol_v2::open_daemon_watchdog().unwrap();
    let child = guard
        .broker()
        .spawn_with_watchdog(
            HelperKind::TestSleep,
            HelperLifetime::OwnedChild,
            OsStr::new("sleep"),
            [OsStr::new("30")],
            Vec::new(),
            watchdog.as_raw_fd(),
        )
        .unwrap();
    let inherited_pidfd = std::fs::read_dir(format!("/proc/{}/fd", child.id()))
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|entry| std::fs::read_link(entry.path()).ok())
        .any(|target| target.as_os_str() == "anon_inode:[pidfd]");
    assert!(inherited_pidfd, "owned child should retain daemon pidfd");
    child.kill_wait().unwrap();
}

#[test]
fn operation_bound_run_terminates_descendants_that_retain_pipes() {
    let guard = start_for_runtime().unwrap();
    let started = Instant::now();
    let output = guard
        .broker()
        .run(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [OsStr::new("-c"), OsStr::new("sleep 30 & echo $!")],
            Vec::new(),
            Duration::from_secs(2),
            1024,
        )
        .unwrap();
    assert_eq!(output.status, 0);
    assert!(!output.timed_out);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(
        std::str::from_utf8(&output.stdout)
            .unwrap()
            .trim()
            .parse::<u32>()
            .is_ok()
    );
}

#[test]
fn normal_broker_shutdown_releases_successful_provider_descendant() {
    let guard = start_for_runtime().unwrap();
    let temp = crate::test_temp::tempdir().unwrap();
    let release_path = temp.path().join("release-provider");
    let proof_path = temp.path().join("provider-survived");
    let pid_path = temp.path().join("provider.pid");
    let output = guard
        .broker()
        .publish(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [
                OsStr::new("-c"),
                OsStr::new(
                    "(while [ ! -e \"$1\" ]; do sleep 0.01; done; printf survived > \"$2\") & echo $! > \"$3\"",
                ),
                OsStr::new("sh"),
                release_path.as_os_str(),
                proof_path.as_os_str(),
                pid_path.as_os_str(),
            ],
            Vec::new(),
            Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(output.status, 0);
    assert!(!output.timed_out);
    let provider_pid = std::fs::read_to_string(pid_path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    // SAFETY: signal zero only checks the test-owned provider.
    assert_eq!(unsafe { libc::kill(provider_pid, 0) }, 0);

    drop(guard);

    release_test_provider(&release_path, &proof_path, provider_pid);
}

#[test]
fn shutdown_channel_peer_loss_kills_retained_provider() {
    let guard = start_for_runtime().unwrap();
    let temp = crate::test_temp::tempdir().unwrap();
    let pid_path = temp.path().join("provider.pid");
    let output = guard
        .broker()
        .publish(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [
                OsStr::new("-c"),
                OsStr::new("sleep 30 & echo $! > \"$1\""),
                OsStr::new("sh"),
                pid_path.as_os_str(),
            ],
            Vec::new(),
            Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(output.status, 0);
    let provider_pid = std::fs::read_to_string(pid_path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    // SAFETY: signal zero only checks the test-owned provider.
    assert_eq!(unsafe { libc::kill(provider_pid, 0) }, 0);

    // Simulate abrupt parent loss without writing the graceful-shutdown packet.
    // SAFETY: the descriptor belongs to this test's live broker guard.
    assert_eq!(
        unsafe { libc::shutdown(guard.broker().inner.shutdown.as_raw_fd(), libc::SHUT_RDWR) },
        0
    );
    drop(guard);

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        // SAFETY: signal zero only probes the recorded test provider PID.
        if unsafe { libc::kill(provider_pid, 0) } != 0 {
            break;
        }
        if Instant::now() >= deadline {
            // SAFETY: cleanup keeps a failing regression test from leaking its provider.
            unsafe {
                libc::kill(provider_pid, libc::SIGKILL);
            }
            panic!("provider survived abnormal broker channel loss");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn retained_publication_replacement_disposes_the_previous_provider() {
    let guard = start_for_runtime().unwrap();
    let temp = crate::test_temp::tempdir().unwrap();
    let first_pid_path = temp.path().join("first-provider.pid");
    let second_pid_path = temp.path().join("second-provider.pid");
    let second_release_path = temp.path().join("release-second-provider");
    let second_proof_path = temp.path().join("second-provider-survived");
    let first = guard
        .broker()
        .publish(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [
                OsStr::new("-c"),
                OsStr::new("sleep 30 & echo $! > \"$1\""),
                OsStr::new("sh"),
                first_pid_path.as_os_str(),
            ],
            Vec::new(),
            Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(first.status, 0);
    let second = guard
        .broker()
        .publish(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [
                OsStr::new("-c"),
                OsStr::new(
                    "(while [ ! -e \"$1\" ]; do sleep 0.01; done; printf survived > \"$2\") & echo $! > \"$3\"",
                ),
                OsStr::new("sh"),
                second_release_path.as_os_str(),
                second_proof_path.as_os_str(),
                second_pid_path.as_os_str(),
            ],
            Vec::new(),
            Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(second.status, 0);

    let first_pid = std::fs::read_to_string(first_pid_path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    let second_pid = std::fs::read_to_string(second_pid_path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        // SAFETY: signal zero only probes the recorded test provider PID.
        if unsafe { libc::kill(first_pid, 0) } != 0 {
            break;
        }
        if Instant::now() >= deadline {
            // SAFETY: cleanup keeps a failing regression test from leaking its helpers.
            unsafe {
                libc::kill(first_pid, libc::SIGKILL);
                libc::kill(second_pid, libc::SIGKILL);
            }
            panic!("replaced publication provider remained alive");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // SAFETY: signal zero only checks the current test-owned provider.
    assert_eq!(unsafe { libc::kill(second_pid, 0) }, 0);

    drop(guard);
    release_test_provider(&second_release_path, &second_proof_path, second_pid);
}

#[test]
fn failed_publication_replacement_preserves_the_current_provider() {
    let guard = start_for_runtime().unwrap();
    let temp = crate::test_temp::tempdir().unwrap();
    let current_pid_path = temp.path().join("current-provider.pid");
    let failed_pid_path = temp.path().join("failed-provider.pid");
    let current_release_path = temp.path().join("release-current-provider");
    let current_proof_path = temp.path().join("current-provider-survived");
    let current = guard
        .broker()
        .publish(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [
                OsStr::new("-c"),
                OsStr::new(
                    "(while [ ! -e \"$1\" ]; do sleep 0.01; done; printf survived > \"$2\") & echo $! > \"$3\"",
                ),
                OsStr::new("sh"),
                current_release_path.as_os_str(),
                current_proof_path.as_os_str(),
                current_pid_path.as_os_str(),
            ],
            Vec::new(),
            Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(current.status, 0);

    let failed = guard
        .broker()
        .publish(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [
                OsStr::new("-c"),
                OsStr::new("sleep 30 & echo $! > \"$1\"; exit 7"),
                OsStr::new("sh"),
                failed_pid_path.as_os_str(),
            ],
            Vec::new(),
            Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(failed.status, 7);

    let current_pid = std::fs::read_to_string(current_pid_path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    let failed_pid = std::fs::read_to_string(failed_pid_path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    // SAFETY: signal zero only checks the current test-owned provider.
    assert_eq!(unsafe { libc::kill(current_pid, 0) }, 0);
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        // SAFETY: signal zero only probes the failed test provider PID.
        if unsafe { libc::kill(failed_pid, 0) } != 0 {
            break;
        }
        if Instant::now() >= deadline {
            // SAFETY: cleanup keeps a failing regression test from leaking its helpers.
            unsafe {
                libc::kill(current_pid, libc::SIGKILL);
                libc::kill(failed_pid, libc::SIGKILL);
            }
            panic!("failed replacement provider survived cleanup");
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    drop(guard);
    release_test_provider(&current_release_path, &current_proof_path, current_pid);
}

#[test]
fn retained_publication_kills_failed_or_input_stalled_provider_groups() {
    let guard = start_for_runtime().unwrap();
    for (script, input) in [
        ("sleep 30 <&0 & echo $! > \"$1\"; exit 7", Vec::new()),
        (
            "sleep 30 <&0 & echo $! > \"$1\"; exit 0",
            vec![b'x'; 1024 * 1024],
        ),
    ] {
        let temp = crate::test_temp::tempdir().unwrap();
        let pid_path = temp.path().join("provider.pid");
        let result = guard.broker().publish(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [
                OsStr::new("-c"),
                OsStr::new(script),
                OsStr::new("sh"),
                pid_path.as_os_str(),
            ],
            input,
            Duration::from_millis(100),
        );
        let provider_pid = std::fs::read_to_string(pid_path)
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap();
        if script.ends_with("exit 7") {
            assert_eq!(result.unwrap().status, 7);
        } else {
            assert!(result.is_err());
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            // SAFETY: signal zero only probes the recorded test child PID.
            if unsafe { libc::kill(provider_pid, 0) } != 0 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "failed provider survived cleanup"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[test]
fn retained_publication_rejects_incomplete_input_after_successful_exit() {
    let guard = start_for_runtime().unwrap();
    let result = guard.broker().publish(
        HelperKind::TestShell,
        OsStr::new("sh"),
        [OsStr::new("-c"), OsStr::new("exit 0")],
        vec![b'x'; 1024 * 1024],
        Duration::from_secs(1),
    );

    assert!(result.is_err(), "incomplete publication input was accepted");
}

#[test]
fn broker_shutdown_preempts_retained_publication_stdin_writer() {
    let guard = start_for_runtime().unwrap();
    let broker = guard.broker().clone();
    let temp = crate::test_temp::tempdir().unwrap();
    let current_pid_path = temp.path().join("current-provider.pid");
    let current_release_path = temp.path().join("release-current-provider");
    let current_proof_path = temp.path().join("current-provider-survived");
    let pid_path = temp.path().join("provider.pid");
    let current = guard
        .broker()
        .publish(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [
                OsStr::new("-c"),
                OsStr::new(
                    "(while [ ! -e \"$1\" ]; do sleep 0.01; done; printf survived > \"$2\") & echo $! > \"$3\"",
                ),
                OsStr::new("sh"),
                current_release_path.as_os_str(),
                current_proof_path.as_os_str(),
                current_pid_path.as_os_str(),
            ],
            Vec::new(),
            Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(current.status, 0);
    let current_pid = std::fs::read_to_string(current_pid_path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    let publication = std::thread::spawn({
        let pid_path = pid_path.clone();
        move || {
            broker.publish(
                HelperKind::TestShell,
                OsStr::new("sh"),
                [
                    OsStr::new("-c"),
                    OsStr::new("sleep 30 <&0 & echo $! > \"$1\"; exit 0"),
                    OsStr::new("sh"),
                    pid_path.as_os_str(),
                ],
                vec![b'x'; 1024 * 1024],
                Duration::from_secs(2),
            )
        }
    });
    let deadline = Instant::now() + Duration::from_secs(1);
    while !pid_path.exists() {
        assert!(
            Instant::now() < deadline,
            "publication helper did not start"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let provider_pid = std::fs::read_to_string(pid_path)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();

    let started = Instant::now();
    drop(guard);
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "broker shutdown waited for the publication deadline"
    );
    assert!(publication.join().unwrap().is_err());
    release_test_provider(&current_release_path, &current_proof_path, current_pid);
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        // SAFETY: signal zero only probes the recorded test child PID.
        if unsafe { libc::kill(provider_pid, 0) } != 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "publication provider survived broker shutdown"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn wl_copy_publication_accepts_capture_sized_input() {
    const PUBLICATION_BYTES: usize = 16 * 1024 * 1024 + 1;
    let guard = start_for_runtime().unwrap();
    let temp = crate::test_temp::tempdir().unwrap();
    let helper = temp.path().join("wl-copy");
    let count_path = temp.path().join("published-bytes");
    std::fs::write(&helper, "#!/bin/sh\nwc -c > \"$1\"\n").unwrap();
    let mut permissions = std::fs::metadata(&helper).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o700);
    std::fs::set_permissions(&helper, permissions).unwrap();

    let output = guard
        .broker()
        .publish(
            HelperKind::WlCopy,
            helper.as_os_str(),
            [count_path.as_os_str()],
            vec![b'x'; PUBLICATION_BYTES],
            Duration::from_secs(5),
        )
        .unwrap();

    assert_eq!(output.status, 0);
    assert!(!output.timed_out);
    assert_eq!(
        std::fs::read_to_string(count_path).unwrap().trim(),
        PUBLICATION_BYTES.to_string()
    );
}
