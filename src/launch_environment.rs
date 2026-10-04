//! The environment the process was launched with.
//!
//! A library can change the live environment before `main` runs. GTK 4.16 and
//! later save `XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID` for their own
//! windows and unset both as soon as libgtk-4 loads (GTK 4.14 unsets only
//! `DESKTOP_STARTUP_ID`), so with the `toolbar-gtk` feature `std::env::var` may
//! no longer see a startup token the launcher or the daemon passed. The
//! kernel's copy of the launch environment, `/proc/self/environ`, keeps the
//! original values. A child process inherits only the live environment, so a
//! relaunch forwards these variables explicitly.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::Path;

use crate::env_vars::{DESKTOP_STARTUP_ID_ENV, XDG_ACTIVATION_TOKEN_ENV};

const LAUNCH_ENVIRONMENT: &str = "/proc/self/environ";

/// The startup-notification variables a launcher passes for the first window,
/// in the order they are tried. The daemon sets both to the same token.
pub(crate) const STARTUP_NOTIFICATION_VARIABLES: [&str; 2] =
    [XDG_ACTIVATION_TOKEN_ENV, DESKTOP_STARTUP_ID_ENV];

/// `name`'s value as the process was launched, the first one if it was passed
/// twice. Without `/proc` this falls back to the live environment, where a
/// linked GTK may already have unset a startup token.
pub(crate) fn var_os(name: &str) -> Option<OsString> {
    var_os_in(Path::new(LAUNCH_ENVIRONMENT), name)
}

fn var_os_in(launch_environment: &Path, name: &str) -> Option<OsString> {
    match std::fs::read(launch_environment) {
        Ok(environment) => {
            value_in(&environment, name).map(|value| OsString::from_vec(value.to_vec()))
        }
        Err(_) => std::env::var_os(name),
    }
}

/// The activation token a launcher passed for the first window: the first of
/// [`STARTUP_NOTIFICATION_VARIABLES`] with a value. A value is trimmed, and an
/// empty one counts as absent.
pub(crate) fn startup_activation_token() -> Option<String> {
    startup_activation_token_from(var_os)
}

fn startup_activation_token_from(lookup: impl Fn(&str) -> Option<OsString>) -> Option<String> {
    STARTUP_NOTIFICATION_VARIABLES.into_iter().find_map(|name| {
        let value = lookup(name)?.into_string().ok()?;
        let value = value.trim();

        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// The value of the first `name=value` entry in a NUL-separated environment.
fn value_in<'a>(environment: &'a [u8], name: &str) -> Option<&'a [u8]> {
    environment
        .split(|byte| *byte == 0)
        .find_map(|entry| entry.strip_prefix(name.as_bytes())?.strip_prefix(b"="))
}

#[cfg(test)]
mod tests;
