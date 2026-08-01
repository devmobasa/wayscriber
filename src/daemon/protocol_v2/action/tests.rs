use std::env;

use super::*;
use crate::env_vars::XDG_RUNTIME_DIR_ENV;

#[test]
fn action_digests_match_protocol_v2_golden_values() {
    const ACTION_ID: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const COMMAND_ID: &str = "cccccccccccccccccccccccccccccccc";
    const DAEMON_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    let anonymous = ActionOwner::Anonymous {
        daemon_token: DAEMON_TOKEN.into(),
    };
    assert_eq!(
        digest_payload(ACTION_ID, 42, &anonymous, TrayAction::CaptureRegion).unwrap(),
        "53a40b0ef73b768dfa543746835ac6704255db3b9c9c1ad6b06a38f98a13a9c1"
    );

    let command = ActionOwner::Command {
        command_identity: COMMAND_ID.into(),
        daemon_token: DAEMON_TOKEN.into(),
    };
    assert_eq!(
        digest_payload(ACTION_ID, 42, &command, TrayAction::ToggleHelp).unwrap(),
        "c8aa91f67acd22621252e0e95cee6221cc02a9f1ca72cdc78e46eb3fd0dabf33"
    );
}

fn with_runtime<T>(run: impl FnOnce() -> T) -> T {
    let _guard = crate::test_env::lock();
    let temp = crate::test_temp::tempdir().unwrap();
    let previous = env::var_os(XDG_RUNTIME_DIR_ENV);
    // SAFETY: serialized by the test environment mutex.
    unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, temp.path()) };
    super::super::command::prepare_layout(&super::super::command_root()).unwrap();
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

