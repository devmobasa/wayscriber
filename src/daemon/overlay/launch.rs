use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::daemon::control::DaemonToggleRequest;
use crate::env_vars::{DESKTOP_STARTUP_ID_ENV, NO_DETACH_ENV, XDG_ACTIVATION_TOKEN_ENV};

pub(in crate::daemon) struct OverlayLaunchRequest {
    mode: Option<String>,
    named_session_file: Option<PathBuf>,
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
        named_session_file: Option<&Path>,
        freeze: bool,
    ) -> Self {
        let request = request.unwrap_or_default();
        let session_resume_override = request.session_resume_override();

        Self {
            mode: request.mode.or_else(|| mode.map(str::to_owned)),
            named_session_file: request
                .session_file
                .or_else(|| named_session_file.map(Path::to_path_buf)),
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

    pub(super) fn named_session_file(&self) -> Option<&Path> {
        self.named_session_file.as_deref()
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

pub(super) fn build_overlay_launch(
    request: &OverlayLaunchRequest,
    resume_default: Option<bool>,
    generation: Option<&str>,
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

                    let launch = build_overlay_launch(&request, default, generation);

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
                    ]);
                    assert_eq!(launch.environment, expected);
                }
            }
        }
    }
}
