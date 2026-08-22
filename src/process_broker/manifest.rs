use std::ffi::OsStr;
use std::process::Command;

use anyhow::{Result, anyhow, bail};

use super::wire::{HelperKind, MAX_ARGUMENT_BYTES, MAX_ARGUMENTS, MAX_INPUT_BYTES, OsWire};

pub(super) fn input_cap(kind: HelperKind) -> usize {
    if matches!(kind, HelperKind::WlCopy) {
        super::max_publish_bytes()
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
            // `--disable` / `--no-config` only suppress the default rc files.
            // A later `--config` / `-K` would re-open that hole.
            let rest = arguments.get(1..).unwrap_or(&[]);
            if basename == "curl" {
                if rest
                    .iter()
                    .any(|argument| is_curl_config_argument(&argument.0))
                {
                    bail!("update fetcher must not re-enable curl configuration after --disable");
                }
            } else if rest
                .iter()
                .any(|argument| is_wget_config_argument(&argument.0))
            {
                bail!(
                    "update fetcher must not re-enable wget configuration or execute directives after --no-config"
                );
            }
        }
        HelperKind::DesktopOpen => {
            let [target] = arguments else {
                bail!("desktop opener requires exactly one target argument");
            };
            if target.0.starts_with(b"-") {
                bail!("desktop opener target must not be an option");
            }
            match std::str::from_utf8(&target.0) {
                Ok(target)
                    if looks_like_uri(target) && !crate::update_check::is_trusted_url(target) =>
                {
                    bail!("desktop opener URL is not a trusted Wayscriber HTTPS URL");
                }
                // xdg-open parses schemes bytewise. Undecodable targets that
                // still look like URIs must not skip the trusted-host gate.
                Err(_) if looks_like_uri_bytes(&target.0) => {
                    bail!("desktop opener URL must be valid UTF-8");
                }
                _ => {}
            }
        }
        HelperKind::Systemctl => {
            if arguments.first().map(|argument| argument.0.as_slice()) != Some(b"--user") {
                bail!("systemctl helper is restricted to the user service manager");
            }
            // `--user` is not sticky against a later `--system` / `--global`,
            // and `--machine` / `-M` can reach the system-scope bus.
            if arguments
                .iter()
                .skip(1)
                .any(|argument| is_systemctl_non_user_manager_flag(&argument.0))
            {
                bail!("systemctl helper must not target the system, global, or machine manager");
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_curl_config_argument(argument: &[u8]) -> bool {
    if long_option_matches(argument, b"config") {
        return true;
    }
    // Short options and clusters: `-K`, `-Kfile`, `-sK/path`, `-vK`, …
    argument.starts_with(b"-") && !argument.starts_with(b"--") && argument[1..].contains(&b'K')
}

fn is_wget_config_argument(argument: &[u8]) -> bool {
    // `--no-config` skips rc files, but `--execute` / `-e` still run .wgetrc
    // directives (headers, output, etc.) from the command line.
    if long_option_matches(argument, b"config") || long_option_matches(argument, b"execute") {
        return true;
    }
    // Short options and clusters: `-e`, `-ecommand`, `-qe…`, …
    argument.starts_with(b"-") && !argument.starts_with(b"--") && argument[1..].contains(&b'e')
}

fn is_systemctl_non_user_manager_flag(argument: &[u8]) -> bool {
    // systemd accepts unique prefixes (`--syst`, `--glob`, `--mach`), not only
    // full forms. `-M` / short clusters with `M` select a machine bus.
    if long_option_matches(argument, b"system")
        || long_option_matches(argument, b"global")
        || long_option_matches(argument, b"machine")
    {
        return true;
    }
    argument.starts_with(b"-") && !argument.starts_with(b"--") && argument[1..].contains(&b'M')
}

/// True when `argument` is `--name`, `--name=…`, or a non-empty unique prefix of
/// `--name` (the form getopt-style parsers accept).
fn long_option_matches(argument: &[u8], name: &[u8]) -> bool {
    let Some(rest) = argument.strip_prefix(b"--") else {
        return false;
    };
    let option = rest.split(|&byte| byte == b'=').next().unwrap_or(rest);
    !option.is_empty() && name.starts_with(option)
}

fn looks_like_uri(value: &str) -> bool {
    looks_like_uri_bytes(value.as_bytes())
}

fn looks_like_uri_bytes(value: &[u8]) -> bool {
    let Some(colon) = value.iter().position(|&byte| byte == b':') else {
        return false;
    };
    let scheme = &value[..colon];
    let mut bytes = scheme.iter().copied();
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
