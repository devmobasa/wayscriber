use std::os::fd::AsRawFd;
use std::time::Duration;

use super::*;

#[test]
fn production_mode_is_closed_to_published_v2_after_cutover() {
    assert_eq!(
        DaemonControlProtocolMode::production(),
        DaemonControlProtocolMode::PublishedV2
    );
    assert_eq!(
        DaemonControlProtocolMode::dark_harness(),
        DaemonControlProtocolMode::DarkV2Harness
    );
    assert_eq!(
        DaemonControlProtocolMode::rollback_compatibility(),
        DaemonControlProtocolMode::LegacyV1
    );
}

#[test]
fn protocol_id_and_token_use_canonical_lowercase_hex() {
    let first = ProtocolId::generate().unwrap().to_string();
    let second = ProtocolId::generate().unwrap().to_string();
    let token = ProtocolToken::generate().unwrap().to_string();

    assert_eq!(first.len(), 32);
    assert_eq!(second.len(), 32);
    assert_eq!(token.len(), 64);
    assert_ne!(first, second);
    assert!(
        first
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    );
    assert!(
        token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    );
}

#[test]
fn kernel_boot_and_namespace_identities_are_stable() {
    let boot = BootIdentity::read().unwrap();
    assert_eq!(boot.as_str().len(), 36);
    assert_eq!(
        NamespaceIdentity::current_time().unwrap(),
        NamespaceIdentity::current_time().unwrap()
    );
    assert_eq!(
        NamespaceIdentity::current_pid().unwrap(),
        NamespaceIdentity::current_pid().unwrap()
    );
}

