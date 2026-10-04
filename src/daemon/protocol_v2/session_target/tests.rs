use std::ffi::OsStr;

use super::*;
use crate::env_vars::{
    OVERLAY_CHILD_GENERATION_ENV, OVERLAY_SESSION_REPORTS_ENV, XDG_RUNTIME_DIR_ENV,
};

/// Runs `body` as the overlay child `generation` of a daemon that reads
/// session reports when `reports` is set, in a private runtime directory.
fn as_daemon_overlay(reports: Option<&str>, body: impl FnOnce(&str)) {
    let runtime = crate::test_temp::tempdir().unwrap();
    let generation = super::super::ProtocolId::generate().unwrap().to_string();

    crate::test_env::with_env_vars(
        &[
            (XDG_RUNTIME_DIR_ENV, Some(runtime.path().as_os_str())),
            (OVERLAY_CHILD_GENERATION_ENV, Some(OsStr::new(&generation))),
            (OVERLAY_SESSION_REPORTS_ENV, reports.map(OsStr::new)),
        ],
        || body(&generation),
    );
}

fn read_record(generation: &str) -> SessionTargetRecord {
    let bytes = std::fs::read(report_path(generation)).unwrap();
    super::super::wire::parse_canonical_json(&bytes, MAX_REPORT_BYTES).unwrap()
}

#[test]
fn a_daemon_overlay_reports_its_session_under_its_own_identity() {
    as_daemon_overlay(Some("1"), |generation| {
        super::super::publish_ready_from_environment().unwrap();

        let named = ReportedSession::Named(PathBuf::from("lectures/b.wayscriber-session"));
        assert!(publish_session_from_environment(&named).unwrap());

        let record = read_record(generation);
        assert_eq!(record.schema, REPORT_SCHEMA);
        assert_eq!(record.generation, generation);
        assert_eq!(record.pid, std::process::id());
        assert_eq!(
            record.process_start_ticks,
            super::super::linux::current_process_start_ticks().unwrap()
        );
        let expected = std::env::current_dir()
            .unwrap()
            .join("lectures/b.wayscriber-session");
        assert_eq!(record.target.as_deref(), expected.to_str());

        // A later report replaces the earlier one; home carries no path.
        assert!(publish_session_from_environment(&ReportedSession::Home).unwrap());
        assert_eq!(read_record(generation).target, None);
    });
}

#[test]
fn only_a_daemon_that_reads_reports_receives_one() {
    for marker in [None, Some("0")] {
        as_daemon_overlay(marker, |generation| {
            super::super::publish_ready_from_environment().unwrap();

            assert!(!publish_session_from_environment(&ReportedSession::Home).unwrap());
            assert!(!report_path(generation).exists());
        });
    }
}

#[test]
fn an_overlay_without_its_child_identity_cannot_report() {
    as_daemon_overlay(Some("1"), |generation| {
        assert!(publish_session_from_environment(&ReportedSession::Home).is_err());
        assert!(!report_path(generation).exists());
    });
}
