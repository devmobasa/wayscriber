use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
#[cfg(unix)]
use std::{
    ffi::CString,
    os::unix::{ffi::OsStrExt, fs::symlink},
};

use super::config::options_from_config;
use super::identifiers::{resolve_display_id, sanitize_identifier};
use super::types::{SessionOptions, SessionTarget};
use super::validation;
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

#[test]
fn named_file_target_uses_exact_path_and_appended_sidecars() {
    let path = PathBuf::from("/tmp/lecture-04.wayscriber-session");
    let mut options = SessionOptions::new(PathBuf::from("/ignored"), "display");
    options.per_output = true;
    options.set_output_identity(Some("DP-1"));

    options.set_named_file_target(path.clone());

    assert_eq!(options.target, SessionTarget::NamedFile(path.clone()));
    assert_eq!(options.base_dir, PathBuf::from("/tmp"));
    assert!(!options.per_output);
    assert_eq!(options.output_identity(), None);
    assert_eq!(options.session_file_path(), path);
    assert_eq!(
        options.backup_file_path(),
        PathBuf::from("/tmp/lecture-04.wayscriber-session.bak")
    );
    assert_eq!(
        options.backup_recovery_marker_file_path(),
        PathBuf::from("/tmp/lecture-04.wayscriber-session.bak.recoverable")
    );
    assert_eq!(
        options.recovery_file_path(),
        PathBuf::from("/tmp/lecture-04.wayscriber-session.recovery")
    );
    assert_eq!(
        options.recovery_recoverable_marker_file_path(),
        PathBuf::from("/tmp/lecture-04.wayscriber-session.recovery.recoverable")
    );
    assert_eq!(
        options.clear_marker_file_path(),
        PathBuf::from("/tmp/lecture-04.wayscriber-session.cleared")
    );
    assert_eq!(
        options.lock_file_path(),
        PathBuf::from("/tmp/lecture-04.wayscriber-session.lock")
    );
}

#[test]
fn force_resume_persistence_enables_all_session_payloads() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "display");
    options.persist_transparent = false;
    options.persist_whiteboard = false;
    options.persist_blackboard = false;
    options.persist_history = false;
    options.restore_tool_state = false;

    options.force_resume_persistence();

    assert!(options.persist_transparent);
    assert!(options.persist_whiteboard);
    assert!(options.persist_blackboard);
    assert!(options.persist_history);
    assert!(options.restore_tool_state);
}

#[test]
fn named_file_foreground_validation_accepts_writable_parent_and_cleans_probe() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("lecture-04.wayscriber-session");

    validation::validate_named_session_file_for_foreground(&path)
        .expect("writable parent should pass foreground validation");

    let leftover_probe = fs::read_dir(temp.path())
        .expect("read temp dir")
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(".wayscriber-session-write-test-"))
        });
    assert!(
        leftover_probe.is_none(),
        "foreground validation should remove its writability probe"
    );
}

#[test]
fn named_file_offline_validation_allows_missing_parent() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp
        .path()
        .join("missing")
        .join("lecture-04.wayscriber-session");

    validation::validate_named_session_file_for_info(&path)
        .expect("info validation should allow missing parents");
    validation::validate_named_session_file_for_clear(&path)
        .expect("clear validation should allow missing parents");
}

#[test]
fn named_file_validation_rejects_missing_directory_shaped_path() {
    let temp = crate::test_temp::tempdir().unwrap();
    let raw_path = format!("{}/", temp.path().join("lecture").display());
    let path = PathBuf::from(raw_path);

    for (label, result) in [
        (
            "foreground",
            validation::validate_named_session_file_for_foreground(&path),
        ),
        (
            "info",
            validation::validate_named_session_file_for_info(&path),
        ),
        (
            "clear",
            validation::validate_named_session_file_for_clear(&path),
        ),
    ] {
        let err = result.expect_err(label);
        assert!(
            err.to_string().contains("must name a session file"),
            "{label}: {err:#}"
        );
    }
}

#[cfg(unix)]
#[test]
fn named_file_validation_rejects_fifo_target() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("session-fifo.wayscriber-session");
    make_fifo(&path);

    for (label, result) in [
        (
            "foreground",
            validation::validate_named_session_file_for_foreground(&path),
        ),
        (
            "info",
            validation::validate_named_session_file_for_info(&path),
        ),
        (
            "clear",
            validation::validate_named_session_file_for_clear(&path),
        ),
    ] {
        let err = result.expect_err(label);
        assert!(
            err.to_string().contains("regular session file"),
            "{label}: {err:#}"
        );
    }
}

#[cfg(unix)]
#[test]
fn named_file_validation_rejects_symlink_target() {
    let temp = crate::test_temp::tempdir().unwrap();
    let target = temp.path().join("real-session.wayscriber-session");
    let link = temp.path().join("linked-session.wayscriber-session");
    fs::write(&target, b"{}").expect("write symlink target");
    symlink(&target, &link).expect("create session symlink");

    for (label, result) in [
        (
            "foreground",
            validation::validate_named_session_file_for_foreground(&link),
        ),
        (
            "info",
            validation::validate_named_session_file_for_info(&link),
        ),
        (
            "clear",
            validation::validate_named_session_file_for_clear(&link),
        ),
    ] {
        let err = result.expect_err(label);
        assert!(
            err.to_string().contains("not a symlink"),
            "{label}: {err:#}"
        );
    }
}

#[cfg(unix)]
#[test]
fn named_file_validation_rejects_parent_with_write_bits_but_no_effective_access() {
    let parent = Path::new("/usr");
    let Ok(metadata) = fs::metadata(parent) else {
        return;
    };
    if !metadata.is_dir() || metadata.permissions().readonly() {
        return;
    }

    let manual_probe = parent.join(format!(
        ".wayscriber-session-manual-access-test-{}",
        std::process::id()
    ));
    if let Ok(file) = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manual_probe)
    {
        drop(file);
        let _ = fs::remove_file(&manual_probe);
        return;
    }

    let path = parent.join(format!(
        "wayscriber-session-validation-{}.wayscriber-session",
        std::process::id()
    ));
    let foreground_err = validation::validate_named_session_file_for_foreground(&path)
        .expect_err("foreground validation should reject process-unwritable parents");
    assert!(
        foreground_err.to_string().contains("not writable"),
        "{foreground_err:#}"
    );

    let clear_err = validation::validate_named_session_file_for_clear(&path)
        .expect_err("clear validation should reject process-unwritable parents");
    assert!(
        clear_err.to_string().contains("not writable for cleanup"),
        "{clear_err:#}"
    );
}

#[cfg(unix)]
fn make_fifo(path: &Path) {
    let raw_path = CString::new(path.as_os_str().as_bytes()).expect("fifo path has no NUL bytes");
    // SAFETY: raw_path is a valid, NUL-terminated filesystem path for this process.
    let result = unsafe { libc::mkfifo(raw_path.as_ptr(), 0o600) };
    assert_eq!(
        result,
        0,
        "mkfifo {} failed: {}",
        path.display(),
        std::io::Error::last_os_error()
    );
}
