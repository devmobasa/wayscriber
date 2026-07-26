use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::*;
use crate::env_vars::{
    HOME_ENV, USERPROFILE_ENV, XDG_CACHE_HOME_ENV, XDG_CONFIG_HOME_ENV, XDG_DATA_HOME_ENV,
    XDG_PICTURES_DIR_ENV, XDG_RUNTIME_DIR_ENV,
};

fn resolver(values: &[(&str, &OsStr)]) -> PathResolver {
    PathResolver::from_environment(PathEnvironment::for_test(values))
}

#[test]
fn absolute_home_wins_over_userprofile() {
    let paths = resolver(&[
        (HOME_ENV, OsStr::new("/home/primary")),
        (USERPROFILE_ENV, OsStr::new("/home/secondary")),
    ]);
    assert_eq!(
        paths.home_dir().expect("absolute HOME is a valid fixture"),
        Path::new("/home/primary")
    );
}

#[test]
fn empty_home_permits_absolute_userprofile() {
    let paths = resolver(&[
        (HOME_ENV, OsStr::new("")),
        (USERPROFILE_ENV, OsStr::new("/profiles/person")),
    ]);
    assert_eq!(
        paths
            .home_dir()
            .expect("empty HOME permits the absolute USERPROFILE fixture"),
        Path::new("/profiles/person")
    );
}

#[test]
fn relative_home_is_a_typed_error_without_userprofile_fallback() {
    let paths = resolver(&[
        (HOME_ENV, OsStr::new("relative-home")),
        (USERPROFILE_ENV, OsStr::new("/profiles/person")),
    ]);
    assert!(matches!(
        paths.home_dir(),
        Err(PathResolutionError::RelativeEnvironmentValue {
            variable: HOME_ENV,
            ..
        })
    ));
}

#[test]
fn missing_home_sources_do_not_fall_back_to_current_directory() {
    let paths = resolver(&[]);
    assert_eq!(
        paths.home_dir(),
        Err(PathResolutionError::Unavailable {
            capability: PathCapability::Home
        })
    );
}

#[test]
fn absolute_xdg_roots_bypass_home_selection() {
    let paths = resolver(&[
        (HOME_ENV, OsStr::new("relative-home")),
        (XDG_CONFIG_HOME_ENV, OsStr::new("/xdg/config")),
        (XDG_CACHE_HOME_ENV, OsStr::new("/xdg/cache")),
        (XDG_DATA_HOME_ENV, OsStr::new("/xdg/data")),
        (XDG_PICTURES_DIR_ENV, OsStr::new("/xdg/pictures")),
        (XDG_RUNTIME_DIR_ENV, OsStr::new("/run/user/1000")),
    ]);
    assert_eq!(
        paths.config_dir().expect("config fixture"),
        Path::new("/xdg/config")
    );
    assert_eq!(
        paths.cache_dir().expect("cache fixture"),
        Path::new("/xdg/cache")
    );
    assert_eq!(
        paths.data_dir().expect("data fixture"),
        Path::new("/xdg/data")
    );
    assert_eq!(
        paths.pictures_dir().expect("pictures fixture"),
        Path::new("/xdg/pictures")
    );
    assert_eq!(
        paths.runtime_dir().expect("runtime fixture"),
        Path::new("/run/user/1000/wayscriber")
    );
}

#[test]
fn relative_xdg_roots_fail_only_the_requested_capability() {
    let paths = resolver(&[
        (HOME_ENV, OsStr::new("/home/person")),
        (XDG_CONFIG_HOME_ENV, OsStr::new("relative-config")),
        (XDG_DATA_HOME_ENV, OsStr::new("/xdg/data")),
    ]);
    assert!(matches!(
        paths.config_dir(),
        Err(PathResolutionError::RelativeEnvironmentValue {
            variable: XDG_CONFIG_HOME_ENV,
            ..
        })
    ));
    assert_eq!(
        paths.data_dir().expect("independent data fixture"),
        Path::new("/xdg/data")
    );
}

#[test]
fn home_fallbacks_follow_the_approved_capability_layout() {
    let paths = resolver(&[(HOME_ENV, OsStr::new("/home/person"))]);
    assert_eq!(
        paths.config_dir().expect("config fallback"),
        Path::new("/home/person/.config")
    );
    assert_eq!(
        paths.cache_dir().expect("cache fallback"),
        Path::new("/home/person/.cache")
    );
    assert_eq!(
        paths.data_dir().expect("data fallback"),
        Path::new("/home/person/.local/share")
    );
    assert_eq!(
        paths.pictures_dir().expect("pictures fallback"),
        Path::new("/home/person/Pictures")
    );
    assert_eq!(
        paths.runtime_dir().expect("runtime fallback"),
        Path::new("/home/person/.local/share/wayscriber/runtime")
    );
}

