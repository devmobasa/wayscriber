use std::env;

use crate::env_vars::WAYLAND_DISPLAY_ENV;

pub(super) fn sanitize_identifier(raw: &str) -> String {
    if raw.is_empty() {
        return "default".to_string();
    }

    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

pub(super) fn resolve_display_id(display_id: Option<&str>) -> String {
    if let Some(display_id) = display_id {
        return sanitize_identifier(display_id);
    }

    let environment_display = env::var(WAYLAND_DISPLAY_ENV).ok();
    match environment_display.as_deref() {
        Some(value) => {
            log::info!("Session display id from {WAYLAND_DISPLAY_ENV}='{value}'");
        }
        None => {
            log::info!("Session display id fallback to 'default' ({WAYLAND_DISPLAY_ENV} missing)");
        }
    }
    resolve_display_id_with_env(None, environment_display.as_deref())
}

pub(super) fn resolve_display_id_with_env(
    display_id: Option<&str>,
    environment_display: Option<&str>,
) -> String {
    display_id
        .or(environment_display)
        .map_or_else(|| "default".to_string(), sanitize_identifier)
}
