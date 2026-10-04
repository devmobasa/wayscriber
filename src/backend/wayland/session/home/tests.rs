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
            }
        );
        assert_eq!(
            launch(None, &environment),
            SessionLaunch {
                home: HomeSession::Default,
                preferred: None,
            }
        );
    }
}

fn home_session(home: HomeSession, preferred: Option<&str>, startup: SessionTarget) -> SessionHome {
    SessionHome::new(
        SessionLaunch {
            home,
            preferred: preferred.map(PathBuf::from),
        },
        None,
        startup,
    )
}

fn file(path: &str) -> SessionTarget {
    SessionTarget::NamedFile(PathBuf::from(path))
}

#[test]
fn home_is_named_like_any_other_session() {
    let named = home_session(named(HOME), None, file(HOME));
    let default = home_session(HomeSession::Default, None, SessionTarget::Configured);

    assert_eq!(named.label(), "home.wayscriber-session");
    assert_eq!(default.label(), "the default session");
}

#[test]
fn only_the_first_load_checks_the_preferred_session() {
    let mut home = home_session(HomeSession::Default, Some(PREFERRED), file(PREFERRED));

    assert_eq!(
        home.take_unchecked_preferred(),
        Some(PathBuf::from(PREFERRED))
    );
    assert_eq!(home.take_unchecked_preferred(), None);
}

#[test]
fn only_a_changed_session_is_reported() {
    let mut home = home_session(named(HOME), Some(PREFERRED), file(PREFERRED));

    assert_eq!(home.report_for(file(PREFERRED)), None);

    let other = "/sessions/d.wayscriber-session";
    assert_eq!(
        home.report_for(file(other)),
        Some(ReportedSession::Named(PathBuf::from(other)))
    );
    assert_eq!(home.report_for(file(other)), None);
}

#[test]
fn home_is_reported_as_home_even_as_a_named_file() {
    let mut named_home = home_session(named(HOME), None, file(PREFERRED));
    // Another spelling of the same file is still home.
    let alias = "/sessions/./home.wayscriber-session";

    assert_eq!(
        named_home.report_for(file(alias)),
        Some(ReportedSession::Home)
    );

    let mut default_home = home_session(HomeSession::Default, None, file(PREFERRED));
    assert_eq!(
        default_home.report_for(SessionTarget::Configured),
        Some(ReportedSession::Home)
    );
    assert_eq!(
        default_home.report_for(file(HOME)),
        Some(ReportedSession::Named(PathBuf::from(HOME)))
    );
}

#[test]
fn a_run_without_persistence_is_in_the_default_session() {
    assert_eq!(session_target(None), SessionTarget::Configured);
}
