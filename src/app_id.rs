const DEFAULT_APP_ID: &str = "wayscriber";
const APP_ID_ENV: &str = "WAYSCRIBER_APP_ID";
const PORTAL_APP_ID_ENV: &str = "WAYSCRIBER_PORTAL_APP_ID";

pub(crate) fn runtime_app_id() -> String {
    std::env::var(APP_ID_ENV)
        .ok()
        .and_then(non_empty_trimmed)
        .or_else(|| {
            std::env::var(PORTAL_APP_ID_ENV)
                .ok()
                .and_then(non_empty_trimmed)
        })
        .unwrap_or_else(|| DEFAULT_APP_ID.to_string())
}

fn non_empty_trimmed(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
