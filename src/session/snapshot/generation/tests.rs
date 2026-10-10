use super::marker::read_marker_generation;
use super::probe::{
    PROBE_READ_BYTES, PROBE_SCAN_BYTES, inflate_probe_prefix, probe_payload_header,
    read_probe_window,
};

use super::*;
use crate::session::snapshot::{load, save, tests::sample_snapshot};
use crate::session::{CompressionMode, SessionOptions};
use crate::test_temp::tempdir;
use std::fs::{self, File};
use std::io::{Seek, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};

#[test]
fn ordering_preserves_each_legacy_rule_and_known_generations_override_clocks() {
    let time = |n| Some(SystemTime::UNIX_EPOCH + Duration::from_secs(n));
    for a_gen in [Generation::Unknown, Generation::Known(1)] {
        for b_gen in [Generation::Unknown, Generation::Known(2)] {
            for a_time in [None, time(1), time(2), time(3)] {
                for b_time in [None, time(1), time(2), time(3)] {
                    let a = ArtifactStamp {
                        generation: a_gen,
                        modified: a_time,
                    };
                    let b = ArtifactStamp {
                        generation: b_gen,
                        modified: b_time,
                    };
                    let known =
                        matches!((a_gen, b_gen), (Generation::Known(_), Generation::Known(_)));
                    let strict = match (a_time, b_time) {
                        (Some(a), Some(b)) => a > b,
                        _ => false,
                    };
                    let inclusive = match (a_time, b_time) {
                        (Some(a), Some(b)) => a >= b,
                        _ => true,
                    };
                    assert_eq!(written_after(a, b, LegacyOrder::Strict), !known && strict);
                    assert_eq!(
                        written_after(a, b, LegacyOrder::NonStrictTrue),
                        !known && inclusive
                    );
                    assert_eq!(cleared_by(a, b), known || !strict);
                }
            }
        }
    }
    let same = ArtifactStamp {
        generation: Generation::Known(2),
        modified: None,
    };
    assert!(
        !cleared_by(same, same),
        "a clear boundary preserves tool state written by the same save"
    );
}

#[test]
fn bounded_header_probe_rejects_ambiguous_or_non_integer_generations() {
    for header in [
        r#"{"version":7,"save_generation":42,"boards":[]}"#,
        r#"{"save_generation":42}"#,
        r#"{"name":"x","time":"y","version":7,"save_generation":42}"#,
    ] {
        assert_eq!(
            payload_prefix_generation(header.as_bytes()),
            Generation::Known(42)
        );
    }
    for value in [
        "0",
        "9007199254740992",
        "12345678901234567",
        "042",
        "-1",
        "4.2",
        "4e1",
        r#""42""#,
        "null",
        "{}",
    ] {
        let header = format!("{{\"version\":7,\"save_generation\":{value},\"boards\":[]}}");
        assert_eq!(
            payload_prefix_generation(header.as_bytes()),
            Generation::Unknown,
            "{header}"
        );
    }
    for header in [
        r#"{"save_generation":42,"save_generation":43}"#,
        r#"{"a":{},"save_generation":42}"#,
        r#"{"a":[],"save_generation":42}"#,
        r#"{"a":"x\ny","save_generation":42}"#,
        r#"{"na\me":"x","save_generation":42}"#,
        r#"{"a":"x","b":"x","c":"x","d":"x","save_generation":42}"#,
        r#""save_generation":42"#,
        "",
        r#"{"version":1,"transparent":{}}"#,
    ] {
        assert_eq!(
            payload_prefix_generation(header.as_bytes()),
            Generation::Unknown,
            "{header}"
        );
    }
    let long = format!("{{\"a\":\"{}\",\"save_generation\":42}}", "x".repeat(257));
    assert_eq!(
        payload_prefix_generation(long.as_bytes()),
        Generation::Unknown
    );
}

