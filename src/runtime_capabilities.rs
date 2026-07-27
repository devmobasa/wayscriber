use crate::backend::wayland::input_monitor::system_input_available;
use crate::shortcut_hint::portal_runtime_supported;

pub const RUNTIME_CAPABILITIES_FLAG: &str = "--runtime-capabilities";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCapabilities {
    pub portal: bool,
    /// Whether system-wide input capture for the input HUD is compiled in and
    /// `/dev/input` is readable by this process.
    pub input_monitor: bool,
}

pub fn current_runtime_capabilities() -> RuntimeCapabilities {
    RuntimeCapabilities {
        portal: portal_runtime_supported(),
        input_monitor: system_input_available(),
    }
}

pub fn render_runtime_capabilities(capabilities: RuntimeCapabilities) -> String {
    format!(
        "portal={}\ninput_monitor={}\n",
        capabilities.portal, capabilities.input_monitor
    )
}

pub fn parse_runtime_capabilities(output: &str) -> Result<RuntimeCapabilities, String> {
    let mut portal = None;
    // Older overlays predate the key; absence means "no system input capture"
    // rather than a malformed report, so the round trip stays compatible.
    let mut input_monitor = false;
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("Invalid runtime capability line: {line}"));
        };
        match key {
            "portal" => portal = Some(parse_bool(value)?),
            "input_monitor" => input_monitor = parse_bool(value)?,
            _ => {}
        }
    }

    Ok(RuntimeCapabilities {
        portal: portal.ok_or_else(|| "Missing portal runtime capability".to_string())?,
        input_monitor,
    })
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("Invalid boolean runtime capability value: {value}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeCapabilities, parse_runtime_capabilities, render_runtime_capabilities};

    #[test]
    fn render_runtime_capabilities_outputs_key_value_lines() {
        assert_eq!(
            render_runtime_capabilities(RuntimeCapabilities {
                portal: true,
                input_monitor: false,
            }),
            "portal=true\ninput_monitor=false\n"
        );
    }

    #[test]
    fn parse_runtime_capabilities_reads_portal_support() {
        assert_eq!(
            parse_runtime_capabilities("portal=false\ninput_monitor=false\n").unwrap(),
            RuntimeCapabilities {
                portal: false,
                input_monitor: false,
            }
        );
    }

    #[test]
    fn runtime_capabilities_round_trip_preserves_every_key() {
        for capabilities in [
            RuntimeCapabilities {
                portal: true,
                input_monitor: true,
            },
            RuntimeCapabilities {
                portal: false,
                input_monitor: true,
            },
        ] {
            let rendered = render_runtime_capabilities(capabilities);
            assert_eq!(parse_runtime_capabilities(&rendered).unwrap(), capabilities);
        }
    }

    /// A report from an overlay that predates the key still parses; the missing
    /// capability reads as unavailable.
    #[test]
    fn parse_runtime_capabilities_defaults_missing_input_monitor_to_false() {
        assert_eq!(
            parse_runtime_capabilities("portal=true\n").unwrap(),
            RuntimeCapabilities {
                portal: true,
                input_monitor: false,
            }
        );
    }

    #[test]
    fn parse_runtime_capabilities_requires_portal_line() {
        assert!(
            parse_runtime_capabilities("other=true\n")
                .unwrap_err()
                .contains("Missing portal")
        );
    }
}
