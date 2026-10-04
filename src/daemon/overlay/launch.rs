use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::daemon::control::DaemonToggleRequest;
use crate::env_vars::{
    DESKTOP_STARTUP_ID_ENV, NO_DETACH_ENV, OVERLAY_HOME_SESSION_ENV, OVERLAY_PREFERRED_SESSION_ENV,
    OVERLAY_SESSION_REPORTS_ENV, XDG_ACTIVATION_TOKEN_ENV,
};

pub(in crate::daemon) struct OverlayLaunchRequest {
    mode: Option<String>,
    /// The session file this request asked for, which outranks any other.
    explicit_session_file: Option<PathBuf>,
    /// The daemon's startup session file, its overlays' home.
    home_session_file: Option<PathBuf>,
    freeze: bool,
    exit_after_capture: bool,
    no_exit_after_capture: bool,
    session_resume_override: Option<bool>,
    activation_token: Option<String>,
}

impl OverlayLaunchRequest {
    pub(super) fn new(
        request: Option<DaemonToggleRequest>,
        activation_token: Option<String>,
        mode: Option<&str>,
        home_session_file: Option<&Path>,
        freeze: bool,
    ) -> Self {
        let request = request.unwrap_or_default();
        let session_resume_override = request.session_resume_override();

        Self {
            mode: request.mode.or_else(|| mode.map(str::to_owned)),
            explicit_session_file: request.session_file,
            home_session_file: home_session_file.map(Path::to_path_buf),
            freeze: request.freeze || freeze,
            exit_after_capture: request.exit_after_capture,
            no_exit_after_capture: request.no_exit_after_capture,
            session_resume_override,
            activation_token,
        }
    }

    pub(super) fn mode(&self) -> Option<&str> {
        self.mode.as_deref()
    }

    /// The session file the overlay is launched with: the requested one, else
    /// home.
    pub(super) fn named_session_file(&self) -> Option<&Path> {
        self.explicit_session_file
            .as_deref()
            .or(self.home_session_file.as_deref())
    }

    pub(super) fn explicit_session_file(&self) -> Option<&Path> {
        self.explicit_session_file.as_deref()
    }

    pub(super) fn activation_token(&self) -> Option<&str> {
        self.activation_token.as_deref()
    }

    pub(super) fn into_activation_token(self) -> Option<String> {
        self.activation_token
    }

