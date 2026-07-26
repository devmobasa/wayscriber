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
        HelperKind::WlPaste => basename == "wl-paste",
        HelperKind::WlCopy => basename == "wl-copy",
        HelperKind::SessionZenity => basename == "zenity",
        HelperKind::SessionKdialog => basename == "kdialog",
        HelperKind::Gsettings => basename == "gsettings",
        HelperKind::Configurator => configurator_program_allowed(
            &program,
            &basename,
            std::env::var_os(crate::env_vars::CONFIGURATOR_ENV).as_deref(),
        ),
        HelperKind::DesktopOpen => matches!(basename.as_str(), "xdg-open" | "open" | "cmd"),
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

pub(super) fn configurator_program_allowed(
    program: &OsStr,
    basename: &str,
    configured: Option<&OsStr>,
) -> bool {
    configured.map_or_else(
        || basename.contains("configurator"),
        |configured| configured == program,
    )
}

pub(super) fn command(
    program: OsWire,
    arguments: Vec<OsWire>,
    environment: Vec<(OsWire, Option<OsWire>)>,
) -> Command {
    let mut command = Command::new(program.into_os());
    command.args(arguments.into_iter().map(OsWire::into_os));
    command
        .env_remove(super::wire::BROKER_FD_ENV)
        .env_remove(super::wire::BROKER_SHUTDOWN_FD_ENV)
        .env_remove(super::wire::BROKER_TOKEN_ENV)
        .env_remove(crate::env_vars::DAEMON_WATCHDOG_FD_ENV);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_commands_strip_every_private_bootstrap_marker() {
        let command = command(
            OsWire::from_os(OsStr::new("true"))
                .expect("manifest fixture encodes its ASCII helper program"),
            Vec::new(),
            Vec::new(),
        );
        let removed = command
            .get_envs()
            .filter_map(|(name, value)| value.is_none().then_some(name))
            .collect::<Vec<_>>();

        for marker in [
            super::super::wire::BROKER_FD_ENV,
            super::super::wire::BROKER_SHUTDOWN_FD_ENV,
            super::super::wire::BROKER_TOKEN_ENV,
            crate::env_vars::DAEMON_WATCHDOG_FD_ENV,
        ] {
            assert!(
                removed.contains(&OsStr::new(marker)),
                "helper command retained private marker {marker}"
            );
        }
    }
}
