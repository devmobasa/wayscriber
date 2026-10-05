//! Files written by real historical writers, without edits to the artifact bytes.
//! Sources: tag v0.9.25 and commit a2bb723a. The generator below was added to
//! each archive as an integration test and run alone with
//! `cargo test --locked --no-default-features -- --ignored --exact --test-threads=1`.
//! COMPAT_FIXTURE_OUT selected an empty output directory. The v0.9.25 generator omits
//! BoardSnapshot.appearance because that revision does not have the field.
//!
//! Historical fixture generator:
//! ```rust,ignore
//! use std::fs;
//! use std::path::{Path, PathBuf};
//! use wayscriber::draw::{Color, Frame, Shape};
//! use wayscriber::session::{
//!     BoardPagesSnapshot, BoardSnapshot, CompressionMode, SessionOptions, SessionSnapshot,
//!     ToolStateSnapshot, save_snapshot,
//! };
//!
//! const TOOL_STATE: &str = r#"{"current_color":{"r":0.0,"g":0.0,"b":1.0,"a":1.0},"current_thickness":3.0,"current_font_size":24.0,"text_background_enabled":false,"arrow_length":20.0,"arrow_angle":30.0,"board_previous_color":null}"#;
//!
//! fn lines(count: i32) -> SessionSnapshot {
//!     let mut frame = Frame::new();
//!     for index in 0..count {
//!         frame.add_shape(Shape::Line {
//!             x1: 0,
//!             y1: index * 10,
//!             x2: 100,
//!             y2: index * 10,
//!             color: Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 },
//!             thick: 2.0,
//!         });
//!     }
//!     SessionSnapshot {
//!         active_board_id: "transparent".to_string(),
//!         boards: vec![BoardSnapshot {
//!             id: "transparent".to_string(),
//!             appearance: None,
//!             pages: BoardPagesSnapshot { pages: vec![frame], active: 0 },
//!         }],
//!         tool_state: None,
//!     }
//! }
//!
//! fn blank() -> SessionSnapshot {
//!     let tool_state: ToolStateSnapshot = serde_json::from_str(TOOL_STATE).unwrap();
//!     SessionSnapshot { active_board_id: "transparent".to_string(), boards: Vec::new(), tool_state: Some(tool_state) }
//! }
//!
//! fn cleared() -> SessionSnapshot {
//!     SessionSnapshot { active_board_id: "transparent".to_string(), boards: Vec::new(), tool_state: None }
//! }
//!
//! fn options(root: &Path, name: &str, compression: CompressionMode) -> SessionOptions {
//!     let mut options = SessionOptions::new(root.join(name), "fixture");
//!     options.persist_transparent = true;
//!     options.restore_tool_state = true;
//!     options.per_output = false;
//!     options.compression = compression;
//!     options
//! }
//!
//! fn keep(from: PathBuf, out: &Path, name: &str) {
//!     fs::copy(&from, out.join(name)).unwrap_or_else(|err| panic!("{}: {err}", from.display()));
//! }
//!
//! #[test]
//! #[ignore = "writes the compatibility fixtures; run explicitly"]
//! fn generate_compat_fixtures() {
//!     let out = PathBuf::from(std::env::var("COMPAT_FIXTURE_OUT").expect("COMPAT_FIXTURE_OUT"));
//!     let root = out.join("work");
//!     fs::create_dir_all(&root).unwrap();
//!
//!     let plain = options(&root, "plain", CompressionMode::Off);
//!     save_snapshot(&lines(1), &plain).unwrap();
//!     keep(plain.session_file_path(), &out, "lines-1.json");
//!     save_snapshot(&lines(2), &plain).unwrap();
//!     keep(plain.session_file_path(), &out, "lines-2.json");
//!     keep(plain.backup_file_path(), &out, "backup-lines-1.json");
//!
//!     let gzip = options(&root, "gzip", CompressionMode::On);
//!     save_snapshot(&lines(4), &gzip).unwrap();
//!     keep(gzip.session_file_path(), &out, "lines-4.json.gz");
//!
//!     let blank_set = options(&root, "blank", CompressionMode::Off);
//!     save_snapshot(&lines(1), &blank_set).unwrap();
//!     save_snapshot(&blank(), &blank_set).unwrap();
//!     keep(blank_set.session_file_path(), &out, "blank.json");
//!     keep(blank_set.backup_recovery_marker_file_path(), &out, "backup-recoverable.marker");
//!
//!     let mut recovery = options(&root, "recovery", CompressionMode::Off);
//!     recovery.max_file_size_bytes = 64;
//!     save_snapshot(&lines(3), &recovery).expect_err("the oversized save must fail");
//!     keep(recovery.recovery_file_path(), &out, "recovery-lines-3.json");
//!     recovery.max_file_size_bytes = 50 * 1024 * 1024;
//!     save_snapshot(&blank(), &recovery).unwrap();
//!     keep(recovery.recovery_recoverable_marker_file_path(), &out, "recovery-recoverable.marker");
//!
//!     let clear = options(&root, "clear", CompressionMode::Off);
//!     save_snapshot(&lines(1), &clear).unwrap();
//!     save_snapshot(&cleared(), &clear).unwrap();
//!     keep(clear.clear_marker_file_path(), &out, "cleared.marker");
//!
//!     fs::remove_dir_all(&root).unwrap();
//! }
//! ```
use super::ToolStateSnapshot;
use super::types::BoardFile;
use super::{
    compression::DEFAULT_MAX_EXPANDED_SESSION_BYTES,
    generation::{self, Generation},
    load::{self, LoadSnapshotOutcome},
    save,
    tests::{sample_snapshot, set_modified},
};
use crate::draw::Frame;
use crate::session::{CompressionMode, SessionOptions};
use crate::test_temp::tempdir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

