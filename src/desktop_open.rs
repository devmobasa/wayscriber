//! Explicit desktop-opener invocations shared by runtime callers.
//!
//! The process broker authorizes the executable and cheap argument shape. This
//! module owns caller policy: paths stay paths, and outbound URLs must use the
//! trusted Wayscriber HTTPS host rule before they reach the broker.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};

const HELPER_TIMEOUT: Duration = Duration::from_secs(10);
const OUTPUT_CAP: usize = 16 * 1024;

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

/// Complete a bounded desktop-open operation before the caller continues.
/// Runtime owners that exit after the action use this form so broker teardown
/// cannot cancel the helper they just requested.
pub(crate) fn open(invocation: &DesktopOpenInvocation) -> Result<()> {
    let broker = crate::process_broker::current()?;
    run_with_broker(&broker, invocation)
}

/// Run a desktop opener without blocking the Wayland or tray callback that
/// requested it. The broker still owns the complete helper lifetime and
/// enforces the timeout/output policy inside the worker.
pub(crate) fn open_in_background(invocation: DesktopOpenInvocation) -> Result<()> {
    let broker = crate::process_broker::current()?;
    std::thread::Builder::new()
        .name("wayscriber-desktop-open".to_string())
        .spawn(move || {
            if let Err(err) = run_with_broker(&broker, &invocation) {
                // Do not include the target or captured output: an About report
                // URL can carry diagnostics in its fragment.
                log::warn!("Desktop opener failed: {err:#}");
            }
        })
        .context("failed to start desktop-open worker")?;
    Ok(())
}

fn run_with_broker(
    broker: &crate::process_broker::ProcessBroker,
    invocation: &DesktopOpenInvocation,
) -> Result<()> {
    run_with(invocation, |program, arguments, timeout, output_cap| {
        broker.run(
            crate::process_broker::HelperKind::DesktopOpen,
            program,
            arguments,
            Vec::new(),
            timeout,
            output_cap,
        )
    })
}

fn run_with(
    invocation: &DesktopOpenInvocation,
    run: impl FnOnce(
        &OsStr,
        &[OsString],
        Duration,
        usize,
    ) -> Result<crate::process_broker::BrokerOutput>,
) -> Result<()> {
    let output = run(
        invocation.program(),
        invocation.arguments(),
        HELPER_TIMEOUT,
        OUTPUT_CAP,
    )?;
    if output.timed_out {
        bail!("desktop opener timed out");
    }
    if output.status != 0 {
        bail!(
            "desktop opener exited unsuccessfully with status {}",
            output.status
        );
    }
    Ok(())
}

/// Open a local path with the platform's desktop integration.
pub(crate) fn path(path: &Path) -> DesktopOpenInvocation {
    invocation(path.as_os_str())
}

/// Open an HTTPS URL on the update check's exact Wayscriber host allowlist.
#[cfg(any(feature = "tray", test))]
pub(crate) fn trusted_wayscriber_url(url: &str) -> Result<DesktopOpenInvocation> {
    if !crate::update_check::is_trusted_url(url) {
        bail!("refusing to open an untrusted Wayscriber URL: {url:?}");
    }
    Ok(invocation(OsStr::new(url)))
}

/// About content is stricter than update metadata: every compiled-in link uses
/// the primary `https://wayscriber.com` origin.
pub(crate) fn about_url(url: &str) -> Result<DesktopOpenInvocation> {
    let Some(tail) = url.strip_prefix("https://wayscriber.com") else {
        bail!("refusing to open an untrusted About URL: {url:?}");
    };
    if !(tail.is_empty() || tail.starts_with(['/', '?', '#'])) {
        bail!("refusing to open an untrusted About URL: {url:?}");
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
    fn desktop_open_run_declares_argv_timeout_and_output_cap() {
        let invocation = path(Path::new("/tmp/Wayscriber Captures"));
        let mut observed = None;

        run_with(&invocation, |program, arguments, timeout, output_cap| {
            observed = Some((program.to_owned(), arguments.to_vec(), timeout, output_cap));
            Ok(crate::process_broker::BrokerOutput {
                status: 0,
                stdout: Vec::new(),
                stderr: Vec::new(),
                timed_out: false,
                stdout_limit_reached: false,
            })
        })
        .unwrap();

        let (program, arguments, timeout, output_cap) = observed.unwrap();
        assert!(!matches!(program.to_str(), Some("sh" | "bash" | "cmd")));
        assert_eq!(arguments, [OsString::from("/tmp/Wayscriber Captures")]);
        assert_eq!(timeout, Duration::from_secs(10));
        assert_eq!(output_cap, 16 * 1024);
    }

    #[test]
    fn desktop_open_run_surfaces_timeout_and_nonzero_exit() {
        let invocation = path(Path::new("/tmp/capture"));
        let output = |status, timed_out| crate::process_broker::BrokerOutput {
            status,
            stdout: Vec::new(),
            stderr: Vec::new(),
            timed_out,
            stdout_limit_reached: false,
        };

        let timeout = run_with(&invocation, |_, _, _, _| Ok(output(137, true))).unwrap_err();
        assert!(timeout.to_string().contains("timed out"));

        let nonzero = run_with(&invocation, |_, _, _, _| Ok(output(4, false))).unwrap_err();
        assert!(nonzero.to_string().contains("status 4"));
    }

    #[test]
    fn about_urls_require_https_and_an_exact_wayscriber_host() {
        let invocation = about_url("https://wayscriber.com/report#d=abc").unwrap();
        assert_eq!(
            invocation.arguments(),
            [OsString::from("https://wayscriber.com/report#d=abc")]
        );

        for untrusted in [
            "http://wayscriber.com/report",
            "https://wayscriber.com.example/report",
            "https://www.wayscriber.com/report",
            "https://example.com/report",
            "file:///etc/passwd",
        ] {
            assert!(
                about_url(untrusted).is_err(),
                "unexpectedly accepted {untrusted}"
            );
        }

        assert!(trusted_wayscriber_url("https://www.wayscriber.com/docs/").is_ok());
    }
}
