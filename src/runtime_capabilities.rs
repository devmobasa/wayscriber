use crate::shortcut_hint::portal_runtime_supported;

pub const RUNTIME_CAPABILITIES_FLAG: &str = "--runtime-capabilities";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCapabilities {
    pub portal: bool,
}

pub fn current_runtime_capabilities() -> RuntimeCapabilities {
    RuntimeCapabilities {
        portal: portal_runtime_supported(),
    }
}

pub fn render_runtime_capabilities(capabilities: RuntimeCapabilities) -> String {
    format!("portal={}\n", capabilities.portal)
}

pub fn parse_runtime_capabilities(output: &str) -> Result<RuntimeCapabilities, String> {
    let mut portal = None;
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("Invalid runtime capability line: {line}"));
        };
        if key == "portal" {
            portal = Some(parse_bool(value)?);
        }
    }

    Ok(RuntimeCapabilities {
        portal: portal.ok_or_else(|| "Missing portal runtime capability".to_string())?,
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
            render_runtime_capabilities(RuntimeCapabilities { portal: true }),
            "portal=true\n"
        );
    }

    #[test]
    fn parse_runtime_capabilities_reads_portal_support() {
        assert_eq!(
            parse_runtime_capabilities("portal=false\n").unwrap(),
            RuntimeCapabilities { portal: false }
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
