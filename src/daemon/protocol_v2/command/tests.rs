use std::env;

use super::super::BootClock;
use super::super::action::ActionJournal;
use super::super::wire::*;
use std::fs;

use super::layout::*;
use super::recovery::parse_queue_name;
use super::*;
use crate::env_vars::XDG_RUNTIME_DIR_ENV;
use crate::tray_action::TrayAction;

fn with_runtime<T>(run: impl FnOnce() -> T) -> T {
    let _guard = crate::test_env::lock();
    let temp = crate::test_temp::tempdir().unwrap();
    let previous = env::var_os(XDG_RUNTIME_DIR_ENV);
    // SAFETY: serialized by the test environment mutex.
    unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, temp.path()) };
    let result = run();
    if let Some(previous) = previous {
        // SAFETY: serialized by the test environment mutex.
        unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, previous) };
    } else {
        // SAFETY: serialized by the test environment mutex.
        unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) };
    }
    result
}

fn token() -> String {
    super::super::ProtocolToken::generate().unwrap().to_string()
}

#[test]
fn publish_claim_commit_complete_ack_and_gc() {
    with_runtime(|| {
        let token = token();
        let owner = CommandOwner::open(&token).unwrap();
        let client = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: None,
                freeze: true,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: None,
            },
            &token,
        )
        .unwrap();
        let mut claimed = owner.claim_next().unwrap().unwrap();
        assert!(claimed.request().freeze);
        claimed.commit(EffectKind::StartAndShow).unwrap();
        claimed.finalize(FinalEffect::Completed, None).unwrap();
        assert_eq!(client.wait().unwrap(), TerminalCommandResult::Succeeded);
        assert_eq!(owner.collect_terminal().unwrap(), 1);
        assert!(
            read_dir_bounded(&controls_dir(&owner.root), 1)
                .unwrap()
                .is_empty()
        );
    });
}

#[test]
fn abandoned_unqueued_publication_is_rejected_and_collected() {
    with_runtime(|| {
        let token = token();
        let owner = CommandOwner::open(&token).unwrap();
        let client = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: None,
                freeze: false,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: None,
            },
            &token,
        )
        .unwrap();
        let mut control = read_control(&client.control_path).unwrap();
        fs::remove_file(queue_path(
            &owner.root,
            control.queue_order,
            &control.identity,
        ))
        .unwrap();
        control.publication_state = PublicationState::VisibleUnqueued;
        bump_revision(&mut control).unwrap();
        write_control(&client.control_path, &control).unwrap();
        let path = client.control_path.clone();
        drop(client);

        assert!(owner.claim_next().unwrap().is_none());
        let recovered = read_control(&path).unwrap();
        assert!(matches!(
            recovered.decision,
            CommandDecision::Rejected { .. }
        ));
        assert!(matches!(
            recovered.caller_disposition,
            CallerDisposition::Abandoned
        ));
        assert_eq!(owner.collect_terminal().unwrap(), 1);
    });
}

#[test]
fn queue_rename_remains_the_enqueue_point_if_control_update_is_interrupted() {
    with_runtime(|| {
        let token = token();
        let owner = CommandOwner::open(&token).unwrap();
        let client = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: None,
                freeze: true,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: None,
            },
            &token,
        )
        .unwrap();
        let mut control = read_control(&client.control_path).unwrap();
        control.publication_state = PublicationState::VisibleUnqueued;
        bump_revision(&mut control).unwrap();
        write_control(&client.control_path, &control).unwrap();
        drop(client);

        let claimed = owner.claim_next().unwrap().unwrap();
        assert!(claimed.request().freeze);
        assert!(matches!(claimed.control().decision, CommandDecision::Open));
        claimed.defer().unwrap();
    });
}

#[test]
fn queued_control_remains_recoverable_if_its_reference_is_lost() {
    with_runtime(|| {
        let token = token();
        let owner = CommandOwner::open(&token).unwrap();
        let client = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: Some("whiteboard".into()),
                freeze: false,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: None,
            },
            &token,
        )
        .unwrap();
        let control = read_control(&client.control_path).unwrap();
        fs::remove_file(queue_path(
            &owner.root,
            control.queue_order,
            &control.identity,
        ))
        .unwrap();

        let claimed = owner.claim_next().unwrap().unwrap();
        assert_eq!(claimed.request().mode.as_deref(), Some("whiteboard"));
        assert!(matches!(
            claimed.control().publication_state,
            PublicationState::Claimed { .. }
        ));
        claimed.defer().unwrap();
        drop(client);
    });
}

