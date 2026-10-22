const DEFAULT_APP_ID: &str = "wayscriber";

use std::env;

use crate::env_vars::{APP_ID_ENV, PORTAL_APP_ID_ENV};

pub(crate) fn runtime_app_id() -> String {
    env::var(APP_ID_ENV)
        .ok()
        .as_deref()
        .and_then(non_empty_trimmed)
        .or_else(|| {
            env::var(PORTAL_APP_ID_ENV)
                .ok()
                .as_deref()
                .and_then(non_empty_trimmed)
        })
        .unwrap_or_else(|| DEFAULT_APP_ID.to_string())
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