macro_rules! fixtures {
    ($era:literal) => {
        [
            include_bytes!(concat!("fixtures/compat/", $era, "/lines-1.json")).as_slice(),
            include_bytes!(concat!("fixtures/compat/", $era, "/lines-2.json")).as_slice(),
            include_bytes!(concat!("fixtures/compat/", $era, "/lines-4.json.gz")).as_slice(),
            include_bytes!(concat!("fixtures/compat/", $era, "/recovery-lines-3.json")).as_slice(),
            include_bytes!(concat!("fixtures/compat/", $era, "/backup-lines-1.json")).as_slice(),
            include_bytes!(concat!("fixtures/compat/", $era, "/blank.json")).as_slice(),
            include_bytes!(concat!("fixtures/compat/", $era, "/cleared.marker")).as_slice(),
            include_bytes!(concat!(
                "fixtures/compat/",
                $era,
                "/backup-recoverable.marker"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "fixtures/compat/",
                $era,
                "/recovery-recoverable.marker"
            ))
            .as_slice(),
        ]
    };
}

const ERAS: [(&str, [&[u8]; 9]); 2] = [
    ("v0.9.25", fixtures!("v0.9.25")),
    ("main-a2bb723a", fixtures!("main-a2bb723a")),
];

fn line_count(snapshot: &super::SessionSnapshot) -> usize {
    snapshot
        .boards
        .iter()
        .flat_map(|b| &b.pages.pages)
        .map(|page| page.shapes.len())
        .sum()
}

fn options(path: &Path) -> SessionOptions {
    let mut options = SessionOptions::new(path.to_path_buf(), "compat");
    options.persist_transparent = true;
    options.restore_tool_state = true;
    options.per_output = false;
    options.compression = CompressionMode::Off;
    options
}

fn files(path: &Path, include_lock: bool) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file() && (include_lock || !p.to_string_lossy().ends_with(".lock")))
        .map(|p| {
            (
                p.file_name().unwrap().to_str().unwrap().to_string(),
                fs::read(p).unwrap(),
            )
        })
        .collect()
}

