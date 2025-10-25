use log::{error, info};
use std::env;
use std::process::{Command, Stdio};

/// Environment variable set by the legacy shim (`hyprmarker`) before invoking the real binary.
pub const LEGACY_ALIAS_ENV: &str = "WAYSCRIBER_LEGACY_INVOCATION";

/// Environment variable users can set to silence rename warnings during scripted runs.
pub const LEGACY_SILENCE_ENV: &str = "HYPRMARKER_SILENCE_RENAME";

/// Returns the value provided by the legacy shim, if the binary was launched via compatibility alias.
pub fn alias_invocation() -> Option<String> {
    env::var(LEGACY_ALIAS_ENV).ok()
}

/// Returns true if rename warnings should be suppressed for the current process.
pub fn warnings_suppressed() -> bool {
    env::var_os(LEGACY_SILENCE_ENV).is_some()
}

/// Returns override value for the configurator binary, checking both new and legacy env vars.
pub fn configurator_override() -> Option<String> {
    env::var("WAYSCRIBER_CONFIGURATOR")
        .ok()
        .or_else(|| env::var("HYPRMARKER_CONFIGURATOR").ok())
}

/// Returns the configured configurator binary path, falling back to default.
pub fn default_configurator_binary() -> String {
    configurator_override().unwrap_or_else(|| "wayscriber-configurator".to_string())
}

/// Launch the configurator binary, logging success or failure.
pub fn launch_configurator(binary_override: Option<&str>) -> std::io::Result<()> {
    let binary = binary_override
        .map(|s| s.to_string())
        .unwrap_or_else(default_configurator_binary);

    match Command::new(&binary)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => {
            info!(
                "Launched wayscriber-configurator (binary: {binary}, pid: {})",
                child.id()
            );
            Ok(())
        }
        Err(err) => {
            error!(
                "Failed to launch wayscriber-configurator using '{}': {}",
                binary, err
            );
            error!(
                "Set WAYSCRIBER_CONFIGURATOR (or legacy HYPRMARKER_CONFIGURATOR) to override the executable path if needed."
            );
            Err(err)
        }
    }
}
