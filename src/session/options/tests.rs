use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::config::options_from_config;
use super::identifiers::{resolve_display_id, sanitize_identifier};
use super::types::SessionOptions;
use crate::config::{SessionConfig, SessionStorageMode};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn sanitize_identifier_replaces_non_alphanumeric() {
    assert_eq!(sanitize_identifier("DP-1"), "DP_1");
    assert_eq!(sanitize_identifier("output:name"), "output_name");
    assert_eq!(sanitize_identifier("abc/def-01"), "abc_def_01");
}

#[test]
fn sanitize_identifier_empty_defaults_to_default() {
    assert_eq!(sanitize_identifier(""), "default");
}

#[test]
fn resolve_display_id_prefers_argument_and_uses_env_fallback() {
    use std::env;

    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let prev = env::var_os("WAYLAND_DISPLAY");
    // SAFETY: serialized via ENV_MUTEX
    unsafe {
        env::set_var("WAYLAND_DISPLAY", "wayland-0");
    }

    let from_arg = resolve_display_id(Some("custom-display"));
    assert_eq!(from_arg, "custom_display");

    let from_env = resolve_display_id(None);
    assert_eq!(from_env, "wayland_0");

    match prev {
        Some(v) => unsafe { env::set_var("WAYLAND_DISPLAY", v) },
        None => unsafe { env::remove_var("WAYLAND_DISPLAY") },
    }
}

#[test]
fn options_from_config_clamps_max_persisted_undo_depth() {
    let mut cfg = SessionConfig {
        max_persisted_undo_depth: Some(5),
        storage: SessionStorageMode::Config,
        ..SessionConfig::default()
    };

    let opts = options_from_config(&cfg, Path::new("/tmp"), Some("display")).unwrap();
    assert_eq!(opts.max_persisted_undo_depth, Some(10));

    cfg.max_persisted_undo_depth = Some(2_000);
    let opts2 = options_from_config(&cfg, Path::new("/tmp"), Some("display")).unwrap();
    assert_eq!(opts2.max_persisted_undo_depth, Some(1_000));
}

#[test]
fn effective_history_limit_respects_persist_history_flag() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.persist_history = false;
    options.max_persisted_undo_depth = Some(10);

    let limit = options.effective_history_limit(50);
    assert_eq!(limit, 0);
}

#[test]
fn effective_history_limit_clamps_to_runtime_limit() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.persist_history = true;
    options.max_persisted_undo_depth = Some(5);

    let limit = options.effective_history_limit(3);
    assert_eq!(limit, 3);
}

#[test]
fn set_output_identity_reports_changes() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.per_output = true;

    assert!(options.set_output_identity(Some("DP-1")));
    assert_eq!(options.output_identity.as_deref(), Some("DP_1"));
    assert!(!options.set_output_identity(Some("DP-1")));
    assert!(options.set_output_identity(None));
    assert!(options.output_identity.is_none());
}