#[test]
fn real_legacy_fixtures_decode_and_carry_no_generation() {
    for (era, fixtures) in ERAS {
        let temp = tempdir().unwrap();
        let options = options(temp.path());
        for (index, expected) in [(0, 1), (1, 2), (2, 4), (3, 3), (4, 1), (5, 0)] {
            fs::write(options.session_file_path(), fixtures[index]).unwrap();
            let loaded = load::load_snapshot_inner(&options.session_file_path(), &options)
                .unwrap()
                .unwrap();
            assert_eq!(
                line_count(&loaded.snapshot),
                expected,
                "{era}, fixture {index}"
            );
            assert_eq!(loaded.compressed, index == 2);
            assert_eq!(
                loaded.version,
                if era == "v0.9.25" { 6 } else { 7 },
                "{era}"
            );
            assert_eq!(
                generation::payload_prefix_generation(fixtures[index]),
                Generation::Unknown
            );
            if index == 5 {
                assert!(loaded.snapshot.tool_state.is_some());
            }
        }
        for (index, kind) in [
            (6, generation::MarkerKind::Cleared),
            (7, generation::MarkerKind::BackupRecoverable),
            (8, generation::MarkerKind::RecoveryRecoverable),
        ] {
            assert_eq!(fixtures[index].len(), 20, "legacy timestamp marker {era}");
            let path = match kind {
                generation::MarkerKind::Cleared => options.clear_marker_file_path(),
                generation::MarkerKind::BackupRecoverable => {
                    options.backup_recovery_marker_file_path()
                }
                generation::MarkerKind::RecoveryRecoverable => {
                    options.recovery_recoverable_marker_file_path()
                }
            };
            fs::write(path, fixtures[index]).unwrap();
            // Allocation uses the real marker reader as part of the artifact view.
            assert_eq!(
                generation::ArtifactSetView::probe(&options)
                    .next_generation(generation::UnreadablePolicy::Warn)
                    .unwrap(),
                Some(1),
                "{era} {kind:?}"
            );
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Expect {
    Empty,
    Primary(usize),
    Backup(usize),
    Recovery(usize),
    Blank,
}

fn classify(outcome: LoadSnapshotOutcome) -> Expect {
    match outcome {
        LoadSnapshotOutcome::Empty => Expect::Empty,
        LoadSnapshotOutcome::Loaded(snapshot) if !snapshot.has_board_data() => Expect::Blank,
        LoadSnapshotOutcome::Loaded(snapshot) => Expect::Primary(line_count(&snapshot)),
        LoadSnapshotOutcome::LoadedFromBackup(snapshot) => Expect::Backup(line_count(&snapshot)),
        LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => {
            Expect::Recovery(line_count(&snapshot))
        }
        other => panic!("unexpected fixture result: {other:?}"),
    }
}

#[derive(Clone, Copy)]
enum Slot {
    P,
    B,
    R,
    C,
    Br,
    Rr,
}

impl Slot {
    fn path(self, o: &SessionOptions) -> std::path::PathBuf {
        match self {
            Self::P => o.session_file_path(),
            Self::B => o.backup_file_path(),
            Self::R => o.recovery_file_path(),
            Self::C => o.clear_marker_file_path(),
            Self::Br => o.backup_recovery_marker_file_path(),
            Self::Rr => o.recovery_recoverable_marker_file_path(),
        }
    }
}

#[test]
fn legacy_precedence_matrix_remains_identical_on_runtime_and_read_only_candidates() {
    use Expect::*;
    use Slot::*;
    type Scenario = (&'static str, &'static [(Slot, usize)], [Expect; 4]);
    let scenarios: [Scenario; 7] = [
        ("S1", &[(C, 6), (P, 1)], [Primary(2), Empty, Empty, Empty]),
        (
            "S2",
            &[(P, 2), (R, 3)],
            [Recovery(3), Recovery(3), Primary(4), Recovery(3)],
        ),
        (
            "S3",
            &[(R, 3), (P, 1)],
            [Primary(2), Recovery(3), Recovery(3), Recovery(3)],
        ),
        (
            "S4",
            &[(B, 4), (P, 1), (C, 6)],
            [Empty, Empty, Primary(2), Empty],
        ),
        ("S5", &[(B, 4), (P, 5)], [Blank, Blank, Backup(1), Blank]),
        (
            "S6",
            &[(C, 6), (B, 4), (Br, 7), (P, 5)],
            [Backup(1), Blank, Blank, Blank],
        ),
        (
            "S7",
            &[(C, 6), (R, 3), (Rr, 8), (P, 5)],
            [Recovery(3), Blank, Blank, Blank],
        ),
    ];
    for (era, fixtures) in ERAS {
        for (scenario, slots, expected) in scenarios {
            for (assignment, expect) in expected.into_iter().enumerate() {
                for named_candidate in [false, true] {
                    let temp = tempdir().unwrap();
                    let mut options = options(temp.path());
                    if named_candidate {
                        options.set_named_file_target(temp.path().join("board.wayscriber-session"));
                    }
                    for (i, (slot, fixture)) in slots.iter().enumerate() {
                        let path = slot.path(&options);
                        fs::write(&path, fixtures[*fixture]).unwrap();
                        let seconds = match assignment {
                            0 => i as u64 * 3,
                            1 => 0,
                            2 => (slots.len() - i) as u64 * 3,
                            _ => i as u64 / 2 * 2,
                        };
                        set_modified(&path, SystemTime::UNIX_EPOCH + Duration::from_secs(seconds));
                    }
                    let before = files(temp.path(), named_candidate);
                    let result = if named_candidate {
                        load::load_named_session_candidate_with_expanded_limit(
                            &options,
                            DEFAULT_MAX_EXPANDED_SESSION_BYTES,
                        )
                    } else {
                        load::load_snapshot_with_expanded_limit(
                            &options,
                            DEFAULT_MAX_EXPANDED_SESSION_BYTES,
                        )
                    }
                    .unwrap();
                    assert_eq!(
                        classify(result),
                        expect,
                        "{era}, {scenario}, assignment {assignment}, candidate={named_candidate}"
                    );
                    assert_eq!(
                        files(temp.path(), named_candidate),
                        before,
                        "read path must preserve fixture bytes {era}, {scenario}"
                    );
                }
            }
        }
    }
}

#[test]
fn legacy_clear_cleanup_retains_only_backups_strictly_newer_than_marker() {
    for (era, fixtures) in ERAS {
        for assignment in 0..4 {
            let temp = tempdir().unwrap();
            let options = options(temp.path());
            fs::write(options.backup_file_path(), fixtures[4]).unwrap();
            fs::write(options.clear_marker_file_path(), fixtures[6]).unwrap();
            let (backup, clear) = match assignment {
                0 => (0, 3),
                1 | 3 => (0, 0),
                _ => (3, 0),
            };
            set_modified(
                &options.backup_file_path(),
                SystemTime::UNIX_EPOCH + Duration::from_secs(backup),
            );
            set_modified(
                &options.clear_marker_file_path(),
                SystemTime::UNIX_EPOCH + Duration::from_secs(clear),
            );
            save::save_snapshot_with_report(&sample_snapshot(), &options).unwrap();
            assert!(!options.clear_marker_file_path().exists());
            assert_eq!(
                options.backup_file_path().exists(),
                assignment == 2,
                "{era} assignment {assignment}"
            );
            assert_eq!(
                line_count(
                    &load::load_snapshot_inner(&options.session_file_path(), &options)
                        .unwrap()
                        .unwrap()
                        .snapshot
                ),
                1
            );
        }
    }
}

// Verbatim pre-generation SessionFile schema from a2bb723a, renamed for this test.
#[derive(Debug, Serialize, Deserialize)]
struct LegacySessionFileA2bb723a {
    #[serde(default = "legacy_file_version")]
    pub version: u32,
    pub last_modified: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_board_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boards: Vec<BoardFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent: Option<Frame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whiteboard: Option<Frame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blackboard: Option<Frame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent_pages: Option<Vec<Frame>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whiteboard_pages: Option<Vec<Frame>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blackboard_pages: Option<Vec<Frame>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent_active_page: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whiteboard_active_page: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blackboard_active_page: Option<usize>,
    #[serde(default)]
    pub tool_state: Option<ToolStateSnapshot>,
}

fn legacy_file_version() -> u32 {
    1
}

#[test]
fn pre_generation_reader_accepts_new_files_and_resaves_without_the_optional_member() {
    let temp = tempdir().unwrap();
    let options = options(temp.path());
    save::save_snapshot_with_report(&sample_snapshot(), &options).unwrap();
    let legacy: LegacySessionFileA2bb723a =
        serde_json::from_slice(&fs::read(options.session_file_path()).unwrap()).unwrap();
    let bytes = serde_json::to_vec_pretty(&legacy).unwrap();
    assert_eq!(
        generation::payload_prefix_generation(&bytes),
        Generation::Unknown
    );
    fs::write(options.session_file_path(), bytes).unwrap();
    let loaded = load::load_snapshot_inner(&options.session_file_path(), &options)
        .unwrap()
        .unwrap();
    assert_eq!(line_count(&loaded.snapshot), 1);
}