#[test]
fn boottime_deadline_source_arms_absolutely_and_drains_once() {
    let source = linux::BootDeadlineSource::new().unwrap();
    assert!(!source.drain().unwrap());

    let deadline = BootClock::now()
        .unwrap()
        .checked_add(Duration::from_millis(5))
        .unwrap();
    source.arm(deadline).unwrap();

    let mut poll_fd = libc::pollfd {
        fd: source.poll_fd().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll_fd remains valid for this bounded poll.
    let ready = unsafe { libc::poll(&mut poll_fd, 1, 500) };
    assert_eq!(ready, 1);
    assert_ne!(poll_fd.revents & libc::POLLIN, 0);
    assert!(source.drain().unwrap());
    assert!(!source.drain().unwrap());
    source.disarm().unwrap();
    assert!(BootClock::now().unwrap().as_nanos() >= deadline.as_nanos());
}

#[test]
fn self_pidfd_is_not_readable_while_process_is_alive() {
    let pidfd = linux::open_self_pidfd().unwrap();
    let mut poll_fd = libc::pollfd {
        fd: pidfd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll_fd remains valid for the zero-timeout observation.
    let ready = unsafe { libc::poll(&mut poll_fd, 1, 0) };
    assert_eq!(ready, 0);
}

#[test]
fn bounded_nofollow_reads_reject_symlinks_and_oversize_records() {
    use std::os::unix::fs::symlink;

    let temp = crate::test_temp::tempdir().unwrap();
    let regular = temp.path().join("record");
    std::fs::write(&regular, b"bounded").unwrap();
    assert_eq!(
        linux::read_bounded_regular_file(&regular, 7).unwrap(),
        b"bounded"
    );
    assert!(linux::read_bounded_regular_file(&regular, 6).is_err());

    let link = temp.path().join("link");
    symlink(&regular, &link).unwrap();
    assert!(linux::read_bounded_regular_file(&link, 7).is_err());
}

#[test]
fn directory_identity_detects_path_replacement() {
    let temp = crate::test_temp::tempdir().unwrap();
    let first = temp.path().join("first");
    let replacement = temp.path().join("replacement");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&replacement).unwrap();
    let (_file, identity) = linux::open_nofollow_directory(&first).unwrap();
    linux::revalidate_path_identity(&first, identity).unwrap();
    std::fs::rename(&first, temp.path().join("old")).unwrap();
    std::fs::rename(&replacement, &first).unwrap();
    assert!(linux::revalidate_path_identity(&first, identity).is_err());
}

#[test]
fn process_start_identity_is_nonzero_and_pidfd_matches_live_process() {
    assert!(linux::current_process_start_ticks().unwrap() > 0);
    let _pidfd = linux::open_pidfd(std::process::id()).unwrap();
}

#[test]
fn command_queue_watch_reports_atomic_publication_and_rejects_replacement() {
    let temp = crate::test_temp::tempdir().unwrap();
    let queue = temp.path().join("queue");
    std::fs::create_dir(&queue).unwrap();
    let mut watcher = CommandQueueWatcher::new(&queue).unwrap();
    std::fs::write(temp.path().join("temporary"), b"request").unwrap();
    std::fs::rename(
        temp.path().join("temporary"),
        queue.join("0000000000000001-00000000000000000000000000000000.request"),
    )
    .unwrap();

    let mut poll_fd = libc::pollfd {
        fd: watcher.poll_fd().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: watcher owns the descriptor throughout the bounded poll.
    assert_eq!(unsafe { libc::poll(&mut poll_fd, 1, 500) }, 1);
    assert!(watcher.drain().unwrap().scan_pending);

    std::fs::rename(&queue, temp.path().join("old-queue")).unwrap();
    std::fs::create_dir(&queue).unwrap();
    assert!(watcher.revalidate().is_err());
}

#[test]
fn fail_stop_has_a_non_returning_signature() {
    let _terminator: unsafe fn(i32) -> ! = linux::fail_stop;
}

#[test]
fn only_an_identified_internal_overlay_can_claim_daemon_actions() {
    let _environment = crate::test_env::lock();
    let temp = crate::test_temp::tempdir().unwrap();
    let previous_runtime = std::env::var_os(crate::env_vars::XDG_RUNTIME_DIR_ENV);
    let previous_generation = std::env::var_os(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV);
    // SAFETY: serialized by the test environment mutex.
    unsafe {
        std::env::set_var(crate::env_vars::XDG_RUNTIME_DIR_ENV, temp.path());
        std::env::remove_var(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV);
    }
    let token = ProtocolToken::generate().unwrap();
    let token_text = token.to_string();
    let _owner = CommandOwner::open(&token_text).unwrap();
    let journal = ActionJournal::open().unwrap();
    journal
        .publish_anonymous(&token_text, crate::tray_action::TrayAction::ToggleFreeze)
        .unwrap();
    let runtime = DaemonRuntimeRecordV2::current(token).unwrap();
    write_runtime_record_v2(&crate::paths::daemon_pid_file(), &runtime).unwrap();

    assert!(matches!(
        try_claim_overlay_action().unwrap(),
        ActionClaimOutcome::Idle
    ));

    let generation = ProtocolId::generate().unwrap().to_string();
    // SAFETY: serialized by the test environment mutex.
    unsafe {
        std::env::set_var(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV, &generation);
    }
    publish_ready_from_environment().unwrap();
    let started = std::time::Instant::now();
    assert!(matches!(
        try_claim_overlay_action().unwrap(),
        ActionClaimOutcome::Deferred
    ));
    assert!(
        started.elapsed() < std::time::Duration::from_millis(100),
        "missing enable proof blocked instead of deferring"
    );
    child::enable_current_generation_for_test(&token_text).unwrap();
    let ActionClaimOutcome::Claimed(action) = try_claim_overlay_action().unwrap() else {
        panic!("enabled overlay should claim the durable action");
    };
    action.finish(true, None).unwrap();

    // SAFETY: this test still holds the environment mutex.
    unsafe {
        match previous_runtime {
            Some(value) => std::env::set_var(crate::env_vars::XDG_RUNTIME_DIR_ENV, value),
            None => std::env::remove_var(crate::env_vars::XDG_RUNTIME_DIR_ENV),
        }
        match previous_generation {
            Some(value) => std::env::set_var(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV, value),
            None => std::env::remove_var(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV),
        }
    }
}
