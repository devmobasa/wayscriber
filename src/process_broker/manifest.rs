use std::ffi::OsStr;
use std::process::Command;

use anyhow::{Result, anyhow, bail};

use super::wire::{
    HelperKind, MAX_ARGUMENT_BYTES, MAX_ARGUMENTS, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES, OsWire,
};

pub(super) fn input_cap(kind: HelperKind) -> usize {
    if matches!(kind, HelperKind::WlCopy) {
        MAX_OUTPUT_BYTES
    } else {
        MAX_INPUT_BYTES
    }
}

pub(super) fn supports_prefix_output(kind: HelperKind) -> bool {
    if matches!(kind, HelperKind::WlPaste) {
        return true;
    }
    #[cfg(test)]
    if matches!(kind, HelperKind::TestShell) {
        return true;
    }
    false
}

pub(super) fn validate(
    kind: HelperKind,
    program: &OsWire,
    arguments: &[OsWire],
    environment: &[(OsWire, Option<OsWire>)],
    input: &[u8],
) -> Result<()> {
    if arguments.len() > MAX_ARGUMENTS
        || arguments.iter().map(|value| value.0.len()).sum::<usize>() > MAX_ARGUMENT_BYTES
        || environment.len() > 32
        || input.len() > input_cap(kind)
    {
        bail!("broker request exceeds manifest bounds");
    }
    let program = program.clone().into_os();
    let basename = std::path::Path::new(&program)
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| anyhow!("broker program has no UTF-8 basename"))?
        .to_owned();
    let allowed = match kind {
        HelperKind::Overlay | HelperKind::InitialDetach | HelperKind::About => {
            basename == "wayscriber" || basename.starts_with("wayscriber-")
        }
        HelperKind::CapabilityProbe => matches!(
            basename.as_str(),
            "grim" | "hyprctl" | "slurp" | "wl-copy" | "wl-paste" | "zenity" | "kdialog"
        ),
        HelperKind::Grim => basename == "grim",
        HelperKind::Hyprctl => basename == "hyprctl",
        HelperKind::Slurp => basename == "slurp",
        HelperKind::Tesseract => basename == "tesseract",
        HelperKind::WlPaste => basename == "wl-paste",
        HelperKind::WlCopy => basename == "wl-copy",
        HelperKind::SessionZenity => basename == "zenity",
        HelperKind::SessionKdialog => basename == "kdialog",
        HelperKind::Gsettings => basename == "gsettings",
        HelperKind::Systemctl => basename == "systemctl",
        HelperKind::Configurator => std::env::var_os(crate::env_vars::CONFIGURATOR_ENV)
            .map_or_else(
                || basename.contains("configurator"),
                |configured| configured == program,
            ),
        HelperKind::DesktopOpen => matches!(basename.as_str(), "xdg-open" | "open"),
        HelperKind::UpdateFetcher => matches!(basename.as_str(), "curl" | "wget"),
        #[cfg(test)]
        HelperKind::TestSleep => basename == "sleep",
        #[cfg(test)]
        HelperKind::TestCat => basename == "cat",
        #[cfg(test)]
        HelperKind::TestShell => basename == "sh",
    };
    if !allowed {
        bail!("program {basename:?} is not allowed for helper kind {kind:?}");
    }
    validate_arguments(kind, &basename, arguments)?;
    for (name, _) in environment {
        let name = std::str::from_utf8(&name.0)?;
        if !matches!(
            name,
            "WAYSCRIBER_NO_DETACH"
                | "WAYSCRIBER_DETACHED"
                | "XDG_ACTIVATION_TOKEN"
                | "DESKTOP_STARTUP_ID"
                | "WAYSCRIBER_RESUME_SESSION"
                | "WAYSCRIBER_OVERLAY_CHILD_GENERATION"
        ) {
            bail!("environment key {name:?} is not broker-allowed");
        }
    }
    Ok(())
}

/// Cheap exec-gate checks for helpers whose safety relies on one indispensable
/// argument. Complete argv content remains the caller's policy.
fn validate_arguments(kind: HelperKind, basename: &str, arguments: &[OsWire]) -> Result<()> {
    match kind {
        HelperKind::UpdateFetcher => {
            let required_first = if basename == "curl" {
                b"--disable".as_slice()
            } else {
                b"--no-config".as_slice()
            };
            if arguments.first().map(|argument| argument.0.as_slice()) != Some(required_first) {
                bail!("update fetcher must disable user configuration in argument one");
            }
        }
        HelperKind::DesktopOpen => {
            let [target] = arguments else {
                bail!("desktop opener requires exactly one target argument");
            };
            if target.0.starts_with(b"-") {
                bail!("desktop opener target must not be an option");
            }
            if let Ok(target) = std::str::from_utf8(&target.0)
                && looks_like_uri(target)
                && !crate::update_check::is_trusted_url(target)
            {
                bail!("desktop opener URL is not a trusted Wayscriber HTTPS URL");
            }
        }
        HelperKind::Systemctl
            if arguments.first().map(|argument| argument.0.as_slice()) != Some(b"--user") =>
        {
            bail!("systemctl helper is restricted to the user service manager");
        }
        _ => {}
    }
    Ok(())
}

fn looks_like_uri(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    let mut bytes = scheme.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

pub(super) fn command(
    program: OsWire,
    arguments: Vec<OsWire>,
    environment: Vec<(OsWire, Option<OsWire>)>,
) -> Command {
    let mut command = Command::new(program.into_os());
    command.args(arguments.into_iter().map(OsWire::into_os));
    for (name, value) in environment {
        let name = name.into_os();
        if let Some(value) = value {
            command.env(name, value.into_os());
        } else {
            command.env_remove(name);
        }
    }
    command
}
