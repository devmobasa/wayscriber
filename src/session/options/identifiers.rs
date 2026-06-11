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
    if let Some(id) = display_id {
        return sanitize_identifier(id);
    }

    match env::var(WAYLAND_DISPLAY_ENV) {
        Ok(value) => {
            log::info!("Session display id from {WAYLAND_DISPLAY_ENV}='{}'", value);
            sanitize_identifier(&value)
        }
        Err(_) => {
            log::info!("Session display id fallback to 'default' ({WAYLAND_DISPLAY_ENV} missing)");
            "default".to_string()
        }
    }
}