#[test]
fn terminal_command_from_a_dead_caller_does_not_leak() {
    with_runtime(|| {
        let token = token();
        let owner = CommandOwner::open(&token).unwrap();
        let client = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: None,
                freeze: false,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: None,
            },
            &token,
        )
        .unwrap();
        let mut claimed = owner.claim_next().unwrap().unwrap();
        claimed.commit(EffectKind::NoOp).unwrap();
        claimed.defer().unwrap();
        drop(client);

        assert_eq!(owner.collect_terminal().unwrap(), 1);
        assert!(
            read_dir_bounded(&controls_dir(&owner.root), 1)
                .unwrap()
                .is_empty()
        );
    });
}

#[test]
fn cancellation_and_commit_are_mutually_exclusive() {
    with_runtime(|| {
        let token = token();
        let owner = CommandOwner::open(&token).unwrap();
        let client = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: None,
                freeze: false,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: Some(TrayAction::ToggleFreeze),
            },
            &token,
        )
        .unwrap();
        assert_eq!(client.cancel().unwrap(), TerminalCommandResult::Canceled);
        assert!(owner.claim_next().unwrap().is_none());
        owner.collect_terminal().unwrap();
        assert!(
            read_dir_bounded(&controls_dir(&owner.root), 1)
                .unwrap()
                .is_empty()
        );
    });
}

#[test]
fn failed_preclaim_action_is_terminal_without_waiting_for_restart() {
    with_runtime(|| {
        let token = token();
        let owner = CommandOwner::open(&token).unwrap();
        let journal = ActionJournal::open().unwrap();
        let client = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: None,
                freeze: false,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: Some(TrayAction::ToggleFreeze),
            },
            &token,
        )
        .unwrap();
        let mut claimed = owner.claim_next().unwrap().unwrap();
        let identity = claimed.identity().to_owned();
        let prepared = claimed.prepare_action(&journal).unwrap().unwrap();
        claimed.commit(EffectKind::StartAndDeliverAction).unwrap();
        claimed.defer().unwrap();

        journal
            .abandon_command(&identity, &prepared, "overlay readiness failed")
            .unwrap();
        assert_eq!(
            client.wait().unwrap(),
            TerminalCommandResult::CommittedIndeterminate("overlay readiness failed".into())
        );
        assert_eq!(owner.collect_terminal().unwrap(), 1);
    });
}

#[test]
fn rollback_rejects_open_work_and_preserves_committed_ambiguity() {
    with_runtime(|| {
        let old_token = token();
        let owner = CommandOwner::open(&old_token).unwrap();
        let committed = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: Some("whiteboard".into()),
                freeze: false,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: None,
            },
            &old_token,
        )
        .unwrap();
        let mut claim = owner.claim_next().unwrap().unwrap();
        claim.commit(EffectKind::StartAndShow).unwrap();
        claim.defer().unwrap();
        let open = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: None,
                freeze: true,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: None,
            },
            &old_token,
        )
        .unwrap();

        super::super::prepare_rollback_compatibility().unwrap();
        assert!(matches!(
            committed.wait().unwrap(),
            TerminalCommandResult::CommittedIndeterminate(_)
        ));
        assert!(matches!(
            open.wait().unwrap(),
            TerminalCommandResult::FailedNoEffect(_)
        ));
        assert!(
            read_dir_bounded(&queue_dir(&command_root()), 1)
                .unwrap()
                .is_empty()
        );
    });
}