#[test]
fn anonymous_actions_keep_global_order_and_terminal_tombstones() {
    with_runtime(|| {
        let journal = ActionJournal::open().unwrap();
        let token = super::super::ProtocolToken::generate().unwrap().to_string();
        journal
            .publish_anonymous(&token, TrayAction::LightDrawOn)
            .unwrap();
        journal
            .publish_anonymous(&token, TrayAction::LightDrawOff)
            .unwrap();
        let first = journal
            .claim_next(&token, |_, _| Ok(false))
            .unwrap()
            .unwrap();
        assert_eq!(first.action(), TrayAction::LightDrawOn);
        first.finish(true, None).unwrap();
        let second = journal
            .claim_next(&token, |_, _| Ok(false))
            .unwrap()
            .unwrap();
        assert_eq!(second.action(), TrayAction::LightDrawOff);
        second.finish(false, Some("not active")).unwrap();
        assert!(
            journal
                .claim_next(&token, |_, _| Ok(false))
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn command_action_stays_ineligible_until_exact_commit_predicate() {
    with_runtime(|| {
        let journal = ActionJournal::open().unwrap();
        let token = super::super::ProtocolToken::generate().unwrap().to_string();
        let command = super::super::ProtocolId::generate().unwrap().to_string();
        let prepared = journal
            .prepare_command(&command, &token, TrayAction::ToggleFreeze)
            .unwrap();
        journal
            .publish_anonymous(&token, TrayAction::CaptureRegion)
            .unwrap();
        assert!(
            journal
                .claim_next(&token, |_, _| Ok(false))
                .unwrap()
                .is_none()
        );
        let claimed = journal
            .claim_next(&token, |identity, candidate| {
                Ok(identity == command && candidate.action_id == prepared.action_id)
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            claimed.owner(),
            &ActionOwner::Command {
                command_identity: command,
                daemon_token: token,
            }
        );
        assert_eq!(claimed.action(), TrayAction::ToggleFreeze);
    });
}

#[test]
fn event_loop_claim_and_finish_defer_instead_of_waiting_for_locks() {
    with_runtime(|| {
        let token = super::super::ProtocolToken::generate().unwrap().to_string();
        let owner = super::super::command::CommandOwner::open(&token).unwrap();
        let journal = ActionJournal::open().unwrap();
        let request = super::super::wire::DaemonRequestV2 {
            mode: None,
            freeze: false,
            exit_after_capture: false,
            no_exit_after_capture: false,
            resume_session: false,
            no_resume_session: false,
            session_file: None,
            overlay_action: Some(TrayAction::ToggleFreeze),
        };
        let _client = super::super::command::ClientCommand::publish(&request, &token).unwrap();
        let mut command = owner.claim_next().unwrap().unwrap();
        let command_identity = command.identity().to_owned();
        let _prepared = command.prepare_action(&journal).unwrap().unwrap();
        command
            .commit(super::super::wire::EffectKind::DeliverReadyAction)
            .unwrap();

        let held_journal_lock = open_journal_lock(&journal.root).unwrap();
        assert!(matches!(
            journal
                .try_claim_next(&token, |identity, candidate| {
                    super::super::command::try_claim_command_action(identity, candidate)
                })
                .unwrap(),
            ActionClaimOutcome::Deferred
        ));
        unlock(&held_journal_lock).unwrap();

        // The command claim still owns decision.lock, so the action claimant
        // must defer without sleeping on the Wayland event-loop thread.
        assert!(matches!(
            journal
                .try_claim_next(&token, |identity, candidate| {
                    super::super::command::try_claim_command_action(identity, candidate)
                })
                .unwrap(),
            ActionClaimOutcome::Deferred
        ));
        command.defer().unwrap();

        let ActionClaimOutcome::Claimed(action) = journal
            .try_claim_next(&token, |identity, candidate| {
                super::super::command::try_claim_command_action(identity, candidate)
            })
            .unwrap()
        else {
            panic!("released command lock should make the action claimable");
        };

        let decision_path = super::super::command_root()
            .join("controls")
            .join(command_identity)
            .join("decision.lock");
        let held_decision = OpenOptions::new()
            .read(true)
            .write(true)
            .open(decision_path)
            .unwrap();
        assert_eq!(
            unsafe { libc::flock(held_decision.as_raw_fd(), libc::LOCK_EX) },
            0
        );
        let ActionFinishOutcome::Deferred(action) = action.try_finish(true, None).unwrap() else {
            panic!("contended command finish should defer");
        };
        assert_eq!(
            unsafe { libc::flock(held_decision.as_raw_fd(), libc::LOCK_UN) },
            0
        );
        assert!(matches!(
            action.try_finish(true, None).unwrap(),
            ActionFinishOutcome::Complete
        ));
    });
}

#[test]
fn cancellation_during_action_preparation_leaves_a_collectable_tombstone() {
    with_runtime(|| {
        let token = super::super::ProtocolToken::generate().unwrap().to_string();
        let owner = super::super::command::CommandOwner::open(&token).unwrap();
        let journal = ActionJournal::open().unwrap();
        let client = super::super::command::ClientCommand::publish(
            &super::super::wire::DaemonRequestV2 {
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
        let command = owner.claim_next().unwrap().unwrap();
        let held_journal_lock = open_journal_lock(&journal.root).unwrap();
        let worker_journal = journal.clone();
        let worker = std::thread::spawn(move || {
            let mut command = command;
            let outcome = command.prepare_action(&worker_journal);
            (outcome, command)
        });

        assert_eq!(
            client.cancel().unwrap(),
            super::super::command::TerminalCommandResult::Canceled
        );
        unlock(&held_journal_lock).unwrap();
        let (outcome, command) = worker.join().unwrap();
        assert!(outcome.unwrap().is_none());
        command.defer().unwrap();

        assert!(
            journal
                .claim_next(&token, |_, _| Ok(false))
                .unwrap()
                .is_none()
        );
        assert_eq!(owner.collect_terminal().unwrap(), 1);
    });
}

#[test]
fn canceled_command_reconciles_prepared_and_crash_left_action_envelopes() {
    for record_preparation in [false, true] {
        with_runtime(|| {
            let token = super::super::ProtocolToken::generate().unwrap().to_string();
            let owner = super::super::command::CommandOwner::open(&token).unwrap();
            let journal = ActionJournal::open().unwrap();
            let client = super::super::command::ClientCommand::publish(
                &super::super::wire::DaemonRequestV2 {
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
            let mut command = owner.claim_next().unwrap().unwrap();
            if record_preparation {
                command.prepare_action(&journal).unwrap().unwrap();
            } else {
                journal
                    .prepare_command(command.identity(), &token, TrayAction::ToggleFreeze)
                    .unwrap();
            }
            command.defer().unwrap();
            assert_eq!(
                client.cancel().unwrap(),
                super::super::command::TerminalCommandResult::Canceled
            );

            assert!(matches!(
                journal
                    .try_claim_next(&token, |identity, candidate| {
                        super::super::command::try_claim_command_action(identity, candidate)
                    })
                    .unwrap(),
                ActionClaimOutcome::Idle
            ));
            assert_eq!(owner.collect_terminal().unwrap(), 1);
        });
    }
}

#[test]
fn orphaned_claim_becomes_committed_indeterminate_without_replay() {
    with_runtime(|| {
        let token = super::super::ProtocolToken::generate().unwrap().to_string();
        let owner = super::super::command::CommandOwner::open(&token).unwrap();
        let journal = ActionJournal::open().unwrap();
        let request = super::super::wire::DaemonRequestV2 {
            mode: None,
            freeze: false,
            exit_after_capture: false,
            no_exit_after_capture: false,
            resume_session: false,
            no_resume_session: false,
            session_file: None,
            overlay_action: Some(TrayAction::ToggleFreeze),
        };
        let client = super::super::command::ClientCommand::publish(&request, &token).unwrap();
        let mut command = owner.claim_next().unwrap().unwrap();
        command.prepare_action(&journal).unwrap().unwrap();
        command
            .commit(super::super::wire::EffectKind::DeliverReadyAction)
            .unwrap();
        command.defer().unwrap();

        let ActionClaimOutcome::Claimed(orphaned) = journal
            .try_claim_next(&token, |identity, candidate| {
                super::super::command::try_claim_command_action(identity, candidate)
            })
            .unwrap()
        else {
            panic!("committed action should be claimable");
        };
        drop(orphaned);

        assert!(matches!(
            journal
                .try_claim_next(&token, |identity, candidate| {
                    super::super::command::try_claim_command_action(identity, candidate)
                })
                .unwrap(),
            ActionClaimOutcome::Idle
        ));
        assert!(matches!(
            client.wait().unwrap(),
            super::super::command::TerminalCommandResult::CommittedIndeterminate(_)
        ));
    });
}

#[test]
fn digest_and_filename_tampering_fail_closed() {
    with_runtime(|| {
        let journal = ActionJournal::open().unwrap();
        let token = super::super::ProtocolToken::generate().unwrap().to_string();
        let action = journal
            .publish_anonymous(&token, TrayAction::CaptureRegion)
            .unwrap();
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(&action.path).unwrap()).unwrap();
        record["action"] = serde_json::json!("capture_full");
        fs::write(&action.path, serde_json::to_vec(&record).unwrap()).unwrap();
        assert!(
            journal
                .claim_next(&token, |_, _| Ok(false))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            fs::read_dir(quarantine_dir(&journal.root)).unwrap().count(),
            1
        );
    });
}

#[test]
fn filename_order_tampering_and_revision_overflow_fail_closed() {
    with_runtime(|| {
        let journal = ActionJournal::open().unwrap();
        let token = super::super::ProtocolToken::generate().unwrap().to_string();
        let action = journal
            .publish_anonymous(&token, TrayAction::CaptureRegion)
            .unwrap();
        let changed_path = queue_dir(&journal.root).join(action_name(
            action.action_order.checked_add(1).unwrap(),
            &action.action_id,
        ));
        fs::rename(&action.path, changed_path).unwrap();
        assert!(
            journal
                .claim_next(&token, |_, _| Ok(false))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            fs::read_dir(quarantine_dir(&journal.root)).unwrap().count(),
            1
        );
    });

    with_runtime(|| {
        let journal = ActionJournal::open().unwrap();
        let token = super::super::ProtocolToken::generate().unwrap().to_string();
        let action = journal
            .publish_anonymous(&token, TrayAction::CaptureRegion)
            .unwrap();
        let mut record: ActionRecord = read_record(&action.path).unwrap();
        record.record_revision = u64::MAX;
        write_record(&action.path, &record).unwrap();
        assert!(journal.abandon(&action, "not applied").is_err());
    });
}

#[test]
fn rollback_keeps_an_indeterminate_command_action_tombstone() {
    with_runtime(|| {
        let token = super::super::ProtocolToken::generate().unwrap().to_string();
        let owner = super::super::CommandOwner::open(&token).unwrap();
        let journal = ActionJournal::open().unwrap();
        let client = super::super::ClientCommand::publish(
            &super::super::DaemonRequestV2 {
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
        let mut claim = owner.claim_next().unwrap().unwrap();
        let prepared = claim.prepare_action(&journal).unwrap().unwrap();
        claim
            .commit(super::super::EffectKind::StartAndDeliverAction)
            .unwrap();
        claim.defer().unwrap();

        super::super::prepare_rollback_compatibility().unwrap();
        let tombstone: ActionRecord = read_record(&prepared.path).unwrap();
        assert!(matches!(tombstone.state, JournalState::Abandoned { .. }));
        assert!(matches!(
            client.wait().unwrap(),
            super::super::TerminalCommandResult::CommittedIndeterminate(_)
        ));
    });
}
