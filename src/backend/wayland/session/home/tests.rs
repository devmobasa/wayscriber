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
            let (options, outcome) = load.loaded.expect("home loads");
            assert_eq!(options.target, self.home.target);
            assert!(matches!(outcome, LoadSnapshotOutcome::Loaded(_)));
            let (path, error) = load.abandoned.expect("the remembered session is given up");
            assert_eq!(path, self.remembered.session_file_path());
            error
        }
    }

    #[test]
    fn a_remembered_session_that_is_still_there_is_continued() {
        let mut sessions = sessions();
        let home = sessions.home.clone();

        let load = sessions.load(Some(home)).unwrap();

        let (options, outcome) = load.loaded.unwrap();
        assert_eq!(options.target, sessions.remembered.target);
        assert!(matches!(outcome, LoadSnapshotOutcome::Loaded(_)));
        assert!(load.abandoned.is_none());
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
    fn a_session_deleted_before_a_retry_is_checked_again() {
        let mut sessions = sessions();
        let path = sessions.remembered.session_file_path();
        let home = sessions.home.clone();
        // The first attempt fails as an I/O error would, before the file goes.
        let failed = load_output_session(
            sessions.remembered.clone(),
            Some(&path),
            Some(home.clone()),
            |_| Err(anyhow::anyhow!("session load failed")),
        );
        assert!(failed.is_err());
        std::fs::remove_file(&path).unwrap();

        // The retry first saves the current session, as an output transition
        // does; a save into the remembered session would recreate it.
        let remembered = sessions.remembered.clone();
        let may_save = may_save_before_output_load(&remembered, Some(&path), |operation| {
            sessions.persistence.run(0, operation)
        })
        .unwrap();
        if may_save {
            stored_session::save_snapshot(&sample_snapshot(), &remembered).unwrap();
        }
        let load = sessions.load(Some(home)).unwrap();

        assert!(!may_save);
        sessions.assert_went_home(load);
    }

    #[test]
    fn only_an_unusable_remembered_session_holds_back_the_save() {
        let mut sessions = sessions();
        let path = sessions.remembered.session_file_path();
        let remembered = sessions.remembered.clone();
        let mut run = |operation| sessions.persistence.run(0, operation);

        assert!(may_save_before_output_load(&remembered, Some(&path), &mut run).unwrap());
        // Another session is saved as before, without a check.
        assert!(
            may_save_before_output_load(&remembered, None, |_| {
                panic!("no check runs for a session that is not remembered")
            })
            .unwrap()
        );
    }

    #[test]
    fn without_persistence_home_has_nothing_to_load() {
        let mut sessions = sessions();
        std::fs::remove_file(sessions.remembered.session_file_path()).unwrap();

        let load = sessions.load(None).unwrap();

        assert!(load.loaded.is_none());
        assert!(load.abandoned.is_some());
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

        let (options, outcome) = load.loaded.unwrap();
        assert_eq!(options.target, remembered.target);
        assert!(matches!(outcome, LoadSnapshotOutcome::Empty));
        assert!(load.abandoned.is_none());
    }
}