    pub(super) fn session_resume_override(&self, default: Option<bool>) -> Option<bool> {
        self.session_resume_override.or(default)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct OverlayLaunch {
    pub(super) arguments: Vec<OsString>,
    pub(super) environment: Vec<(OsString, Option<OsString>)>,
}

/// The launch for `request`. A child `generation` reports its session to the
/// daemon, which passes the session it remembers from the last report as the
/// one to continue, unless the request asked for a session file.
pub(super) fn build_overlay_launch(
    request: &OverlayLaunchRequest,
    resume_default: Option<bool>,
    generation: Option<&str>,
    remembered_session_file: Option<&Path>,
) -> OverlayLaunch {
    let mut arguments = vec![OsString::from("--active")];
    if request.freeze {
        arguments.push("--freeze".into());
    }
    if request.exit_after_capture {
        arguments.push("--exit-after-capture".into());
    } else if request.no_exit_after_capture {
        arguments.push("--no-exit-after-capture".into());
    }

    // Daemon children are already backgrounded and tracked; do not detach again.
    let mut environment = vec![(OsString::from(NO_DETACH_ENV), Some("1".into()))];
    if let Some(generation) = generation {
        environment.push((
            crate::env_vars::OVERLAY_CHILD_GENERATION_ENV.into(),
            Some(generation.into()),
        ));
    }
    if let Some(token) = request.activation_token() {
        environment.push((XDG_ACTIVATION_TOKEN_ENV.into(), Some(token.into())));
        environment.push((DESKTOP_STARTUP_ID_ENV.into(), Some(token.into())));
    } else {
        environment.push((XDG_ACTIVATION_TOKEN_ENV.into(), None));
        environment.push((DESKTOP_STARTUP_ID_ENV.into(), None));
    }
    environment.push((
        crate::RESUME_SESSION_ENV.into(),
        request
            .session_resume_override(resume_default)
            .map(|enabled| if enabled { "on".into() } else { "off".into() }),
    ));
    // Optional inputs, so an older overlay ignores them and starts as before.
    environment.extend([
        (
            OVERLAY_SESSION_REPORTS_ENV.into(),
            generation.map(|_| "1".into()),
        ),
        (
            OVERLAY_HOME_SESSION_ENV.into(),
            request.home_session_file.as_deref().map(Into::into),
        ),
        (
            OVERLAY_PREFERRED_SESSION_ENV.into(),
            remembered_session_file
                .filter(|_| request.explicit_session_file.is_none())
                .map(Into::into),
        ),
    ]);

    if let Some(mode) = request.mode() {
        arguments.push("--mode".into());
        arguments.push(mode.into());
    }
    if let Some(path) = request.named_session_file() {
        arguments.push("--session-file".into());
        arguments.push(path.as_os_str().into());
    }

    OverlayLaunch {
        arguments,
        environment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_environment_preserves_token_removal_generation_and_resume_precedence() {
        for (requested, default, expected_resume) in [
            (None, None, None),
            (None, Some(true), Some("on")),
            (None, Some(false), Some("off")),
            (Some(true), None, Some("on")),
            (Some(false), None, Some("off")),
            (Some(true), Some(false), Some("on")),
            (Some(false), Some(true), Some("off")),
        ] {
            for token in [None, Some("activation-token")] {
                for generation in [None, Some("child-generation")] {
                    let request = OverlayLaunchRequest::new(
                        requested.map(|enabled| DaemonToggleRequest {
                            resume_session: enabled,
                            no_resume_session: !enabled,
                            ..Default::default()
                        }),
                        token.map(str::to_owned),
                        None,
                        None,
                        false,
                    );

                    let launch = build_overlay_launch(&request, default, generation, None);

                    let mut expected = vec![(NO_DETACH_ENV.into(), Some("1".into()))];
                    if let Some(generation) = generation {
                        expected.push((
                            crate::env_vars::OVERLAY_CHILD_GENERATION_ENV.into(),
                            Some(generation.into()),
                        ));
                    }
                    expected.extend([
                        (XDG_ACTIVATION_TOKEN_ENV.into(), token.map(OsString::from)),
                        (DESKTOP_STARTUP_ID_ENV.into(), token.map(OsString::from)),
                        (
                            crate::RESUME_SESSION_ENV.into(),
                            expected_resume.map(OsString::from),
                        ),
                        (
                            OVERLAY_SESSION_REPORTS_ENV.into(),
                            generation.map(|_| "1".into()),
                        ),
                        (OVERLAY_HOME_SESSION_ENV.into(), None),
                        (OVERLAY_PREFERRED_SESSION_ENV.into(), None),
                    ]);
                    assert_eq!(launch.environment, expected);
                }
            }
        }
    }

    #[test]
    fn launch_offers_the_remembered_session_only_without_a_requested_file() {
        let home = "/sessions/home.wayscriber-session";
        let remembered = Path::new("/sessions/b.wayscriber-session");
        let requested = "/sessions/c.wayscriber-session";
        for (session_file, preferred) in [(None, Some(remembered)), (Some(requested), None)] {
            let request = OverlayLaunchRequest::new(
                session_file.map(|file| DaemonToggleRequest {
                    session_file: Some(file.into()),
                    ..Default::default()
                }),
                None,
                None,
                Some(Path::new(home)),
                false,
            );

            let launch = build_overlay_launch(&request, None, Some("generation"), Some(remembered));

            let value = |name: &str| {
                launch
                    .environment
                    .iter()
                    .find(|(key, _)| key == name)
                    .and_then(|(_, value)| value.clone())
            };
            assert_eq!(value(OVERLAY_SESSION_REPORTS_ENV), Some("1".into()));
            assert_eq!(value(OVERLAY_HOME_SESSION_ENV), Some(home.into()));
            assert_eq!(
                value(OVERLAY_PREFERRED_SESSION_ENV),
                preferred.map(Into::into)
            );
            // The command line stays one an older overlay understands.
            assert_eq!(
                launch.arguments,
                ["--active", "--session-file", session_file.unwrap_or(home)].map(OsString::from)
            );
        }
    }
}
