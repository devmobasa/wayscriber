use std::collections::HashMap;

use super::*;

const HOME: &str = "/sessions/home.wayscriber-session";
const PREFERRED: &str = "/sessions/b.wayscriber-session";

fn launch(startup_file: Option<&str>, environment: &[(&str, &str)]) -> SessionLaunch {
    let environment: HashMap<_, _> = environment.iter().copied().collect();
    SessionLaunch::from_lookup(startup_file.map(Path::new), |name| {
        environment.get(name).map(OsString::from)
    })
}

fn named(path: &str) -> HomeSession {
    HomeSession::Named(PathBuf::from(path))
}

#[test]
fn a_daemon_launch_carries_home_and_the_remembered_session() {
    let daemon = [
        (OVERLAY_SESSION_REPORTS_ENV, "1"),
        (OVERLAY_HOME_SESSION_ENV, HOME),
        (OVERLAY_PREFERRED_SESSION_ENV, PREFERRED),
    ];

    assert_eq!(
        launch(Some(HOME), &daemon),
        SessionLaunch {
            home: named(HOME),
            preferred: Some(PathBuf::from(PREFERRED)),
            from_daemon: true,
        }
    );
}

#[test]
fn without_a_startup_file_home_is_the_default_session() {
    let daemon = [
        (OVERLAY_SESSION_REPORTS_ENV, "1"),
        (OVERLAY_PREFERRED_SESSION_ENV, PREFERRED),
    ];

    assert_eq!(
        launch(None, &daemon),
        SessionLaunch {
            home: HomeSession::Default,
            preferred: Some(PathBuf::from(PREFERRED)),
            from_daemon: true,
        }
    );
}

#[test]
fn an_explicit_session_file_outranks_the_remembered_one() {
    let explicit = "/sessions/c.wayscriber-session";
    for home in [None, Some(HOME)] {
        let mut daemon = vec![
            (OVERLAY_SESSION_REPORTS_ENV, "1"),
            (OVERLAY_PREFERRED_SESSION_ENV, PREFERRED),
        ];
        daemon.extend(home.map(|home| (OVERLAY_HOME_SESSION_ENV, home)));

        let launch = launch(Some(explicit), &daemon);

        assert_eq!(launch.home, home.map_or(HomeSession::Default, named));
        assert_eq!(launch.preferred, None, "home {home:?}");
    }
}

#[test]
fn without_a_daemon_that_reads_reports_home_is_the_startup_session() {
    let inputs = [
        (OVERLAY_HOME_SESSION_ENV, HOME),
        (OVERLAY_PREFERRED_SESSION_ENV, PREFERRED),
    ];
    for marker in [None, Some("0")] {
        let mut environment = inputs.to_vec();
        environment.extend(marker.map(|marker| (OVERLAY_SESSION_REPORTS_ENV, marker)));
        let startup = "/sessions/startup.wayscriber-session";

        assert_eq!(
            launch(Some(startup), &environment),
            SessionLaunch {
                home: named(startup),
                preferred: None,
                from_daemon: false,
            }
        );
        assert_eq!(
            launch(None, &environment),
            SessionLaunch {
                home: HomeSession::Default,
                preferred: None,
                from_daemon: false,
            }
        );
    }
}

fn home_session(home: HomeSession, preferred: Option<&str>, startup: SessionTarget) -> SessionHome {
    SessionHome::new(
        SessionLaunch {
            home,
            preferred: preferred.map(PathBuf::from),
            from_daemon: true,
        },
        None,
        startup,
    )
}

fn file(path: &str) -> SessionTarget {
    SessionTarget::NamedFile(PathBuf::from(path))
}

/// Enters `target` and reports it, returning what was reported.
fn report(home: &mut SessionHome, target: SessionTarget) -> Option<ReportedSession> {
    home.enter(target);
    let report = home.unreported();
    home.mark_reported();
    report
}

