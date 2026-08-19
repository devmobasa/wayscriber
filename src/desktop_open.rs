//! Explicit desktop-opener invocations shared by runtime callers.
//!
//! The process broker authorizes the executable and cheap argument shape. This
//! module owns caller policy: paths stay paths, and outbound URLs must use the
//! trusted Wayscriber HTTPS host rule before they reach the broker.
//!
//! Desktop openers are spawned with [`HelperLifetime::DetachedAfterExec`]: an
//! opener's job is to leave a descendant running, so a bounded `run` that
//! SIGKILLs the process group would kill the application it just launched.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::process_broker::{HelperKind, HelperLifetime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopOpenInvocation {
    program: OsString,
    arguments: Vec<OsString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopOpenRequest {
    CaptureFolder(PathBuf),
    ConfigFile(PathBuf),
}

impl DesktopOpenRequest {
    pub(crate) fn invocation(&self) -> DesktopOpenInvocation {
        path(self.path())
    }

    pub(crate) fn path(&self) -> &Path {
        match self {
            Self::CaptureFolder(path) | Self::ConfigFile(path) => path,
        }
    }

    pub(crate) fn target_name(&self) -> &'static str {
        match self {
            Self::CaptureFolder(_) => "capture folder",
            Self::ConfigFile(_) => "config file",
        }
    }

    pub(crate) fn failure_notice(&self) -> &'static str {
        match self {
            Self::CaptureFolder(_) => "Failed to open capture folder.",
            Self::ConfigFile(_) => "Failed to open config file.",
        }
    }
}

impl DesktopOpenInvocation {
    pub(crate) fn program(&self) -> &OsStr {
        &self.program
    }

    pub(crate) fn arguments(&self) -> &[OsString] {
        &self.arguments
    }
}

/// Spawn a desktop opener and return once the broker has transferred ownership.
/// Runtime owners that exit after the action use this form so teardown cannot
/// cancel a helper that must outlive the overlay.
pub(crate) fn open(invocation: &DesktopOpenInvocation) -> Result<()> {
    let broker = crate::process_broker::current()?;
    spawn_with_broker(&broker, invocation)
}

/// Spawn a desktop opener without blocking the Wayland or tray callback that
/// requested it. The broker still authorizes the argv; DetachedAfterExec keeps
/// the launched application alive after this process exits.
///
/// Callers that own the process broker must join the returned worker before
/// tearing the broker down, or a still-in-flight spawn exchange can be cancelled.
pub(crate) fn open_in_background(
    invocation: DesktopOpenInvocation,
) -> Result<std::thread::JoinHandle<()>> {
    let broker = crate::process_broker::current()?;
    std::thread::Builder::new()
        .name("wayscriber-desktop-open".to_string())
        .spawn(move || {
            if let Err(err) = spawn_with_broker(&broker, &invocation) {
                // Do not include the target or captured output: an About report
                // URL can carry diagnostics in its fragment.
                log::warn!("Desktop opener failed: {err:#}");
            }
        })
        .context("failed to start desktop-open worker")
}

fn spawn_with_broker(
    broker: &crate::process_broker::ProcessBroker,
    invocation: &DesktopOpenInvocation,
) -> Result<()> {
    broker
        .spawn(
            HelperKind::DesktopOpen,
            HelperLifetime::DetachedAfterExec,
            invocation.program(),
            invocation.arguments(),
            Vec::new(),
        )
        .map(|_| ())
}

/// Open a local path with the platform's desktop integration.
pub(crate) fn path(path: &Path) -> DesktopOpenInvocation {
    invocation(path.as_os_str())
}

/// Open an HTTPS URL on the update-check Wayscriber host allowlist
/// (`wayscriber.com` and `www.wayscriber.com`).
pub(crate) fn trusted_url(url: &str) -> Result<DesktopOpenInvocation> {
    if !crate::update_check::is_trusted_url(url) {
        bail!("refusing to open an untrusted Wayscriber URL: {url:?}");
    }
    Ok(invocation(OsStr::new(url)))
}

fn invocation(target: &OsStr) -> DesktopOpenInvocation {
    // `cmd /C start` is intentionally absent: desktop opening must never route
    // through a shell. Wayscriber is a Wayland application, while `open`
    // preserves the existing non-shell macOS build path.
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    DesktopOpenInvocation {
        program: program.into(),
        arguments: vec![target.into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_invocation_is_one_explicit_argument_without_a_shell() {
        let invocation = path(Path::new("/tmp/Wayscriber Captures"));

        assert!(!matches!(
            invocation.program().to_str(),
            Some("sh" | "bash" | "cmd")
        ));
        assert_eq!(
            invocation.arguments(),
            [OsString::from("/tmp/Wayscriber Captures")]
        );
    }

    #[test]
    fn trusted_urls_share_the_update_check_host_allowlist() {
        let invocation = trusted_url("https://wayscriber.com/report#d=abc").unwrap();
        assert_eq!(
            invocation.arguments(),
            [OsString::from("https://wayscriber.com/report#d=abc")]
        );
        assert!(
            trusted_url("https://www.wayscriber.com/docs/getting-started/updating.html").is_ok()
        );

        for untrusted in [
            "http://wayscriber.com/report",
            "https://wayscriber.com.example/report",
            "https://example.com/report",
            "file:///etc/passwd",
        ] {
            assert!(
                trusted_url(untrusted).is_err(),
                "unexpectedly accepted {untrusted}"
            );
        }
    }
}