#[test]
fn probes_are_bounded_and_do_not_consume_file_position() {
    let temp = tempdir().unwrap();
    let mut file = File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .open(temp.path().join("large"))
        .unwrap();
    file.write_all(b"{\"version\":7,\"save_generation\":42,")
        .unwrap();
    file.set_len(64 * 1024 * 1024).unwrap();
    file.rewind().unwrap();
    assert_eq!(probe_payload_header(&file).unwrap(), Generation::Known(42));
    assert_eq!(file.stream_position().unwrap(), 0);
    assert_eq!(read_probe_window(&file).unwrap().len(), PROBE_READ_BYTES);

    let mut raw = b"{\"version\":7,\"save_generation\":42,".to_vec();
    raw.resize(1024 * 1024, b' ');
    let mut encoder = flate2::GzBuilder::new()
        .filename("session.json")
        .extra(vec![1, 2, 3])
        .write(Vec::new(), flate2::Compression::default());
    encoder.write_all(&raw).unwrap();
    let gzip = encoder.finish().unwrap();
    assert_eq!(payload_prefix_generation(&gzip), Generation::Known(42));
    assert_eq!(inflate_probe_prefix(&gzip).len(), PROBE_SCAN_BYTES);
    assert_eq!(
        payload_prefix_generation(&gzip[..200.min(gzip.len())]),
        Generation::Known(42)
    );
}

#[test]
fn marker_format_round_trips_and_rejects_wrong_kind_or_version() {
    assert_eq!(marker_record_bytes(MarkerKind::Cleared,42,"2026-09-29T10:15:02Z".into()).unwrap(),b"{\"format\":1,\"kind\":\"cleared\",\"generation\":42,\"written\":\"2026-09-29T10:15:02Z\"}\n");
    let temp = tempdir().unwrap();
    let path = temp.path().join("marker");
    for kind in [
        MarkerKind::Cleared,
        MarkerKind::BackupRecoverable,
        MarkerKind::RecoveryRecoverable,
    ] {
        fs::write(&path, marker_record_bytes(kind, 42, String::new()).unwrap()).unwrap();
        assert_eq!(
            read_marker_generation(&File::open(&path).unwrap(), kind).unwrap(),
            Generation::Known(42)
        );
    }
    for bytes in [
        b"2026-09-29T10:15:02Z".to_vec(),
        br#"{"format":2,"kind":"cleared","generation":42}"#.to_vec(),
        br#"{"format":1,"kind":"backup-recoverable","generation":42}"#.to_vec(),
        br#"{"format":1,"kind":"cleared","generation":0}"#.to_vec(),
        vec![b' '; 1025],
    ] {
        fs::write(&path, bytes).unwrap();
        assert_eq!(
            read_marker_generation(&File::open(&path).unwrap(), MarkerKind::Cleared).unwrap(),
            Generation::Unknown
        );
    }
}

fn options(path: &std::path::Path) -> SessionOptions {
    let mut options = SessionOptions::new(path.to_path_buf(), "generation");
    options.persist_transparent = true;
    options.compression = CompressionMode::Off;
    options
}
fn generation(path: std::path::PathBuf) -> Generation {
    payload_prefix_generation(&fs::read(path).unwrap())
}