#[test]
fn home_is_named_like_any_other_session() {
    let named = home_session(named(HOME), None, file(HOME));
    let default = home_session(HomeSession::Default, None, SessionTarget::Configured);

    assert_eq!(named.label(), "home.wayscriber-session");
    assert_eq!(named.name().as_deref(), Some("home.wayscriber-session"));
    assert_eq!(default.label(), "the default session");
    assert_eq!(default.name(), None);
}

#[test]
fn a_continued_remembered_session_is_reported_before_anything_changes() {
    // The daemon launched the overlay at home; only the overlay knows that it
    // continues the remembered session instead.
    let continued = home_session(named(HOME), Some(PREFERRED), file(PREFERRED));
    assert_eq!(
        continued.unreported(),
        Some(ReportedSession::Named(PathBuf::from(PREFERRED)))
    );

    for started_at_launch_target in [
        home_session(named(HOME), None, file(HOME)),
        home_session(named(HOME), None, file("/sessions/c.wayscriber-session")),
        home_session(HomeSession::Default, None, SessionTarget::Configured),
    ] {
        assert_eq!(started_at_launch_target.unreported(), None);
    }
}

#[test]
fn only_a_changed_session_is_reported() {
    let mut home = home_session(named(HOME), None, file(HOME));

    assert_eq!(report(&mut home, file(HOME)), None);

    let other = "/sessions/d.wayscriber-session";
    assert_eq!(
        report(&mut home, file(other)),
        Some(ReportedSession::Named(PathBuf::from(other)))
    );
    assert_eq!(report(&mut home, file(other)), None);
}

#[test]
fn a_report_that_was_not_written_is_tried_again() {
    let mut home = home_session(named(HOME), None, file(HOME));
    let other = "/sessions/d.wayscriber-session";

    home.enter(file(other));
    assert!(home.unreported().is_some());
    // The write failed, so nothing was marked reported.
    home.enter(file(other));

    assert_eq!(
        home.unreported(),
        Some(ReportedSession::Named(PathBuf::from(other)))
    );
}

#[test]
fn home_is_reported_as_home_even_as_a_named_file() {
    let mut named_home = home_session(named(HOME), None, file(PREFERRED));
    // Another spelling of the same file is still home.
    let alias = "/sessions/./home.wayscriber-session";

    assert_eq!(
        report(&mut named_home, file(alias)),
        Some(ReportedSession::Home)
    );

    let mut default_home = home_session(HomeSession::Default, None, file(PREFERRED));
    assert_eq!(
        report(&mut default_home, SessionTarget::Configured),
        Some(ReportedSession::Home)
    );
    assert_eq!(
        report(&mut default_home, file(HOME)),
        Some(ReportedSession::Named(PathBuf::from(HOME)))
    );
}

#[test]
fn the_overlay_knows_whether_it_is_home() {
    let mut named_home = home_session(named(HOME), Some(PREFERRED), file(PREFERRED));
    assert!(!named_home.is_at_home());
    named_home.enter(file("/sessions/./home.wayscriber-session"));
    assert!(named_home.is_at_home());
    named_home.enter(file(PREFERRED));
    assert!(!named_home.is_at_home());
    assert!(home_session(named(HOME), None, file(HOME)).is_at_home());

    let mut default_home = home_session(HomeSession::Default, None, file(PREFERRED));
    assert!(!default_home.is_at_home());
    default_home.enter(SessionTarget::Configured);
    assert!(default_home.is_at_home());
    assert!(home_session(HomeSession::Default, None, SessionTarget::Configured).is_at_home());
}

#[test]
fn a_run_without_persistence_is_in_the_default_session() {
    assert_eq!(session_target(None), SessionTarget::Configured);
}

mod load {
    use super::super::super::tests::{EnvGuard, named_options, sample_snapshot};
    use super::*;
    use crate::backend::wayland::session::PersistenceController;
    use crate::session as stored_session;