#[test]
fn derived_files_remain_below_their_capability_roots() {
    let paths = resolver(&[
        (XDG_CONFIG_HOME_ENV, OsStr::new("/xdg/config")),
        (XDG_CACHE_HOME_ENV, OsStr::new("/xdg/cache")),
        (XDG_DATA_HOME_ENV, OsStr::new("/xdg/data")),
    ]);
    assert_eq!(
        paths.config_file().expect("config file fixture"),
        Path::new("/xdg/config/wayscriber/config.toml")
    );
    assert_eq!(
        paths.update_check_cache_file().expect("cache file fixture"),
        Path::new("/xdg/cache/wayscriber/update-check.json")
    );
    assert_eq!(
        paths.runtime_ui_state_file().expect("runtime UI fixture"),
        Path::new("/xdg/data/wayscriber/runtime-ui.toml")
    );
}

#[test]
fn tilde_expansion_uses_the_selected_absolute_home() {
    let paths = resolver(&[(HOME_ENV, OsStr::new("/home/person"))]);
    assert_eq!(
        paths
            .expand_tilde("~/sessions/talk")
            .expect("tilde fixture"),
        Path::new("/home/person/sessions/talk")
    );
    assert_eq!(
        paths
            .expand_tilde("plain/path")
            .expect("plain path fixture"),
        Path::new("plain/path")
    );
}

#[test]
fn absolute_user_path_policy_rejects_relative_values() {
    let paths = resolver(&[(HOME_ENV, OsStr::new("/home/person"))]);
    assert_eq!(
        paths
            .require_absolute_user_path("~/Pictures", PathCapability::Pictures)
            .expect("expanded fixture is absolute"),
        Path::new("/home/person/Pictures")
    );
    assert!(matches!(
        paths.require_absolute_user_path("captures", PathCapability::Pictures),
        Err(PathResolutionError::RelativeUserPath {
            capability: PathCapability::Pictures,
            ..
        })
    ));
}

#[test]
fn prepared_runtime_paths_secure_and_project_one_root() {
    let temp = crate::test_temp::tempdir().expect("runtime fixture root exists");
    let paths = resolver(&[(XDG_RUNTIME_DIR_ENV, temp.path().as_os_str())]);
    let prepared = PreparedRuntimePaths::prepare(&paths)
        .expect("runtime fixture can prepare a private directory");
    assert_eq!(prepared.root(), temp.path().join("wayscriber"));
    assert_eq!(
        prepared.daemon_pid_file(),
        prepared.root().join("wayscriber.pid")
    );
    assert_eq!(
        prepared.protocol_v2_root(),
        prepared.root().join("daemon-commands/v2")
    );
    let mode = std::fs::metadata(prepared.root())
        .expect("prepared runtime directory is inspectable")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
}

#[test]
fn prepared_runtime_paths_restrict_an_existing_owned_directory() {
    let temp = crate::test_temp::tempdir().expect("runtime fixture root exists");
    let root = temp.path().join("wayscriber");
    std::fs::create_dir(&root).expect("runtime fixture directory exists");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755))
        .expect("runtime fixture starts with public permissions");

    let paths = resolver(&[(XDG_RUNTIME_DIR_ENV, temp.path().as_os_str())]);
    let prepared = PreparedRuntimePaths::prepare(&paths)
        .expect("an owned runtime directory can be restricted safely");

    let mode = std::fs::metadata(prepared.root())
        .expect("prepared runtime directory is inspectable")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
}

#[test]
fn prepared_runtime_paths_reject_a_symlink_root() {
    let temp = crate::test_temp::tempdir().expect("runtime fixture root exists");
    let target = temp.path().join("target");
    std::fs::create_dir(&target).expect("symlink target fixture exists");
    std::os::unix::fs::symlink(&target, temp.path().join("wayscriber"))
        .expect("runtime symlink fixture exists");
    let paths = resolver(&[(XDG_RUNTIME_DIR_ENV, temp.path().as_os_str())]);
    assert!(matches!(
        PreparedRuntimePaths::prepare(&paths),
        Err(PrepareRuntimePathsError::Prepare(
            RuntimeDirectoryError::Symlink { .. }
        ))
    ));
}