#[test]
fn writers_allocate_across_rotated_payloads_and_marker_records() {
    let temp = tempdir().unwrap();
    let options = options(temp.path());
    let snapshot = sample_snapshot();
    assert_eq!(
        save::save_snapshot_with_report(&snapshot, &options)
            .unwrap()
            .unwrap()
            .generation,
        Some(1)
    );
    assert_eq!(
        save::save_snapshot_with_report(&snapshot, &options)
            .unwrap()
            .unwrap()
            .generation,
        Some(2)
    );
    assert_eq!(generation(options.backup_file_path()), Generation::Known(1));
    fs::write(
        options.clear_marker_file_path(),
        marker_record_bytes(MarkerKind::Cleared, 70, String::new()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        save::save_snapshot_with_report(&snapshot, &options)
            .unwrap()
            .unwrap()
            .generation,
        Some(71)
    );
    assert_eq!(
        generation(options.session_file_path()),
        Generation::Known(71)
    );
    let empty = super::super::types::SessionSnapshot {
        active_board_id: "transparent".into(),
        boards: vec![],
        tool_state: None,
    };
    let report = save::save_snapshot_with_report(&empty, &options)
        .unwrap()
        .unwrap();
    assert_eq!(report.generation, Some(72));
    assert_eq!(
        read_marker_generation(
            &File::open(options.clear_marker_file_path()).unwrap(),
            MarkerKind::Cleared
        )
        .unwrap(),
        Generation::Known(72)
    );
}

#[test]
fn ceiling_drops_generation_and_plain_estimates_bound_actual_size() {
    let temp = tempdir().unwrap();
    let options = options(temp.path());
    let snapshot = sample_snapshot();
    let estimate = save::estimate_snapshot_save(&snapshot, &options).unwrap();
    let report = save::save_snapshot_with_report(&snapshot, &options)
        .unwrap()
        .unwrap();
    assert!(estimate.full.written_size >= report.written_size);
    assert!(estimate.full.written_size - report.written_size <= 15);
    fs::write(
        options.backup_file_path(),
        format!("{{\"version\":7,\"save_generation\":{MAX_GENERATION}}}"),
    )
    .unwrap();
    let report = save::save_snapshot_with_report(&snapshot, &options)
        .unwrap()
        .unwrap();
    assert_eq!(report.generation, None);
    assert_eq!(generation(options.session_file_path()), Generation::Unknown);
}

#[test]
fn concurrent_writers_allocate_in_exclusive_lock_order() {
    let temp = tempdir().unwrap();
    let options = options(temp.path());
    let mut threads = vec![];
    for _ in 0..2 {
        let options = options.clone();
        threads.push(std::thread::spawn(move || {
            (0..20)
                .map(|_| {
                    save::save_snapshot_with_report(&sample_snapshot(), &options)
                        .unwrap()
                        .unwrap()
                        .generation
                        .unwrap()
                })
                .collect::<Vec<_>>()
        }));
    }
    let mut all = vec![];
    for thread in threads {
        let values = thread.join().unwrap();
        assert!(values.windows(2).all(|p| p[0] < p[1]));
        all.extend(values);
    }
    all.sort();
    assert_eq!(all, (1..=40).collect::<Vec<_>>());
}

#[test]
fn malformed_optional_generation_never_quarantines_a_valid_snapshot() {
    let temp = tempdir().unwrap();
    let options = options(temp.path());
    save::save_snapshot_with_report(&sample_snapshot(), &options).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(options.session_file_path()).unwrap()).unwrap();
    for invalid in [
        serde_json::json!("x"),
        serde_json::json!(-1),
        serde_json::json!(1.5),
        serde_json::json!(null),
        serde_json::json!({}),
        serde_json::json!(MAX_GENERATION + 1),
    ] {
        value["save_generation"] = invalid;
        fs::write(
            options.session_file_path(),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            load::load_snapshot_with_expanded_limit(
                &options,
                super::super::compression::DEFAULT_MAX_EXPANDED_SESSION_BYTES
            )
            .unwrap(),
            load::LoadSnapshotOutcome::Loaded(_)
        ));
        assert!(options.session_file_path().exists());
        assert!(!options.backup_file_path().exists());
    }
}

#[test]
fn blank_recovery_and_clear_boundary_markers_share_their_payload_generation() {
    let temp = tempdir().unwrap();
    let mut options = options(temp.path());
    let snapshot = sample_snapshot();
    save::save_snapshot_with_report(&snapshot, &options).unwrap();
    let mut blank = snapshot.clone();
    blank.boards.clear();
    blank.tool_state = Some(super::super::tests::sample_tool_state());
    let blank_report = save::save_snapshot_with_report(&blank, &options)
        .unwrap()
        .unwrap();
    assert_eq!(blank_report.generation, Some(2));
    assert_eq!(generation(options.backup_file_path()), Generation::Known(1));
    assert_eq!(
        read_marker_generation(
            &File::open(options.backup_recovery_marker_file_path()).unwrap(),
            MarkerKind::BackupRecoverable
        )
        .unwrap(),
        Generation::Known(2)
    );

    let boundary = save::save_snapshot_with_report_and_clear_boundary(&blank, &options, true)
        .unwrap()
        .unwrap();
    assert_eq!(boundary.generation, Some(3));
    assert_eq!(
        generation(options.session_file_path()),
        Generation::Known(3)
    );
    assert_eq!(
        read_marker_generation(
            &File::open(options.clear_marker_file_path()).unwrap(),
            MarkerKind::Cleared
        )
        .unwrap(),
        Generation::Known(3)
    );
    assert!(!options.backup_file_path().exists());

    options.max_file_size_bytes = 64;
    assert!(save::save_snapshot_with_report(&snapshot, &options).is_err());
    assert_eq!(
        generation(options.recovery_file_path()),
        Generation::Known(4)
    );
    fs::remove_file(options.session_file_path()).unwrap();
    options.max_file_size_bytes = 50 * 1024 * 1024;
    let report = save::save_snapshot_with_report(&blank, &options)
        .unwrap()
        .unwrap();
    assert_eq!(report.generation, Some(5));
    assert_eq!(
        read_marker_generation(
            &File::open(options.recovery_recoverable_marker_file_path()).unwrap(),
            MarkerKind::RecoveryRecoverable
        )
        .unwrap(),
        Generation::Known(5)
    );
}