    struct Sessions {
        temp: crate::test_temp::TempDir,
        remembered: SessionOptions,
        home: SessionOptions,
        persistence: PersistenceController,
    }

    /// A saved remembered session and a saved configured home.
    /// A session file as this build writes one with tool settings and no ink.
    fn blank_session_bytes(dir: &std::path::Path) -> Vec<u8> {
        let scratch = named_options(dir, "blank");
        let snapshot = stored_session::SessionSnapshot {
            active_board_id: "transparent".to_string(),
            boards: Vec::new(),
            tool_state: Some(stored_session::ToolStateSnapshot::from_config(
                &crate::config::Config::default(),
            )),
        };
        stored_session::save_snapshot(&snapshot, &scratch).unwrap();
        std::fs::read(scratch.session_file_path()).unwrap()
    }

    fn sessions() -> Sessions {
        let temp = crate::test_temp::tempdir().unwrap();
        let remembered = named_options(temp.path(), "remembered");
        let mut home = SessionOptions::new(temp.path().join("configured"), "home");
        home.persist_transparent = true;
        stored_session::save_snapshot(&sample_snapshot(), &remembered).unwrap();
        stored_session::save_snapshot(&sample_snapshot(), &home).unwrap();

        Sessions {
            temp,
            remembered,
            home,
            persistence: PersistenceController::start_for_test().unwrap(),
        }
    }

    impl Sessions {
        fn load(&mut self, home: Option<SessionOptions>) -> Result<OutputSessionLoad> {
            let path = self.remembered.session_file_path();
            load_output_session(self.remembered.clone(), Some(&path), home, |operation| {
                self.persistence.run(0, operation)
            })
        }

        fn assert_went_home(&mut self, load: OutputSessionLoad) -> anyhow::Error {
            let OutputSessionLoad::WentHome {
                remembered,
                reason,
                home: Some((options, outcome)),
            } = load
            else {
                panic!("expected home to load in place of the remembered session: {load:?}");
            };
            assert_eq!(options.target, self.home.target);
            assert!(matches!(outcome, LoadSnapshotOutcome::Loaded(_)));
            assert_eq!(remembered, self.remembered.session_file_path());
            reason
        }
    }

    #[test]
    fn a_remembered_session_that_is_still_there_is_continued() {
        let mut sessions = sessions();
        let home = sessions.home.clone();

        let load = sessions.load(Some(home)).unwrap();

        let OutputSessionLoad::Loaded(options, outcome) = load else {
            panic!("expected the remembered session to load: {load:?}");
        };
        assert_eq!(options.target, sessions.remembered.target);
        assert!(matches!(outcome, LoadSnapshotOutcome::Loaded(_)));
    }

    #[test]
    fn a_deleted_remembered_session_starts_at_home() {
        let mut sessions = sessions();
        std::fs::remove_file(sessions.remembered.session_file_path()).unwrap();
        let home = sessions.home.clone();

        let load = sessions.load(Some(home)).unwrap();

        let error = sessions.assert_went_home(load);
        assert!(
            error
                .downcast_ref::<stored_session::MissingNamedSessionFile>()
                .is_some(),
            "{error:#}"
        );
    }

