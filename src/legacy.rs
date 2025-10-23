use std::env;

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
