//! Explicit desktop-opener invocations shared by runtime callers.
//!
//! The process broker authorizes the executable and cheap argument shape. This
//! module owns caller policy: paths stay paths, and outbound URLs must use the
//! trusted Wayscriber HTTPS host rule before they reach the broker.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopOpenInvocation {
    program: OsString,
    arguments: Vec<OsString>,
}

impl DesktopOpenInvocation {
    pub(crate) fn program(&self) -> &OsStr {
        &self.program
    }

    pub(crate) fn arguments(&self) -> &[OsString] {
        &self.arguments
    }
}

/// Open a local path with the platform's desktop integration.
pub(crate) fn path(path: &Path) -> DesktopOpenInvocation {
    invocation(path.as_os_str())
}

/// Open an HTTPS URL on the update check's exact Wayscriber host allowlist.
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
