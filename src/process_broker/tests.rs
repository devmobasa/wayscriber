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
    std::fs::write(release_path, b"release")
        .expect("provider fixture publishes its release marker");
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if std::fs::read(proof_path).is_ok_and(|bytes| bytes == b"survived") {
            return;
        }
        let completed_in_time = Instant::now() < deadline;
        if !completed_in_time {
            // SAFETY: cleanup keeps a failing regression test from leaking its provider.
            unsafe {
                libc::kill(provider_pid, libc::SIGKILL);
            }
        }
        assert!(
            completed_in_time,
            "successful provider could not act after normal broker shutdown"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn configurator_manifest_preserves_arbitrary_explicit_override_name() {
    let configured = OsStr::new("/tmp/open-wayscriber-settings");

    assert!(super::manifest::configurator_program_allowed(
        configured,
        "open-wayscriber-settings",
        Some(configured),
    ));
    assert!(!super::manifest::configurator_program_allowed(
        OsStr::new("/tmp/unrelated-program"),
        "unrelated-program",
        Some(configured),
    ));
}

#[test]
fn update_fetcher_manifest_allows_only_curl_and_wget() {
    for program in ["/usr/bin/curl", "/usr/bin/wget"] {
        let program = super::wire::OsWire::from_os(OsStr::new(program))
            .expect("allowed update-fetcher path is valid broker wire input");
        super::manifest::validate(HelperKind::UpdateFetcher, &program, &[], &[], &[])
            .expect("manifest accepts curl and wget update fetchers");
    }

    let unrelated = super::wire::OsWire::from_os(OsStr::new("/usr/bin/sh"))
        .expect("unrelated update-fetcher path is valid broker wire input");
    assert!(
        super::manifest::validate(HelperKind::UpdateFetcher, &unrelated, &[], &[], &[]).is_err()
    );
}

#[test]
fn prelock_broker_runs_bounded_helpers_and_owns_reaping() {
    let guard = start_for_runtime().expect("bounded-helper fixture starts its broker owner");
    let output = guard
        .handle()
        .run(
            HelperKind::TestSleep,
            OsStr::new("sleep"),
            [OsStr::new("0")],
            Vec::new(),
            Duration::from_secs(1),
            1024,
        )
        .expect("broker runs the zero-duration bounded helper");
    assert_eq!(output.status, 0);
    assert!(!output.timed_out);

    let child = guard
        .handle()
        .spawn(
            HelperKind::TestSleep,
            HelperLifetime::OwnedChild,
            OsStr::new("sleep"),
            [OsStr::new("30")],
            Vec::new(),
        )
        .expect("broker spawns the owned sleep helper");
    assert!(
        child
            .try_wait()
            .expect("broker reports the newly spawned helper state")
            .is_none()
    );
    child
        .signal(libc::SIGTERM)
        .expect("broker delivers SIGTERM to its owned helper");
    let deadline = Instant::now() + Duration::from_secs(1);
    while child
        .try_wait()
        .expect("broker reports helper exit during bounded reap polling")
        .is_none()
    {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn two_explicit_broker_owners_are_independent() {
    let first_owner =
        start_for_runtime().expect("first broker fixture starts an isolated owner actor");
    let first_handle = first_owner.handle();
    let second_owner =
        start_for_runtime().expect("second broker fixture starts an isolated owner actor");
    let second_handle = second_owner.handle();

    let first_output = first_handle
        .run(
            HelperKind::TestSleep,
            OsStr::new("sleep"),
            [OsStr::new("0")],
            Vec::new(),
            Duration::from_secs(1),
            1024,
        )
        .expect("first isolated broker runs its fixture helper");
    let second_output = second_handle
        .run(
            HelperKind::TestSleep,
            OsStr::new("sleep"),
            [OsStr::new("0")],
            Vec::new(),
            Duration::from_secs(1),
            1024,
        )
        .expect("second isolated broker runs its fixture helper");
    assert_eq!(first_output.status, 0);
    assert_eq!(second_output.status, 0);

    drop(first_owner);
    assert!(
        first_handle
            .run(
                HelperKind::TestSleep,
                OsStr::new("sleep"),
                [OsStr::new("0")],
                Vec::new(),
                Duration::from_secs(1),
                1024,
            )
            .is_err()
    );
    let surviving_output = second_handle
        .run(
            HelperKind::TestSleep,
            OsStr::new("sleep"),
            [OsStr::new("0")],
            Vec::new(),
            Duration::from_secs(1),
            1024,
        )
        .expect("dropping the first owner leaves the second broker usable");
    assert_eq!(surviving_output.status, 0);
}

#[test]
fn broker_rejects_delayed_publication_after_its_wire_admission_deadline() {
    let (owner, admission) = super::client::start_for_runtime_with_admission_gate()
        .expect("delayed-publication fixture starts a broker with an admission gate");
    let temp = crate::test_temp::tempdir()
        .expect("delayed-publication fixture creates an isolated helper directory");
    let helper = temp.path().join("wl-copy");
    let side_effect = temp.path().join("provider-started");
    std::fs::write(
        &helper,
        "#!/bin/sh\nprintf started > \"$1\"\ncat >/dev/null\n",
    )
    .expect("delayed-publication fixture writes its provider probe");
    let mut permissions = std::fs::metadata(&helper)
        .expect("delayed-publication fixture reads its provider permissions")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o700);
    std::fs::set_permissions(&helper, permissions)
        .expect("delayed-publication fixture makes its provider probe executable");

    let handle = owner.handle();
    let (deadline_sent, deadline_received) = std::sync::mpsc::sync_channel(1);
    let request = std::thread::spawn(move || {
        let deadline = super::wire::admission_deadline_after(Duration::from_secs(1))?;
        deadline_sent
            .send(deadline)
            .map_err(|_| anyhow::anyhow!("deadline observer disconnected"))?;
        handle.request_with_admission_deadline_for_test(
            super::wire::BrokerOperation::Publish {
                kind: HelperKind::WlCopy,
                program: super::wire::OsWire::from_os(helper.as_os_str())?,
                arguments: vec![super::wire::OsWire::from_os(side_effect.as_os_str())?],
                environment: Vec::new(),
                input: super::wire::BlobWire::Inline {
                    bytes: b"clipboard".to_vec(),
                },
                timeout_ms: 1_000,
            },
            deadline,
        )
    });
    let deadline = deadline_received
        .recv_timeout(Duration::from_secs(1))
        .expect("delayed-publication fixture receives the absolute wire deadline");
    admission
        .wait_until_paused(Duration::from_secs(2))
        .expect("delayed-publication fixture observes the broker before admission");
    while super::wire::monotonic_now_ns()
        .expect("delayed-publication fixture reads Linux monotonic time")
        < deadline
    {
        std::thread::yield_now();
    }
    admission
        .release()
        .expect("delayed-publication fixture releases the stale broker request");

    let error = request
        .join()
        .expect("delayed-publication request thread returns its typed result")
        .expect_err("broker rejects publication that expires at its admission gate");
    assert!(error.to_string().contains("admission deadline expired"));
    assert!(
        !temp.path().join("provider-started").exists(),
        "expired publication started its provider helper"
    );
}

#[test]
fn process_group_guard_cleans_up_before_ownership_transfer() {
    let mut command = std::process::Command::new("sleep");
    command.arg("30").process_group(0);
    let child = super::execution::OwnedProcess::process_group(
        command
            .spawn()
            .expect("process-group fixture starts its owned sleep helper"),
    );
    let pid = i32::try_from(child.id()).expect("fixture helper PID fits libc pid_t");

    drop(child);

    // SAFETY: signal zero only probes the test-owned helper PID after guard cleanup.
    assert_ne!(unsafe { libc::kill(pid, 0) }, 0);
}

#[test]
fn initial_detach_child_remains_eligible_to_create_a_session() {
    let temp = crate::test_temp::tempdir()
        .expect("initial-detach fixture creates an isolated executable directory");
    let helper = temp.path().join("wayscriber-detach-probe");
    let proof = temp.path().join("detach-state");
    std::fs::write(
        &helper,
        r#"#!/bin/sh
read -r pid comm state ppid pgrp rest < "/proc/$$/stat"
if [ "$pid" = "$pgrp" ]; then
    printf process-group-leader > "$1"
else
    printf session-eligible > "$1"
fi
"#,
    )
    .expect("initial-detach fixture writes its session eligibility probe");
    let mut permissions = std::fs::metadata(&helper)
        .expect("initial-detach fixture reads its probe permissions")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o700);
    std::fs::set_permissions(&helper, permissions)
        .expect("initial-detach fixture makes its probe executable");

    let guard = start_for_runtime().expect("initial-detach fixture starts its broker owner");
    let _child = guard
        .handle()
        .spawn(
            HelperKind::InitialDetach,
            HelperLifetime::DetachedAfterExec,
            helper.as_os_str(),
            [proof.as_os_str()],
            Vec::new(),
        )
        .expect("broker starts the initial-detach session eligibility probe");

    let deadline = Instant::now() + Duration::from_secs(1);
    let observed = loop {
        if let Ok(value) = std::fs::read_to_string(&proof)
            && matches!(value.as_str(), "session-eligible" | "process-group-leader")
        {
            break value;
        }
        assert!(Instant::now() < deadline, "detach probe did not complete");
        std::thread::yield_now();
    };
    assert_eq!(observed, "session-eligible");
}

#[test]
fn broker_rejects_cross_kind_programs_and_enforces_timeout() {
    let guard = start_for_runtime().expect("manifest-timeout fixture starts its broker owner");
    assert!(
        guard
            .handle()
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
        .handle()
        .run(
            HelperKind::TestSleep,
            OsStr::new("sleep"),
            [OsStr::new("30")],
            Vec::new(),
            Duration::from_millis(20),
            1024,
        )
        .expect("broker returns the bounded helper timeout outcome");
    assert!(timed.timed_out);
}

#[test]
fn broker_transfers_large_input_and_output_through_sealed_memfds() {
    let guard = start_for_runtime().expect("memfd-transfer fixture starts its broker owner");
    let input = (0..(128 * 1024))
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let output = guard
        .handle()
        .run(
            HelperKind::TestCat,
            OsStr::new("cat"),
            std::iter::empty::<&OsStr>(),
            input.clone(),
            Duration::from_secs(2),
            input.len(),
        )
        .expect("broker round-trips large input and output through sealed memfds");
    assert_eq!(output.status, 0);
    assert!(!output.timed_out);
    assert_eq!(output.stdout, input);
    assert!(output.stderr.is_empty());
}

#[test]
fn broker_transfers_capture_output_beyond_the_legacy_limit() {
    const CAPTURE_BYTES: usize = 16 * 1024 * 1024 + 1;
    let guard = start_for_runtime().expect("capture-output fixture starts its broker owner");
    let output = guard
        .handle()
        .run(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [OsStr::new("-c"), OsStr::new("head -c 16777217 /dev/zero")],
            Vec::new(),
            Duration::from_secs(5),
            CAPTURE_BYTES,
        )
        .expect("broker returns output beyond the legacy capture limit");
    assert_eq!(output.status, 0);
    assert_eq!(output.stdout.len(), CAPTURE_BYTES);
}

#[test]
fn broker_rejects_output_that_exceeds_the_requested_cap() {
    let guard = start_for_runtime().expect("output-cap fixture starts its broker owner");
    let result = guard.handle().run(
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
fn broker_stops_an_endless_stream_when_it_reaches_the_output_cap() {
    let guard = start_for_runtime().expect("endless-output fixture starts its broker owner");
    let started = Instant::now();
    let result = guard.handle().run(
        HelperKind::TestShell,
        OsStr::new("sh"),
        [
            OsStr::new("-c"),
            OsStr::new("while :; do printf 1234567890; done"),
        ],
        Vec::new(),
        Duration::from_secs(5),
        1024,
    );

    assert!(result.is_err(), "broker accepted an endless response");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "broker waited for the operation timeout instead of stopping at the cap"
    );
}

#[test]
fn broker_prefix_read_returns_the_requested_prefix_without_weakening_strict_runs() {
    let guard = start_for_runtime().expect("prefix-output fixture starts its broker owner");
    let output = guard
        .handle()
        .run_prefix(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [OsStr::new("-c"), OsStr::new("printf 123456789")],
            Vec::new(),
            Duration::from_secs(1),
            5,
        )
        .expect("broker returns the requested bounded stdout prefix");

    assert_eq!(output.stdout, b"12345");
    assert!(output.stdout_limit_reached);
    assert!(!output.timed_out);
    assert!(
        guard
            .handle()
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
    let guard = start_for_runtime().expect("active-operation fixture starts its broker owner");
    let broker = guard.handle();
    let temp = crate::test_temp::tempdir()
        .expect("active-operation fixture creates an isolated PID directory");
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
        .expect("active-operation helper writes its process-group PID")
        .trim()
        .parse::<i32>()
        .expect("active-operation helper PID is valid libc pid_t input");

    let started = Instant::now();
    drop(guard);
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "broker shutdown waited for the active operation"
    );
    assert!(
        run.join()
            .expect("active-operation request thread returns after owner teardown")
            .is_err()
    );
    // SAFETY: signal zero only probes the test-owned helper PID.
    assert_ne!(unsafe { libc::kill(helper_pid, 0) }, 0);
}

#[test]
fn owned_child_inherits_daemon_pidfd_without_leaking_broker_copy() {
    let guard = start_for_runtime().expect("watchdog fixture starts its broker owner");
    let watchdog = crate::daemon::protocol_v2::open_daemon_watchdog()
        .expect("watchdog fixture opens its daemon pidfd");
    let child = guard
        .handle()
        .spawn_with_watchdog(
            HelperKind::TestSleep,
            HelperLifetime::OwnedChild,
            OsStr::new("sleep"),
            [OsStr::new("30")],
            Vec::new(),
            watchdog.as_raw_fd(),
        )
        .expect("broker spawns the watchdog-bearing owned helper");
    let inherited_pidfd = std::fs::read_dir(format!("/proc/{}/fd", child.id()))
        .expect("watchdog fixture enumerates the owned helper descriptors")
        .filter_map(Result::ok)
        .filter_map(|entry| std::fs::read_link(entry.path()).ok())
        .any(|target| target.as_os_str() == "anon_inode:[pidfd]");
    assert!(inherited_pidfd, "owned child should retain daemon pidfd");
    child
        .kill_wait()
        .expect("broker kills and reaps the watchdog-bearing helper");
}

#[test]
fn operation_bound_run_terminates_descendants_that_retain_pipes() {
    let guard = start_for_runtime().expect("descendant-pipe fixture starts its broker owner");
    let started = Instant::now();
    let output = guard
        .handle()
        .run(
            HelperKind::TestShell,
            OsStr::new("sh"),
            [OsStr::new("-c"), OsStr::new("sleep 30 & echo $!")],
            Vec::new(),
            Duration::from_secs(2),
            1024,
        )
        .expect("operation-bound run terminates descendants retaining stdout");
    assert_eq!(output.status, 0);
    assert!(!output.timed_out);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(
        std::str::from_utf8(&output.stdout)
            .expect("descendant-pipe fixture output is valid UTF-8")
            .trim()
            .parse::<u32>()
            .is_ok()
    );
}

#[test]
fn normal_broker_shutdown_releases_successful_provider_descendant() {
    let guard = start_for_runtime().expect("normal-shutdown fixture starts its broker owner");
    let temp = crate::test_temp::tempdir()
        .expect("normal-shutdown fixture creates an isolated provider directory");
    let release_path = temp.path().join("release-provider");
    let proof_path = temp.path().join("provider-survived");
    let pid_path = temp.path().join("provider.pid");
    let output = guard
        .handle()
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
        .expect("normal-shutdown fixture publishes through its retained provider");
    assert_eq!(output.status, 0);
    assert!(!output.timed_out);
    let provider_pid = std::fs::read_to_string(pid_path)
        .expect("normal-shutdown provider writes its PID marker")
        .trim()
        .parse::<i32>()
        .expect("normal-shutdown provider PID is valid libc pid_t input");
    // SAFETY: signal zero only checks the test-owned provider.
    assert_eq!(unsafe { libc::kill(provider_pid, 0) }, 0);

    drop(guard);

    release_test_provider(&release_path, &proof_path, provider_pid);
}

#[test]
fn shutdown_channel_peer_loss_kills_retained_provider() {
    let guard = start_for_runtime().expect("peer-loss fixture starts its broker owner");
    let temp = crate::test_temp::tempdir()
        .expect("peer-loss fixture creates an isolated provider directory");
    let pid_path = temp.path().join("provider.pid");
    let output = guard
        .handle()
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
        .expect("peer-loss fixture publishes through its retained provider");
    assert_eq!(output.status, 0);
    let provider_pid = std::fs::read_to_string(pid_path)
        .expect("peer-loss provider writes its PID marker")
        .trim()
        .parse::<i32>()
        .expect("peer-loss provider PID is valid libc pid_t input");
    // SAFETY: signal zero only checks the test-owned provider.
    assert_eq!(unsafe { libc::kill(provider_pid, 0) }, 0);

    // Simulate abrupt parent loss without writing the graceful-shutdown packet.
    // SAFETY: the descriptor belongs to this test's live broker guard.
    assert!(guard.disconnect_shutdown_channel().is_ok());
    drop(guard);

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        // SAFETY: signal zero only probes the recorded test provider PID.
        if unsafe { libc::kill(provider_pid, 0) } != 0 {
            break;
        }
        let stopped_in_time = Instant::now() < deadline;
        if !stopped_in_time {
            // SAFETY: cleanup keeps a failing regression test from leaking its provider.
            unsafe {
                libc::kill(provider_pid, libc::SIGKILL);
            }
        }
        assert!(
            stopped_in_time,
            "provider survived abnormal broker channel loss"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn abandoned_actor_reply_fail_stops_broker_and_retained_provider() {
    let guard = start_for_runtime().expect("reply-ambiguity fixture starts its broker owner");
    let broker = guard.handle();
    let temp = crate::test_temp::tempdir()
        .expect("reply-ambiguity fixture creates an isolated provider directory");
    let pid_path = temp.path().join("provider.pid");
    let output = broker
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
        .expect("reply-ambiguity fixture starts its retained provider");
    assert_eq!(output.status, 0);
    let provider_pid = std::fs::read_to_string(pid_path)
        .expect("reply-ambiguity provider writes its PID marker")
        .trim()
        .parse::<i32>()
        .expect("reply-ambiguity provider PID is valid libc pid_t input");
    // SAFETY: signal zero only checks the test-owned retained provider.
    assert_eq!(unsafe { libc::kill(provider_pid, 0) }, 0);

    broker
        .abandon_ping_reply_for_test()
        .expect("reply-ambiguity fixture queues a request with no reply receiver");
    let actor_exit_started = Instant::now();
    let follow_up = broker.run(
        HelperKind::TestSleep,
        OsStr::new("sleep"),
        [OsStr::new("0")],
        Vec::new(),
        Duration::from_secs(1),
        1024,
    );
    let actor_stopped_promptly = actor_exit_started.elapsed() < Duration::from_secs(1);

    let deadline = Instant::now() + Duration::from_secs(1);
    let provider_stopped = loop {
        // SAFETY: signal zero only probes the recorded test provider PID.
        if unsafe { libc::kill(provider_pid, 0) } != 0 {
            break true;
        }
        if Instant::now() >= deadline {
            // SAFETY: cleanup prevents a failing regression from leaking its helper.
            unsafe {
                libc::kill(provider_pid, libc::SIGKILL);
            }
            break false;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    drop(guard);

    assert!(
        follow_up.is_err(),
        "actor accepted work after reply ambiguity"
    );
    assert!(
        actor_stopped_promptly,
        "actor did not reap the fail-stopped broker promptly"
    );
    assert!(
        provider_stopped,
        "retained provider survived actor reply ambiguity"
    );
}

#[test]
fn retained_publication_replacement_disposes_the_previous_provider() {
    let guard = start_for_runtime().expect("replacement fixture starts its broker owner");
    let temp = crate::test_temp::tempdir()
        .expect("replacement fixture creates an isolated provider directory");
    let first_pid_path = temp.path().join("first-provider.pid");
    let second_pid_path = temp.path().join("second-provider.pid");
    let second_release_path = temp.path().join("release-second-provider");
    let second_proof_path = temp.path().join("second-provider-survived");
    let first = guard
        .handle()
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
        .expect("replacement fixture starts its first retained provider");
    assert_eq!(first.status, 0);
    let second = guard
        .handle()
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
        .expect("replacement fixture starts its second retained provider");
    assert_eq!(second.status, 0);

    let first_pid = std::fs::read_to_string(first_pid_path)
        .expect("first replacement provider writes its PID marker")
        .trim()
        .parse::<i32>()
        .expect("first replacement provider PID is valid libc pid_t input");
    let second_pid = std::fs::read_to_string(second_pid_path)
        .expect("second replacement provider writes its PID marker")
        .trim()
        .parse::<i32>()
        .expect("second replacement provider PID is valid libc pid_t input");
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        // SAFETY: signal zero only probes the recorded test provider PID.
        if unsafe { libc::kill(first_pid, 0) } != 0 {
            break;
        }
        let replaced_in_time = Instant::now() < deadline;
        if !replaced_in_time {
            // SAFETY: cleanup keeps a failing regression test from leaking its helpers.
            unsafe {
                libc::kill(first_pid, libc::SIGKILL);
                libc::kill(second_pid, libc::SIGKILL);
            }
        }
        assert!(
            replaced_in_time,
            "replaced publication provider remained alive"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    // SAFETY: signal zero only checks the current test-owned provider.
    assert_eq!(unsafe { libc::kill(second_pid, 0) }, 0);

    drop(guard);
    release_test_provider(&second_release_path, &second_proof_path, second_pid);
}

#[test]
fn failed_publication_replacement_preserves_the_current_provider() {
    let guard = start_for_runtime().expect("failed-replacement fixture starts its broker owner");
    let temp = crate::test_temp::tempdir()
        .expect("failed-replacement fixture creates an isolated provider directory");
    let current_pid_path = temp.path().join("current-provider.pid");
    let failed_pid_path = temp.path().join("failed-provider.pid");
    let current_release_path = temp.path().join("release-current-provider");
    let current_proof_path = temp.path().join("current-provider-survived");
    let current = guard
        .handle()
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
        .expect("failed-replacement fixture starts its current provider");
    assert_eq!(current.status, 0);

    let failed = guard
        .handle()
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
        .expect("failed-replacement fixture receives the provider's exit status");
    assert_eq!(failed.status, 7);

    let current_pid = std::fs::read_to_string(current_pid_path)
        .expect("current provider writes its PID marker")
        .trim()
        .parse::<i32>()
        .expect("current provider PID is valid libc pid_t input");
    let failed_pid = std::fs::read_to_string(failed_pid_path)
        .expect("failed provider writes its PID marker")
        .trim()
        .parse::<i32>()
        .expect("failed provider PID is valid libc pid_t input");
    // SAFETY: signal zero only checks the current test-owned provider.
    assert_eq!(unsafe { libc::kill(current_pid, 0) }, 0);
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        // SAFETY: signal zero only probes the failed test provider PID.
        if unsafe { libc::kill(failed_pid, 0) } != 0 {
            break;
        }
        let cleaned_in_time = Instant::now() < deadline;
        if !cleaned_in_time {
            // SAFETY: cleanup keeps a failing regression test from leaking its helpers.
            unsafe {
                libc::kill(current_pid, libc::SIGKILL);
                libc::kill(failed_pid, libc::SIGKILL);
            }
        }
        assert!(
            cleaned_in_time,
            "failed replacement provider survived cleanup"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    drop(guard);
    release_test_provider(&current_release_path, &current_proof_path, current_pid);
}

#[test]
fn retained_publication_kills_failed_or_input_stalled_provider_groups() {
    let guard = start_for_runtime().expect("failed-provider fixture starts its broker owner");
    for (script, input) in [
        ("sleep 30 <&0 & echo $! > \"$1\"; exit 7", Vec::new()),
        (
            "sleep 30 <&0 & echo $! > \"$1\"; exit 0",
            vec![b'x'; 1024 * 1024],
        ),
    ] {
        let temp = crate::test_temp::tempdir()
            .expect("failed-provider fixture creates an isolated provider directory");
        let pid_path = temp.path().join("provider.pid");
        let result = guard.handle().publish(
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
            .expect("failed or stalled provider writes its PID marker")
            .trim()
            .parse::<i32>()
            .expect("failed or stalled provider PID is valid libc pid_t input");
        if script.ends_with("exit 7") {
            assert_eq!(
                result
                    .expect("failed provider returns its nonzero broker outcome")
                    .status,
                7
            );
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
    let guard = start_for_runtime().expect("incomplete-input fixture starts its broker owner");
    let result = guard.handle().publish(
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
    let guard = start_for_runtime().expect("publication-shutdown fixture starts its broker owner");
    let broker = guard.handle();
    let temp = crate::test_temp::tempdir()
        .expect("publication-shutdown fixture creates an isolated provider directory");
    let current_pid_path = temp.path().join("current-provider.pid");
    let current_release_path = temp.path().join("release-current-provider");
    let current_proof_path = temp.path().join("current-provider-survived");
    let pid_path = temp.path().join("provider.pid");
    let current = guard
        .handle()
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
        .expect("publication-shutdown fixture starts its current retained provider");
    assert_eq!(current.status, 0);
    let current_pid = std::fs::read_to_string(current_pid_path)
        .expect("current publication provider writes its PID marker")
        .trim()
        .parse::<i32>()
        .expect("current publication provider PID is valid libc pid_t input");
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
        .expect("blocked publication provider writes its PID marker")
        .trim()
        .parse::<i32>()
        .expect("blocked publication provider PID is valid libc pid_t input");

    let started = Instant::now();
    drop(guard);
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "broker shutdown waited for the publication deadline"
    );
    assert!(
        publication
            .join()
            .expect("blocked publication request thread returns after broker shutdown")
            .is_err()
    );
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
    let guard = start_for_runtime().expect("large-publication fixture starts its broker owner");
    let temp = crate::test_temp::tempdir()
        .expect("large-publication fixture creates an isolated helper directory");
    let helper = temp.path().join("wl-copy");
    let count_path = temp.path().join("published-bytes");
    std::fs::write(&helper, "#!/bin/sh\nwc -c > \"$1\"\n")
        .expect("large-publication fixture writes its byte-counting provider");
    let mut permissions = std::fs::metadata(&helper)
        .expect("large-publication fixture reads its provider permissions")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o700);
    std::fs::set_permissions(&helper, permissions)
        .expect("large-publication fixture makes its provider executable");

    let output = guard
        .handle()
        .publish(
            HelperKind::WlCopy,
            helper.as_os_str(),
            [count_path.as_os_str()],
            vec![b'x'; PUBLICATION_BYTES],
            Duration::from_secs(5),
        )
        .expect("broker publishes capture-sized clipboard input");

    assert_eq!(output.status, 0);
    assert!(!output.timed_out);
    assert_eq!(
        std::fs::read_to_string(count_path)
            .expect("large-publication provider writes its byte count")
            .trim(),
        PUBLICATION_BYTES.to_string()
    );
}
