use std::sync::atomic::{AtomicU8, Ordering};

pub const RESUME_SESSION_ENV: &str = "WAYSCRIBER_RESUME_SESSION";
pub const SESSION_OVERRIDE_FOLLOW_CONFIG: u8 = 0;
pub const SESSION_OVERRIDE_FORCE_ON: u8 = 1;
pub const SESSION_OVERRIDE_FORCE_OFF: u8 = 2;

pub static SESSION_RESUME_OVERRIDE: AtomicU8 = AtomicU8::new(SESSION_OVERRIDE_FOLLOW_CONFIG);

pub fn encode_session_override(value: Option<bool>) -> u8 {
    match value {
        Some(true) => SESSION_OVERRIDE_FORCE_ON,
        Some(false) => SESSION_OVERRIDE_FORCE_OFF,
        None => SESSION_OVERRIDE_FOLLOW_CONFIG,
    }
}

pub fn decode_session_override(raw: u8) -> Option<bool> {
    match raw {
        SESSION_OVERRIDE_FORCE_ON => Some(true),
        SESSION_OVERRIDE_FORCE_OFF => Some(false),
        _ => None,
    }
}

pub fn set_runtime_session_override(value: Option<bool>) {
    SESSION_RESUME_OVERRIDE.store(encode_session_override(value), Ordering::Release);
}

pub fn runtime_session_override() -> Option<bool> {
    decode_session_override(SESSION_RESUME_OVERRIDE.load(Ordering::Acquire))
}