#[test]
fn save_as_and_gzip_preserve_the_allocated_header_generation() {
    let temp = tempdir().unwrap();
    let mut options = options(temp.path());
    options.set_named_file_target(temp.path().join("named.wayscriber-session"));
    options.compression = CompressionMode::On;
    fs::write(
        options.clear_marker_file_path(),
        marker_record_bytes(MarkerKind::Cleared, 90, String::new()).unwrap(),
    )
    .unwrap();
    let report = save::save_snapshot_as_with_report(
        &sample_snapshot(),
        &options,
        save::SaveAsOverwrite::ConfirmReplace,
    )
    .unwrap();
    assert_eq!(report.generation, Some(91));
    assert!(report.compressed);
    assert_eq!(
        generation(options.session_file_path()),
        Generation::Known(91)
    );
    assert!(!options.clear_marker_file_path().exists());
}

#[test]
fn fifo_marker_probe_never_blocks_and_oversized_generations_do_not_raise_the_sequence() {
    let temp = tempdir().unwrap();
    let options = options(temp.path());
    super::super::tests::make_fifo(&options.clear_marker_file_path());
    assert_eq!(
        ArtifactSetView::probe(&options)
            .next_generation(UnreadablePolicy::Refuse)
            .unwrap(),
        Some(1)
    );
    fs::remove_file(options.clear_marker_file_path()).unwrap();
    fs::write(
        options.clear_marker_file_path(),
        b"{\"format\":1,\"kind\":\"cleared\",\"generation\":1152921504606846976}",
    )
    .unwrap();
    assert_eq!(
        save::save_snapshot_with_report(&sample_snapshot(), &options)
            .unwrap()
            .unwrap()
            .generation,
        Some(1)
    );
}

#[test]
fn writer_header_restart_output_isolation_and_clear_reset_are_stable() {
    let temp = tempdir().unwrap();
    let mut first = options(temp.path());
    first.per_output = true;
    first.set_output_identity(Some("DP-1"));
    let mut second = first.clone();
    second.set_output_identity(Some("DP-2"));
    let snapshot = sample_snapshot();
    let report = save::save_snapshot_with_report(&snapshot, &first)
        .unwrap()
        .unwrap();
    assert_eq!(report.generation, Some(1));
    assert!(
        fs::read(first.session_file_path())
            .unwrap()
            .starts_with(b"{\n  \"version\": 8,\n  \"save_generation\": 1,")
    );
    // Reconstruct options rather than retaining any writer state across saves.
    let mut restarted = options(temp.path());
    restarted.per_output = true;
    restarted.set_output_identity(Some("DP-1"));
    assert_eq!(
        save::save_snapshot_with_report(&snapshot, &restarted)
            .unwrap()
            .unwrap()
            .generation,
        Some(2)
    );
    assert_eq!(
        save::save_snapshot_with_report(&snapshot, &second)
            .unwrap()
            .unwrap()
            .generation,
        Some(1)
    );
    crate::session::clear_session(&first).unwrap();
    assert_eq!(
        save::save_snapshot_with_report(&snapshot, &first)
            .unwrap()
            .unwrap()
            .generation,
        Some(1)
    );
    assert_eq!(generation(second.session_file_path()), Generation::Known(1));
}

#[test]
fn each_marker_slot_advances_the_next_real_save() {
    for kind in [
        MarkerKind::Cleared,
        MarkerKind::BackupRecoverable,
        MarkerKind::RecoveryRecoverable,
    ] {
        let temp = tempdir().unwrap();
        let options = options(temp.path());
        let path = match kind {
            MarkerKind::Cleared => options.clear_marker_file_path(),
            MarkerKind::BackupRecoverable => options.backup_recovery_marker_file_path(),
            MarkerKind::RecoveryRecoverable => options.recovery_recoverable_marker_file_path(),
        };
        fs::write(path, marker_record_bytes(kind, 42, String::new()).unwrap()).unwrap();
        let report = save::save_snapshot_with_report(&sample_snapshot(), &options)
            .unwrap()
            .unwrap();
        assert_eq!(report.generation, Some(43), "{kind:?}");
        assert_eq!(
            generation(options.session_file_path()),
            Generation::Known(43)
        );
    }
}

