use super::{DialogFramePhase, dialog_frame_accepted, populate_session_snapshot};
use crate::backend::wayland::session::{SessionHome, SessionLaunch};
use crate::backend::wayland::state::RenderOutcome;
use crate::backend::wayland::state::toolbar::ToolbarSnapshot;
use crate::session::SessionTarget;

#[test]
fn queued_commands_that_did_not_run_are_all_named_in_one_toast() {
    use crate::backend::wayland::handlers::test_support::HandlerFixture;
    use crate::backend::wayland::session::SessionCommand;
    let mut fixture = HandlerFixture::new(crate::config::Config::default());
    let reason = || anyhow::anyhow!("the visible session changed");

    fixture.state.fail_queued_session_commands(
        vec![
            (
                SessionCommand::SaveAs(
                    "/sessions/copy.wayscriber-session".into(),
                    crate::session::SaveAsOverwrite::Deny,
                ),
                reason(),
            ),
            (SessionCommand::Clear, reason()),
        ],
        false,
    );

    let message = &fixture.state.input_state.active_toast().unwrap().message;
    assert!(
        message.contains("save as copy.wayscriber-session")
            && message.contains("clear the session"),
        "{message}"
    );
}

#[test]
fn the_session_menu_learns_home_and_whether_it_is_active() {
    let input = crate::input::state::test_support::make_test_input_state();
    let mut snapshot = ToolbarSnapshot::from_input_with_bindings(&input, Default::default());
    let home = std::path::PathBuf::from("/sessions/home.wayscriber-session");
    let away = SessionHome::new(
        SessionLaunch {
            home: crate::backend::wayland::session::HomeSession::Named(home),
            preferred: None,
            from_daemon: true,
        },
        None,
        SessionTarget::NamedFile("/sessions/b.wayscriber-session".into()),
    );

    populate_session_snapshot(&mut snapshot, None, &away);

    assert_eq!(
        snapshot.home_session_name.as_deref(),
        Some("home.wayscriber-session")
    );
    assert!(!snapshot.at_home_session);
}

#[test]
fn dialog_entry_requires_a_committed_frame() {
    assert!(dialog_frame_accepted(
        DialogFramePhase::Entry,
        RenderOutcome::Committed {
            keep_rendering: false
        }
    ));
    assert!(dialog_frame_accepted(
        DialogFramePhase::Entry,
        RenderOutcome::Committed {
            keep_rendering: true
        }
    ));
    assert!(!dialog_frame_accepted(
        DialogFramePhase::Entry,
        RenderOutcome::BuffersInFlight
    ));
}

#[test]
fn dialog_restoration_accepts_a_deferred_frame() {
    assert!(dialog_frame_accepted(
        DialogFramePhase::Restoration,
        RenderOutcome::Committed {
            keep_rendering: false
        }
    ));
    assert!(dialog_frame_accepted(
        DialogFramePhase::Restoration,
        RenderOutcome::Committed {
            keep_rendering: true
        }
    ));
    assert!(dialog_frame_accepted(
        DialogFramePhase::Restoration,
        RenderOutcome::BuffersInFlight
    ));
}