    #[test]
    fn a_moved_remembered_session_is_not_continued_from_its_backup() {
        let mut sessions = sessions();
        let path = sessions.remembered.session_file_path();
        // A second save leaves a backup of the first beside the file.
        stored_session::save_snapshot(&sample_snapshot(), &sessions.remembered).unwrap();
        assert!(sessions.remembered.backup_file_path().exists());
        std::fs::rename(&path, path.with_extension("moved")).unwrap();
        let home = sessions.home.clone();
        let _env = EnvGuard::set_xdg_data_home(sessions.temp.path());

        let load = sessions.load(Some(home)).unwrap();

        sessions.assert_went_home(load);
        // Its backup was never loaded, so nothing records it as opened.
        assert!(
            stored_session::catalog::recent_sessions()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_remembered_session_that_became_a_symlink_or_directory_is_not_used() {
        for replace in ["symlink", "directory"] {
            let mut sessions = sessions();
            let path = sessions.remembered.session_file_path();
            let moved = path.with_extension("moved");
            std::fs::rename(&path, &moved).unwrap();
            if replace == "symlink" {
                std::os::unix::fs::symlink(&moved, &path).unwrap();
            } else {
                std::fs::create_dir(&path).unwrap();
            }
            let home = sessions.home.clone();

            let load = sessions.load(Some(home)).unwrap();

            let error = sessions.assert_went_home(load);
            assert!(format!("{error:#}").contains(replace), "{error:#}");
        }
    }

    #[test]
    fn without_persistence_home_has_nothing_to_load() {
        let mut sessions = sessions();
        std::fs::remove_file(sessions.remembered.session_file_path()).unwrap();

        let load = sessions.load(None).unwrap();

        assert!(matches!(
            load,
            OutputSessionLoad::WentHome { home: None, .. }
        ));
    }

    #[test]
    fn a_session_file_given_at_launch_keeps_its_startup_rules() {
        let mut sessions = sessions();
        std::fs::remove_file(sessions.remembered.session_file_path()).unwrap();
        let home = sessions.home.clone();
        let remembered = sessions.remembered.clone();

        // Not the remembered session: a missing file is a new, empty session.
        let load = load_output_session(remembered.clone(), None, Some(home), |operation| {
            sessions.persistence.run(0, operation)
        })
        .unwrap();

        let OutputSessionLoad::Loaded(options, outcome) = load else {
            panic!("expected the startup session to load: {load:?}");
        };
        assert_eq!(options.target, remembered.target);
        assert!(matches!(outcome, LoadSnapshotOutcome::Empty));
    }

    #[test]
    fn a_cleared_remembered_primary_cannot_revive_ink_or_quarantine_suppressed_bytes() {
        for primary_kind in ["ink", "corrupt", "blank"] {
            let mut sessions = sessions();
            let options = sessions.remembered.clone();
            let primary = options.session_file_path();
            let ink = std::fs::read(&primary).unwrap();
            let blank = blank_session_bytes(sessions.temp.path());
            let bytes = match primary_kind {
                "ink" => ink.as_slice(),
                "corrupt" => b"suppressed corrupt primary".as_slice(),
                "blank" => blank.as_slice(),
                _ => unreachable!(),
            };
            std::fs::write(&primary, bytes).unwrap();
            std::fs::write(options.backup_file_path(), &ink).unwrap();
            std::fs::write(options.recovery_file_path(), &ink).unwrap();
            let clear = options.clear_marker_file_path();
            std::fs::write(&clear, b"cleared").unwrap();
            let modified = |seconds| {
                std::fs::FileTimes::new().set_modified(
                    std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(seconds),
                )
            };
            std::fs::File::open(&primary)
                .unwrap()
                .set_times(modified(10))
                .unwrap();
            std::fs::File::open(&clear)
                .unwrap()
                .set_times(modified(20))
                .unwrap();
            for sidecar in [options.backup_file_path(), options.recovery_file_path()] {
                std::fs::File::open(sidecar)
                    .unwrap()
                    .set_times(modified(10))
                    .unwrap();
            }
            let home = sessions.home.clone();

            let load = sessions.load(Some(home)).unwrap();

            let OutputSessionLoad::Loaded(_, outcome) = load else {
                panic!("cleared {primary_kind} must stay on the remembered target: {load:?}");
            };
            assert!(
                !outcome.has_board_data(),
                "cleared {primary_kind}: {outcome:?}"
            );
            if primary_kind == "blank" {
                assert!(matches!(outcome, LoadSnapshotOutcome::Loaded(_)));
            } else {
                assert!(matches!(outcome, LoadSnapshotOutcome::Empty));
            }
            assert_eq!(std::fs::read(&primary).unwrap(), bytes);
            assert_eq!(std::fs::read(options.backup_file_path()).unwrap(), ink);
            assert_eq!(std::fs::read(options.recovery_file_path()).unwrap(), ink);
            assert!(!stored_session::append_path_suffix(&primary, ".corrupt-1").exists());
        }
    }

    #[test]
    fn a_corrupt_remembered_primary_restores_its_backup_and_keeps_diagnostics() {
        let mut sessions = sessions();
        let options = &sessions.remembered;
        let primary = options.session_file_path();
        let saved = std::fs::read(&primary).unwrap();
        std::fs::write(options.backup_file_path(), &saved).unwrap();
        std::fs::write(options.recovery_file_path(), &saved).unwrap();
        std::fs::write(options.backup_recovery_marker_file_path(), b"recoverable").unwrap();
        std::fs::write(
            options.recovery_recoverable_marker_file_path(),
            b"recoverable",
        )
        .unwrap();
        let bytes = b"broken primary".as_slice();
        std::fs::write(&primary, bytes).unwrap();
        let modified = |seconds| {
            std::fs::FileTimes::new().set_modified(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(seconds),
            )
        };
        std::fs::File::open(options.recovery_file_path())
            .unwrap()
            .set_times(modified(10))
            .unwrap();
        std::fs::File::open(&primary)
            .unwrap()
            .set_times(modified(20))
            .unwrap();
        let home = sessions.home.clone();

        let load = sessions.load(Some(home)).unwrap();
        let OutputSessionLoad::Loaded(
            loaded_options,
            LoadSnapshotOutcome::RestoredAfterCorruption { snapshot, .. },
        ) = load
        else {
            panic!("expected restored remembered session");
        };
        assert_eq!(loaded_options.target, sessions.remembered.target);
        assert!(snapshot.has_board_data());
        assert_eq!(
            std::fs::read(stored_session::append_path_suffix(&primary, ".corrupt-1")).unwrap(),
            bytes
        );
        let options = &sessions.remembered;
        assert_eq!(std::fs::read(options.session_file_path()).unwrap(), saved);
        assert_eq!(std::fs::read(options.backup_file_path()).unwrap(), saved);
        assert_eq!(std::fs::read(options.recovery_file_path()).unwrap(), saved);
    }

    #[test]
    fn a_remembered_session_goes_home_when_oversized_corruption_restore_leaves_no_primary() {
        let mut sessions = sessions();
        let mut snapshot = sample_snapshot();
        snapshot.boards[0].pages.pages[0].add_shape(crate::draw::Shape::Freehand {
            points: (0..40_000)
                .map(|index| (index % 1000, (index * 17) % 800))
                .collect(),
            color: crate::draw::WHITE,
            thick: 2.0,
        });
        let mut scratch = named_options(sessions.temp.path(), "large-recovery");
        scratch.compression = stored_session::CompressionMode::Off;
        stored_session::save_snapshot(&snapshot, &scratch).unwrap();
        let saved = std::fs::read(scratch.session_file_path()).unwrap();

        // This valid filename leaves room for the sidecars, but not the atomic
        // diagnostic copy's temporary name. Its failed copy moves the corrupt
        // primary aside, exercising the real fallback without filling the disk.
        let long_name = format!("{}.wayscriber-session", "a".repeat(203));
        sessions
            .remembered
            .set_named_file_target(sessions.temp.path().join(long_name));
        sessions.remembered.max_file_size_bytes = 1024 * 1024;
        assert!(saved.len() as u64 > sessions.remembered.max_file_size_bytes);
        let primary = sessions.remembered.session_file_path();
        let recovery = sessions.remembered.recovery_file_path();
        let corrupt_bytes = b"broken primary";
        std::fs::write(&primary, corrupt_bytes).unwrap();
        std::fs::write(&recovery, &saved).unwrap();
        std::fs::write(
            sessions.remembered.recovery_recoverable_marker_file_path(),
            b"recoverable",
        )
        .unwrap();
        // Older marked recovery forces the corrupt-primary restoration path,
        // rather than loading a newer recovery before examining the primary.
        for (path, seconds) in [(&primary, 20), (&recovery, 10)] {
            std::fs::File::open(path)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(
                    std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(seconds),
                ))
                .unwrap();
        }
        let home = sessions.home.clone();

        let load = sessions.load(Some(home)).unwrap();

        let error = sessions.assert_went_home(load);
        assert!(
            error
                .downcast_ref::<stored_session::MissingNamedSessionFile>()
                .is_some(),
            "{error:#}"
        );
        assert!(!primary.exists());
        assert_eq!(std::fs::read(&recovery).unwrap(), saved);
        assert_eq!(
            std::fs::read(stored_session::append_path_suffix(&primary, ".corrupt-1")).unwrap(),
            corrupt_bytes
        );
    }

    #[test]
    fn remembered_recovery_and_marked_backup_survive_the_next_fitting_save() {
        for source in ["newer recovery", "marked backup"] {
            let mut sessions = sessions();
            let mut options = sessions.remembered.clone();
            let mut newest = sample_snapshot();
            newest.boards[0].pages.pages[0].add_shape(crate::draw::Shape::Line {
                x1: 10,
                y1: 10,
                x2: 71,
                y2: 20,
                color: crate::draw::Color {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                thick: 2.0,
            });
            if source == "newer recovery" {
                options.max_file_size_bytes = 64;
                stored_session::save_snapshot(&newest, &options).unwrap_err();
                let modified = |seconds| {
                    std::fs::FileTimes::new().set_modified(
                        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(seconds),
                    )
                };
                std::fs::File::open(options.session_file_path())
                    .unwrap()
                    .set_times(modified(10))
                    .unwrap();
                std::fs::File::open(options.recovery_file_path())
                    .unwrap()
                    .set_times(modified(20))
                    .unwrap();
                options.max_file_size_bytes = sessions.remembered.max_file_size_bytes;
            } else {
                stored_session::save_snapshot(&newest, &options).unwrap();
                let blank = stored_session::SessionSnapshot {
                    active_board_id: "transparent".into(),
                    boards: Vec::new(),
                    tool_state: None,
                };
                // Tool-state-only blank retains a live accidental-blank marker.
                let mut blank = blank;
                blank.tool_state = Some(stored_session::ToolStateSnapshot::from_config(
                    &crate::config::Config::default(),
                ));
                stored_session::save_snapshot(&blank, &options).unwrap();
            }
            let home = sessions.home.clone();

            let load = sessions.load(Some(home)).unwrap();

            let OutputSessionLoad::Loaded(_, outcome) = load else {
                panic!("{source} must continue the remembered session: {load:?}");
            };
            let snapshot = match outcome {
                LoadSnapshotOutcome::LoadedFromRecovery(snapshot) if source == "newer recovery" => {
                    snapshot
                }
                LoadSnapshotOutcome::LoadedFromBackup(snapshot) if source == "marked backup" => {
                    snapshot
                }
                other => panic!("{source} must be restored, got {other:?}"),
            };
            assert_eq!(
                snapshot.boards[0].pages.pages[0].shapes.len(),
                2,
                "{source}"
            );
            stored_session::save_snapshot(&snapshot, &options).unwrap();
            let OutputSessionLoad::Loaded(_, outcome) = sessions.load(None).unwrap() else {
                panic!("the fitting save must still continue the remembered file");
            };
            let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
                panic!("{outcome:?}");
            };
            assert_eq!(
                snapshot.boards[0].pages.pages[0].shapes.len(),
                2,
                "{source} ink survived recovery cleanup"
            );
        }
    }
}
