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

fn current_identity() -> (u32, u64) {
    (
        std::process::id(),
        super::super::linux::current_process_start_ticks().unwrap(),
    )
}

/// Writes `bytes` as `name` in the report directory, private like the writer
/// leaves it unless `mode` says otherwise.
fn write_entry(name: &str, bytes: &[u8], mode: u32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    // An overlay's identity proof creates this before any report is written.
    std::fs::create_dir_all(crate::paths::daemon_command_dir()).unwrap();
    create_report_dir().unwrap();
    let path = report_dir().join(name);
    std::fs::write(&path, bytes).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    path
}

fn record_bytes(generation: &str, pid: u32, ticks: u64, target: Option<&str>) -> Vec<u8> {
    super::super::wire::canonical_json(
        &SessionTargetRecord {
            schema: REPORT_SCHEMA,
            generation: generation.to_owned(),
            pid,
            process_start_ticks: ticks,
            target: target.map(str::to_owned),
        },
        MAX_REPORT_BYTES,
    )
    .unwrap()
}

#[test]
fn the_daemon_reads_a_report_only_under_the_identity_it_owns() {
    as_daemon_overlay(Some("1"), |generation| {
        super::super::publish_ready_from_environment().unwrap();
        let session = ReportedSession::Named(PathBuf::from("/sessions/b.wayscriber-session"));
        publish_session_from_environment(&session).unwrap();
        let (pid, ticks) = current_identity();

        assert_eq!(
            read_session_report(generation, pid, ticks).unwrap(),
            Some(session)
        );
        assert!(read_session_report(generation, pid + 1, ticks).is_err());
        assert!(read_session_report(generation, pid, ticks + 1).is_err());
        let other = super::super::ProtocolId::generate().unwrap().to_string();
        assert_eq!(read_session_report(&other, pid, ticks).unwrap(), None);
    });
}

#[test]
fn an_untrusted_report_is_ignored_and_still_removed() {
    as_daemon_overlay(Some("1"), |generation| {
        let (pid, ticks) = current_identity();
        let name = format!("{generation}.target");
        let other = super::super::ProtocolId::generate().unwrap().to_string();
        let foreign = record_bytes(&other, pid, ticks, None);
        let relative = record_bytes(generation, pid, ticks, Some("b.wayscriber-session"));
        let mut newer_schema =
            String::from_utf8(record_bytes(generation, pid, ticks, None)).unwrap();
        newer_schema = newer_schema.replace(&format!("\"schema\":{REPORT_SCHEMA}"), "\"schema\":2");
        let oversize = vec![b' '; MAX_REPORT_BYTES + 1];
        let valid = record_bytes(generation, pid, ticks, None);

        for (case, bytes, mode) in [
            ("malformed", b"{".as_slice(), 0o600),
            ("another generation's record", &foreign, 0o600),
            ("relative target", &relative, 0o600),
            ("newer schema", newer_schema.as_bytes(), 0o600),
            ("oversize", &oversize, 0o600),
            ("readable by others", &valid, 0o644),
        ] {
            let path = write_entry(&name, bytes, mode);

            assert_eq!(
                take_final_session_report(generation, pid, ticks),
                None,
                "{case}"
            );
            assert!(!path.exists(), "{case}");
        }

        let target = write_entry("elsewhere", &valid, 0o600);
        std::os::unix::fs::symlink(&target, report_path(generation)).unwrap();
        assert_eq!(take_final_session_report(generation, pid, ticks), None);
        assert!(std::fs::symlink_metadata(report_path(generation)).is_err());
        assert!(target.exists(), "only the report's own name is removed");
    });
}

#[test]
fn retirement_removes_a_childs_report_and_its_writer_temporaries() {
    as_daemon_overlay(Some("1"), |generation| {
        let (pid, ticks) = current_identity();
        let report = write_entry(
            &format!("{generation}.target"),
            &record_bytes(
                generation,
                pid,
                ticks,
                Some("/sessions/b.wayscriber-session"),
            ),
            0o600,
        );
        let temporary = write_entry(&format!(".{generation}.target.1.2.3.tmp"), b"{", 0o600);
        let other = super::super::ProtocolId::generate().unwrap().to_string();
        let other_report = write_entry(&format!("{other}.target"), b"{", 0o600);

        assert_eq!(
            take_final_session_report(generation, pid, ticks),
            Some(ReportedSession::Named(PathBuf::from(
                "/sessions/b.wayscriber-session"
            )))
        );
        assert!(!report.exists());
        assert!(!temporary.exists());
        assert!(other_report.exists());
    });
}

#[test]
fn startup_removes_every_stale_report_and_nothing_else() {
    as_daemon_overlay(None, |generation| {
        let stale = [
            write_entry(&format!("{generation}.target"), b"{", 0o600),
            write_entry(&format!(".{generation}.target.1.2.3.tmp"), b"{", 0o600),
        ];
        let unrelated = [
            write_entry("notes.txt", b"kept", 0o600),
            write_entry("not-an-id.target", b"kept", 0o600),
        ];

        clear_stale_session_reports().unwrap();

        assert!(stale.iter().all(|path| !path.exists()));
        assert!(unrelated.iter().all(|path| path.exists()));
    });
}
