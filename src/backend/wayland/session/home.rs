//! The overlay's home session and the session inputs its launch carries.
//!
//! An overlay the daemon launches returns home to the daemon's startup session
//! file, or to the configured default session when the daemon has none. The
//! daemon may also pass a remembered session to continue in place of home.
//! Neither input is a command-line flag, so an older overlay ignores both and
//! keeps starting in the session its `--session-file` names.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::daemon::protocol_v2::ReportedSession;
use crate::env_vars::{
    OVERLAY_HOME_SESSION_ENV, OVERLAY_PREFERRED_SESSION_ENV, OVERLAY_SESSION_REPORTS_ENV,
};
use crate::session::catalog::session_paths_match;
use crate::session::{SessionOptions, SessionTarget};
use crate::ui::toolbar::session_format::session_display_name;

/// The session an overlay returns home to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) enum HomeSession {
    /// The configured default session.
    Default,
    /// A session file the daemon started with.
    Named(PathBuf),
}

impl HomeSession {
    pub(in crate::backend::wayland) fn file(&self) -> Option<&Path> {
        match self {
            Self::Default => None,
            Self::Named(path) => Some(path),
        }
    }
}

/// The session inputs one overlay launch carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct SessionLaunch {
    pub(in crate::backend::wayland) home: HomeSession,
    /// A remembered session to start in instead of home, if it still exists.
    pub(in crate::backend::wayland) preferred: Option<PathBuf>,
}

impl SessionLaunch {
    /// The inputs for an overlay launched with `startup_file` as its
    /// `--session-file`.
    pub(in crate::backend::wayland) fn from_environment(startup_file: Option<&Path>) -> Self {
        Self::from_lookup(startup_file, |name| std::env::var_os(name))
    }

    fn from_lookup(startup_file: Option<&Path>, lookup: impl Fn(&str) -> Option<OsString>) -> Self {
        let present = |name| {
            lookup(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        // Without a daemon that reads reports, this is a standalone overlay or
        // one an older daemon launched: home is what it started in.
        if lookup(OVERLAY_SESSION_REPORTS_ENV).is_none_or(|value| value != "1") {
            return Self {
                home: startup_file.map_or(HomeSession::Default, |path| {
                    HomeSession::Named(path.to_path_buf())
                }),
                preferred: None,
            };
        }

        let home =
            present(OVERLAY_HOME_SESSION_ENV).map_or(HomeSession::Default, HomeSession::Named);
        // A `--session-file` other than home was asked for explicitly, and an
        // explicit file outranks the remembered one.
        let preferred =
            present(OVERLAY_PREFERRED_SESSION_ENV).filter(|_| startup_file == home.file());

        Self { home, preferred }
    }
}

/// What the running overlay keeps about its home session.
#[derive(Debug)]
pub(in crate::backend::wayland) struct SessionHome {
    home: HomeSession,
    /// Home's session options before an output identity is applied; `None`
    /// when home has persistence disabled.
    options: Option<SessionOptions>,
    /// The preferred session, until the first load checks that it still exists.
    unchecked_preferred: Option<PathBuf>,
    /// The target the daemon last learned this overlay is in, by launching it
    /// there or from a report.
    reported: SessionTarget,
}

impl SessionHome {
    /// `startup` is the target this run starts in.
    pub(in crate::backend::wayland) fn new(
        launch: SessionLaunch,
        options: Option<SessionOptions>,
        startup: SessionTarget,
    ) -> Self {
        Self {
            home: launch.home,
            options,
            unchecked_preferred: launch.preferred,
            reported: startup,
        }
    }

    pub(in crate::backend::wayland) fn options(&self) -> Option<&SessionOptions> {
        self.options.as_ref()
    }

    /// The preferred session this run started in, once: only the first load
    /// checks it.
    pub(in crate::backend::wayland) fn take_unchecked_preferred(&mut self) -> Option<PathBuf> {
        self.unchecked_preferred.take()
    }

    /// How the Session menu and notices name home.
    pub(in crate::backend::wayland) fn label(&self) -> String {
        match &self.home {
            HomeSession::Default => "the default session".to_owned(),
            HomeSession::Named(path) => session_display_name(path),
        }
    }

    /// What to report now that the overlay is in `target`, or `None` when the
    /// daemon already knows. Home is reported as such even when it is a named
    /// file, so the daemon never remembers home as a session of its own.
    pub(in crate::backend::wayland) fn report_for(
        &mut self,
        target: SessionTarget,
    ) -> Option<ReportedSession> {
        if target == self.reported {
            return None;
        }
        let report = match &target {
            SessionTarget::NamedFile(path) if !self.is_home_file(path) => {
                ReportedSession::Named(path.clone())
            }
            _ => ReportedSession::Home,
        };
        self.reported = target;
        Some(report)
    }

    fn is_home_file(&self, path: &Path) -> bool {
        self.home
            .file()
            .is_some_and(|home| session_paths_match(home, path))
    }
}

/// The target a run with `options` is in: a run without persistence is in the
/// configured default session all the same.
pub(in crate::backend::wayland) fn session_target(
    options: Option<&SessionOptions>,
) -> SessionTarget {
    options.map_or(SessionTarget::Configured, |options| options.target.clone())
}

#[cfg(test)]
mod tests;
