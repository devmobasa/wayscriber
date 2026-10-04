//! The overlay's home session and the session inputs its launch carries.
//!
//! An overlay the daemon launches returns home to the daemon's startup session
//! file, or to the configured default session when the daemon has none. The
//! daemon may also pass a remembered session to continue in place of home.
//! Neither input is a command-line flag, so an older overlay ignores both and
//! keeps starting in the session its `--session-file` names.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::{PersistenceOperation, PersistenceOutcome};
use crate::daemon::protocol_v2::ReportedSession;
use crate::env_vars::{
    OVERLAY_HOME_SESSION_ENV, OVERLAY_PREFERRED_SESSION_ENV, OVERLAY_SESSION_REPORTS_ENV,
};
use crate::session::catalog::session_paths_match;
use crate::session::{LoadSnapshotOutcome, SessionOptions, SessionTarget};
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
    /// Whether a daemon that reads reports launched the overlay, so its
    /// resume policy is the one the daemon passed.
    pub(in crate::backend::wayland) from_daemon: bool,
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
                from_daemon: false,
            };
        }

        let home =
            present(OVERLAY_HOME_SESSION_ENV).map_or(HomeSession::Default, HomeSession::Named);
        // A `--session-file` other than home was asked for explicitly, and an
        // explicit file outranks the remembered one.
        let preferred =
            present(OVERLAY_PREFERRED_SESSION_ENV).filter(|_| startup_file == home.file());

        Self {
            home,
            preferred,
            from_daemon: true,
        }
    }
}

/// What the running overlay keeps about its home session.
#[derive(Debug)]
pub(in crate::backend::wayland) struct SessionHome {
    home: HomeSession,
    /// Home's session options before an output identity is applied; `None`
    /// when home has persistence disabled.
    options: Option<SessionOptions>,
    /// The remembered session this run was asked to continue.
    remembered: Option<PathBuf>,
    /// The target the overlay is in, as of the last commit.
    current: SessionTarget,
    /// Whether `current` is home. Kept rather than worked out when asked,
    /// since the Session menu asks on every redraw.
    at_home: bool,
    /// The target the daemon last learned this overlay is in, by launching it
    /// there or from a report. `None` until a continued remembered session is
    /// reported: the daemon launched the overlay at home, not in it.
    reported: Option<SessionTarget>,
}

impl SessionHome {
    /// `startup` is the target this run starts in.
    pub(in crate::backend::wayland) fn new(
        launch: SessionLaunch,
        options: Option<SessionOptions>,
        startup: SessionTarget,
    ) -> Self {
        Self {
            at_home: is_home(&launch.home, &startup),
            reported: launch.preferred.is_none().then(|| startup.clone()),
            home: launch.home,
            options,
            remembered: launch.preferred,
            current: startup,
        }
    }

    /// Home's options for the output identified as `output_identity`.
    pub(in crate::backend::wayland) fn options_for_output(
        &self,
        output_identity: Option<&str>,
    ) -> Option<SessionOptions> {
        let mut options = self.options.clone()?;
        options.set_output_identity(output_identity);
        Some(options)
    }

    /// The remembered session this run was asked to continue.
    pub(in crate::backend::wayland) fn remembered(&self) -> Option<&Path> {
        self.remembered.as_deref()
    }

    /// The name of a named home session; `None` for the default session.
    pub(in crate::backend::wayland) fn name(&self) -> Option<String> {
        self.home.file().map(session_display_name)
    }

    /// How notices name home.
    pub(in crate::backend::wayland) fn label(&self) -> String {
        self.name()
            .unwrap_or_else(|| "the default session".to_owned())
    }

    /// Whether the overlay is in its home session, as of the last commit.
    pub(in crate::backend::wayland) fn is_at_home(&self) -> bool {
        self.at_home
    }

    /// Records that the overlay is now in `target`.
    pub(in crate::backend::wayland) fn enter(&mut self, target: SessionTarget) {
        if target != self.current {
            self.at_home = is_home(&self.home, &target);
            self.current = target;
        }
    }

    /// What the daemon has yet to learn about the overlay's session. Home is
    /// reported as such even when it is a named file, so the daemon never
    /// remembers home as a session of its own.
    pub(in crate::backend::wayland) fn unreported(&self) -> Option<ReportedSession> {
        if self.reported.as_ref() == Some(&self.current) {
            return None;
        }

        Some(match &self.current {
            SessionTarget::NamedFile(path) if !self.at_home => ReportedSession::Named(path.clone()),
            _ => ReportedSession::Home,
        })
    }

    /// The daemon now knows the overlay's session; a failed report is tried
    /// again on the next commit instead.
    pub(in crate::backend::wayland) fn mark_reported(&mut self) {
        self.reported = Some(self.current.clone());
    }
}

fn is_home(home: &HomeSession, target: &SessionTarget) -> bool {
    match (home, target) {
        (HomeSession::Default, SessionTarget::Configured) => true,
        (HomeSession::Named(home), SessionTarget::NamedFile(path)) => {
            session_paths_match(home, path)
        }
        _ => false,
    }
}

/// The target a run with `options` is in: a run without persistence is in the
/// configured default session all the same.
pub(in crate::backend::wayland) fn session_target(
    options: Option<&SessionOptions>,
) -> SessionTarget {
    options.map_or(SessionTarget::Configured, |options| options.target.clone())
}

/// What an output's session load found.
#[derive(Debug)]
pub(in crate::backend::wayland) struct OutputSessionLoad {
    /// The options loaded and what loading them found: the staged options, or
    /// home's in place of a remembered session that can no longer be used.
    /// `None` when that home has persistence disabled, leaving nothing to load.
    pub(in crate::backend::wayland) loaded: Option<(SessionOptions, LoadSnapshotOutcome)>,
    /// The remembered session given up for home, and why.
    pub(in crate::backend::wayland) abandoned: Option<(PathBuf, anyhow::Error)>,
}

/// Loads `staged`, the session an output starts in, through `run`. When it is
/// the `remembered` session the run was asked to continue, its file must be
/// usable as the load runs, every attempt: otherwise `home` loads instead.
pub(in crate::backend::wayland) fn load_output_session(
    staged: SessionOptions,
    remembered: Option<&Path>,
    home: Option<SessionOptions>,
    mut run: impl FnMut(PersistenceOperation) -> Result<PersistenceOutcome>,
) -> Result<OutputSessionLoad> {
    let continues_remembered =
        remembered.is_some_and(|path| staged.target == SessionTarget::NamedFile(path.into()));
    let operation = if continues_remembered {
        PersistenceOperation::LoadRemembered {
            options: staged.clone(),
        }
    } else {
        PersistenceOperation::LoadConfigured {
            options: staged.clone(),
        }
    };
    let abandoned = match run(operation)? {
        PersistenceOutcome::Load(outcome) => {
            return Ok(OutputSessionLoad {
                loaded: Some((staged, outcome)),
                abandoned: None,
            });
        }
        PersistenceOutcome::RememberedUnavailable(error) => (staged.session_file_path(), error),
        other => bail!("unexpected session load outcome: {other:?}"),
    };

    let loaded = match home {
        Some(home) => match run(PersistenceOperation::LoadConfigured {
            options: home.clone(),
        })? {
            PersistenceOutcome::Load(outcome) => Some((home, outcome)),
            other => bail!("unexpected home session load outcome: {other:?}"),
        },
        None => None,
    };

    Ok(OutputSessionLoad {
        loaded,
        abandoned: Some(abandoned),
    })
}

#[cfg(test)]
mod tests;