#[test]
fn canonical_queue_order_survives_restart_and_ref_less_claim() {
    with_runtime(|| {
        let token = token();
        let owner = CommandOwner::open(&token).unwrap();
        let _first_client = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: Some("whiteboard".into()),
                freeze: false,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: None,
            },
            &token,
        )
        .unwrap();
        let claimed = owner.claim_next().unwrap().unwrap();
        let order = claimed.control().queue_order;
        drop(claimed);
        let _second_client = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: Some("blackboard".into()),
                freeze: false,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: None,
            },
            &token,
        )
        .unwrap();
        let restarted = CommandOwner::open(&token).unwrap();
        let claimed = restarted.claim_next().unwrap().unwrap();
        assert_eq!(claimed.control().queue_order, order);
        assert_eq!(claimed.request().mode.as_deref(), Some("whiteboard"));
    });
}

#[test]
fn queue_filename_order_and_no_op_commit_are_strict() {
    with_runtime(|| {
        let token = token();
        let owner = CommandOwner::open(&token).unwrap();
        let client = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: None,
                freeze: false,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: None,
            },
            &token,
        )
        .unwrap();
        let original = read_dir_bounded(&queue_dir(&owner.root), 1)
            .unwrap()
            .pop()
            .unwrap()
            .path();
        let name = original.file_name().unwrap().to_str().unwrap();
        let (order, identity) = parse_queue_name(name).unwrap();
        let changed = queue_path(&owner.root, order.checked_add(1).unwrap(), &identity);
        fs::rename(&original, changed).unwrap();
        assert!(owner.claim_next().unwrap().is_none());
        assert_eq!(
            read_dir_bounded(&quarantine_dir(&owner.root).join("queue"), 2)
                .unwrap()
                .len(),
            1
        );
        drop(client);
    });

    with_runtime(|| {
        let token = token();
        let owner = CommandOwner::open(&token).unwrap();
        let client = ClientCommand::publish(
            &DaemonRequestV2 {
                mode: None,
                freeze: false,
                exit_after_capture: false,
                no_exit_after_capture: false,
                resume_session: false,
                no_resume_session: false,
                session_file: None,
                overlay_action: Some(TrayAction::LightDrawOff),
            },
            &token,
        )
        .unwrap();
        let mut claimed = owner.claim_next().unwrap().unwrap();
        claimed.commit(EffectKind::NoOp).unwrap();
        claimed.defer().unwrap();
        assert_eq!(client.wait().unwrap(), TerminalCommandResult::Succeeded);
    });
}

#[test]
fn unknown_root_entries_and_noncanonical_queue_names_fail_closed() {
    with_runtime(|| {
        let root = command_root();
        prepare_layout(&root).unwrap();
        fs::write(root.join("future"), b"sentinel").unwrap();
        assert!(validate_root_shape(&root).is_err());
        fs::remove_file(root.join("future")).unwrap();
        assert!(
            parse_queue_name("000000000000000A-00000000000000000000000000000000.request").is_err()
        );
    });
}

#[test]
fn transition_types_cover_reconciliation_targets() {
    let target = ReconciliationTarget::DaemonCleanup {
        daemon_token: "0".repeat(64),
        recovery_generation: "1".repeat(32),
        cleanup_id: "2".repeat(32),
    };
    let status = ReconciliationStatus::Pending {
        reconciliation_id: "3".repeat(32),
        target,
        notification_kind: NotificationKind::Record,
        effect_id: None,
        action_id: None,
        opened_required_revision: 1,
    };
    assert!(matches!(status, ReconciliationStatus::Pending { .. }));
}

#[test]
fn lock_deadline_is_reported_without_parsing_error_text() {
    with_runtime(|| {
        let root = command_root();
        prepare_layout(&root).unwrap();
        let path = root.join("structured-timeout.lock");
        let held = open_lock(&path, true).unwrap();
        let contender = open_lock(&path, false).unwrap();
        flock(&held, libc::LOCK_EX).unwrap();

        let deadline = BootClock::now()
            .unwrap()
            .checked_add(std::time::Duration::from_millis(5))
            .unwrap();
        assert!(!try_lock_until(&contender, libc::LOCK_EX, deadline).unwrap());

        unlock(&held).unwrap();
        let deadline = BootClock::now()
            .unwrap()
            .checked_add(std::time::Duration::from_millis(5))
            .unwrap();
        assert!(try_lock_until(&contender, libc::LOCK_EX, deadline).unwrap());
        unlock(&contender).unwrap();
    });
}