#[test]
fn socket_and_symlink_sidecars_are_classified_without_blocking_autosave() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    for slot in ["clear", "recovery", "backup"] {
        for kind in ["socket", "socket link", "loop", "regular link"] {
            let short_root = [Path::new("/tmp"), Path::new(".")]
                .into_iter()
                .find(|path| path.is_dir())
                .unwrap();
            let temp = tempfile::Builder::new()
                .prefix("ws-")
                .tempdir_in(short_root)
                .unwrap();
            let options = options(temp.path());
            fs::create_dir_all(&options.base_dir).unwrap();
            let path = match slot {
                "clear" => options.clear_marker_file_path(),
                "recovery" => options.recovery_file_path(),
                _ => options.backup_file_path(),
            };
            let socket_path = temp.path().join("s");
            let listener = if kind == "socket" || kind == "socket link" {
                let listener = UnixListener::bind(&socket_path).unwrap();
                if kind == "socket" {
                    fs::rename(&socket_path, &path).unwrap();
                } else {
                    symlink(&socket_path, &path).unwrap();
                }
                Some(listener)
            } else {
                None
            };
            if kind == "loop" {
                symlink(&path, &path).unwrap();
            }
            if kind == "regular link" {
                let bytes = if slot == "clear" {
                    marker_record_bytes(MarkerKind::Cleared, 42, String::new()).unwrap()
                } else {
                    br#"{"version":7,"save_generation":42,"boards":[]}"#.to_vec()
                };
                let regular = temp.path().join("regular");
                fs::write(&regular, bytes).unwrap();
                symlink(&regular, &path).unwrap();
            }
            let report = save::save_snapshot_autosave_with_report(&sample_snapshot(), &options)
                .unwrap()
                .unwrap();
            let expected = if kind == "regular link" { 43 } else { 1 };
            assert_eq!(report.generation, Some(expected), "{slot}, {kind}");
            assert_eq!(
                generation(options.session_file_path()),
                Generation::Known(expected)
            );
            drop(listener);
        }
    }
}

#[test]
fn unreadable_regular_sidecars_follow_autosave_and_normal_save_policy() {
    use std::os::unix::fs::PermissionsExt;
    for slot in ["clear", "recovery", "backup"] {
        let temp = tempdir().unwrap();
        let options = options(temp.path());
        save::save_snapshot_with_report(&sample_snapshot(), &options).unwrap();
        save::save_snapshot_with_report(&sample_snapshot(), &options).unwrap();
        let primary = fs::read(options.session_file_path()).unwrap();
        let backup = fs::read(options.backup_file_path()).unwrap();
        let path = if slot == "clear" {
            options.clear_marker_file_path()
        } else if slot == "recovery" {
            options.recovery_file_path()
        } else {
            options.backup_file_path()
        };
        if slot != "backup" {
            fs::write(&path, b"regular sidecar").unwrap();
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        // Root and privileged runners can bypass chmod; they cannot prove this denial.
        if File::open(&path).is_ok() {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            eprintln!("Skipping chmod-denial contract: runner can read mode-000 files");
            return;
        }
        let result = save::save_snapshot_autosave_with_report(&sample_snapshot(), &options);
        if slot == "backup" {
            result.expect("an unreadable backup cannot hide newer recovery or clear data");
            assert_eq!(fs::read(options.backup_file_path()).unwrap(), primary);
            continue;
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let error = result.unwrap_err();
        assert!(
            format!("{error:#}").contains(&path.display().to_string()),
            "{error:#}"
        );
        assert_eq!(fs::read(options.session_file_path()).unwrap(), primary);
        assert_eq!(fs::read(options.backup_file_path()).unwrap(), backup);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        let normal = save::save_snapshot_with_report(&sample_snapshot(), &options);
        if path.exists() {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        normal.expect("normal save warns and continues for unreadable generation sidecars");
    }
}
