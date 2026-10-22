use crate::{test_env, test_temp};

use super::*;
use crate::env_vars::{
    HOME_ENV, USERPROFILE_ENV, XDG_CACHE_HOME_ENV, XDG_CONFIG_HOME_ENV, XDG_DATA_HOME_ENV,
    XDG_PICTURES_DIR_ENV, XDG_RUNTIME_DIR_ENV,
};
use std::env;

#[test]
#[cfg(unix)]
fn tray_action_prefers_runtime_dir_when_set() {
    let _guard = test_env::lock();
    let tmp = test_temp::tempdir().unwrap();
    let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
    // SAFETY: serialised via ENV_MUTEX
    unsafe {
        env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
    }

    let path = tray_action_file();
    assert!(path.starts_with(tmp.path()));
    assert!(tray_action_dir().starts_with(tmp.path()));
    assert!(daemon_command_file().starts_with(tmp.path()));
    assert!(daemon_command_dir().starts_with(tmp.path()));
    assert!(daemon_pid_file().starts_with(tmp.path()));

    if let Some(prev) = prev {
        // SAFETY: this test still holds the environment mutex in `_guard`.
        unsafe {
            env::set_var(XDG_RUNTIME_DIR_ENV, prev);
        }
    } else {
        // SAFETY: this test still holds the environment mutex in `_guard`.
        unsafe {
            env::remove_var(XDG_RUNTIME_DIR_ENV);
        }
    }
}

#[test]
fn config_dir_prefers_xdg_config_home_when_set() {
    let _guard = test_env::lock();

    let tmp = test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_CONFIG_HOME_ENV);

    // SAFETY: serialized by the test environment mutex held in `_guard`.
    unsafe {
        env::set_var(XDG_CONFIG_HOME_ENV, tmp.path());
        env::remove_var(HOME_ENV);
        env::remove_var(USERPROFILE_ENV);
    }

    let dir = config_dir()
        .unwrap_or_else(|| panic!("config_dir should resolve from {XDG_CONFIG_HOME_ENV}"));
    assert_eq!(dir, tmp.path());

    // SAFETY: this test still holds the environment mutex in `_guard`.
    unsafe {
        match prev_xdg {
            Some(v) => env::set_var(XDG_CONFIG_HOME_ENV, v),
            None => env::remove_var(XDG_CONFIG_HOME_ENV),
        }
        match prev_home {
            Some(v) => env::set_var(HOME_ENV, v),
            None => env::remove_var(HOME_ENV),
        }
        match prev_userprofile {
            Some(v) => env::set_var(USERPROFILE_ENV, v),
            None => env::remove_var(USERPROFILE_ENV),
        }
    }
}

#[test]
fn update_check_cache_follows_the_cache_directory() {
    let _guard = test_env::lock();

    let tmp = test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_CACHE_HOME_ENV);

    // SAFETY: serialized by the test environment mutex held in `_guard`.
    unsafe {
        env::set_var(XDG_CACHE_HOME_ENV, tmp.path());
        env::remove_var(HOME_ENV);
        env::remove_var(USERPROFILE_ENV);
    }

    assert_eq!(cache_dir().as_deref(), Some(tmp.path()));
    assert_eq!(
        update_check_cache_file(),
        tmp.path().join("wayscriber").join("update-check.json")
    );

    // Falling back to HOME keeps the result inside ~/.cache, never in data or
    // config directories.
    // SAFETY: serialized by the test environment mutex held in `_guard`.
    unsafe {
        env::remove_var(XDG_CACHE_HOME_ENV);
        env::set_var(HOME_ENV, tmp.path());
    }
    assert_eq!(
        update_check_cache_file(),
        tmp.path()
            .join(".cache")
            .join("wayscriber")
            .join("update-check.json")
    );

    // SAFETY: this test still holds the environment mutex in `_guard`.
    unsafe {
        match prev_xdg {
            Some(v) => env::set_var(XDG_CACHE_HOME_ENV, v),
            None => env::remove_var(XDG_CACHE_HOME_ENV),
        }
        match prev_home {
            Some(v) => env::set_var(HOME_ENV, v),
            None => env::remove_var(HOME_ENV),
        }
        match prev_userprofile {
            Some(v) => env::set_var(USERPROFILE_ENV, v),
            None => env::remove_var(USERPROFILE_ENV),
        }
    }
}

#[test]
fn config_dir_falls_back_to_home_config() {
    let _guard = test_env::lock();

    let tmp = test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_CONFIG_HOME_ENV);

    // SAFETY: serialized by the test environment mutex held in `_guard`.
    unsafe {
        env::set_var(HOME_ENV, tmp.path());
        env::remove_var(USERPROFILE_ENV);
        env::remove_var(XDG_CONFIG_HOME_ENV);
    }

    let dir = config_dir().expect("config_dir should resolve from HOME");
    assert_eq!(dir, tmp.path().join(".config"));

    // SAFETY: this test still holds the environment mutex in `_guard`.
    unsafe {
        match prev_xdg {
            Some(v) => env::set_var(XDG_CONFIG_HOME_ENV, v),
            None => env::remove_var(XDG_CONFIG_HOME_ENV),
        }
        match prev_home {
            Some(v) => env::set_var(HOME_ENV, v),
            None => env::remove_var(HOME_ENV),
        }
        match prev_userprofile {
            Some(v) => env::set_var(USERPROFILE_ENV, v),
            None => env::remove_var(USERPROFILE_ENV),
        }
    }
}

#[test]
fn data_dir_prefers_xdg_data_home_when_set() {
    let _guard = test_env::lock();

    let tmp = test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_DATA_HOME_ENV);

    // SAFETY: serialized by the test environment mutex held in `_guard`.
    unsafe {
        env::set_var(XDG_DATA_HOME_ENV, tmp.path());
        env::remove_var(HOME_ENV);
        env::remove_var(USERPROFILE_ENV);
    }

    let dir =
        data_dir().unwrap_or_else(|| panic!("data_dir should resolve from {XDG_DATA_HOME_ENV}"));
    assert_eq!(dir, tmp.path());
    assert_eq!(
        runtime_ui_state_file(),
        tmp.path().join("wayscriber/runtime-ui.toml")
    );

    // SAFETY: this test still holds the environment mutex in `_guard`.
    unsafe {
        match prev_xdg {
            Some(v) => env::set_var(XDG_DATA_HOME_ENV, v),
            None => env::remove_var(XDG_DATA_HOME_ENV),
        }
        match prev_home {
            Some(v) => env::set_var(HOME_ENV, v),
            None => env::remove_var(HOME_ENV),
        }
        match prev_userprofile {
            Some(v) => env::set_var(USERPROFILE_ENV, v),
            None => env::remove_var(USERPROFILE_ENV),
        }
    }
}

#[test]
fn data_dir_falls_back_to_home_share() {
    let _guard = test_env::lock();

    let tmp = test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_DATA_HOME_ENV);

    // SAFETY: serialized by the test environment mutex held in `_guard`.
    unsafe {
        env::set_var(HOME_ENV, tmp.path());
        env::remove_var(USERPROFILE_ENV);
        env::remove_var(XDG_DATA_HOME_ENV);
    }

    let dir = data_dir().expect("data_dir should resolve from HOME");
    assert_eq!(dir, tmp.path().join(".local").join("share"));

    // SAFETY: this test still holds the environment mutex in `_guard`.
    unsafe {
        match prev_xdg {
            Some(v) => env::set_var(XDG_DATA_HOME_ENV, v),
            None => env::remove_var(XDG_DATA_HOME_ENV),
        }
        match prev_home {
            Some(v) => env::set_var(HOME_ENV, v),
            None => env::remove_var(HOME_ENV),
        }
        match prev_userprofile {
            Some(v) => env::set_var(USERPROFILE_ENV, v),
            None => env::remove_var(USERPROFILE_ENV),
        }
    }
}

#[test]
fn searched_data_dirs_follow_the_base_directory_spec() {
    let home = || Some(PathBuf::from("/home/user"));
    let default_home = PathBuf::from("/home/user/.local/share");
    let system_defaults = [
        PathBuf::from("/usr/local/share"),
        PathBuf::from("/usr/share"),
    ];
    let with_defaults = |data_home: PathBuf| {
        let mut dirs = vec![data_home];
        dirs.extend(system_defaults.clone());
        dirs
    };

    assert_eq!(
        searched_data_dirs_from(None, home(), None),
        with_defaults(default_home.clone())
    );
    assert_eq!(
        searched_data_dirs_from(Some(OsString::new()), home(), Some(OsString::new())),
        with_defaults(default_home.clone())
    );
    // A relative data home would resolve against the current directory.
    assert_eq!(
        searched_data_dirs_from(Some("share".into()), home(), None),
        with_defaults(default_home)
    );
    assert_eq!(
        searched_data_dirs_from(Some("/data".into()), home(), None),
        with_defaults(PathBuf::from("/data"))
    );
    assert_eq!(
        searched_data_dirs_from(
            Some("/data".into()),
            home(),
            Some("/opt/share::relative/share:/usr/share/".into())
        ),
        vec![
            PathBuf::from("/data"),
            PathBuf::from("/opt/share"),
            PathBuf::from("/usr/share/")
        ]
    );
}

#[test]
fn pictures_dir_prefers_xdg_when_set() {
    let _guard = test_env::lock();

    let tmp = test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_PICTURES_DIR_ENV);

    // SAFETY: serialized by the test environment mutex held in `_guard`.
    unsafe {
        env::set_var(XDG_PICTURES_DIR_ENV, tmp.path());
        env::remove_var(HOME_ENV);
        env::remove_var(USERPROFILE_ENV);
    }

    let dir = pictures_dir()
        .unwrap_or_else(|| panic!("pictures_dir should resolve from {XDG_PICTURES_DIR_ENV}"));
    assert_eq!(dir, tmp.path());

    // SAFETY: this test still holds the environment mutex in `_guard`.
    unsafe {
        match prev_xdg {
            Some(v) => env::set_var(XDG_PICTURES_DIR_ENV, v),
            None => env::remove_var(XDG_PICTURES_DIR_ENV),
        }
        match prev_home {
            Some(v) => env::set_var(HOME_ENV, v),
            None => env::remove_var(HOME_ENV),
        }
        match prev_userprofile {
            Some(v) => env::set_var(USERPROFILE_ENV, v),
            None => env::remove_var(USERPROFILE_ENV),
        }
    }
}

#[test]
fn pictures_dir_falls_back_to_home() {
    let _guard = test_env::lock();

    let tmp = test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_PICTURES_DIR_ENV);

    // SAFETY: serialized by the test environment mutex held in `_guard`.
    unsafe {
        env::set_var(HOME_ENV, tmp.path());
        env::remove_var(USERPROFILE_ENV);
        env::remove_var(XDG_PICTURES_DIR_ENV);
    }

    let dir = pictures_dir().expect("pictures_dir should resolve from HOME");
    assert_eq!(dir, tmp.path().join("Pictures"));

    // SAFETY: this test still holds the environment mutex in `_guard`.
    unsafe {
        match prev_xdg {
            Some(v) => env::set_var(XDG_PICTURES_DIR_ENV, v),
            None => env::remove_var(XDG_PICTURES_DIR_ENV),
        }
        match prev_home {
            Some(v) => env::set_var(HOME_ENV, v),
            None => env::remove_var(HOME_ENV),
        }
        match prev_userprofile {
            Some(v) => env::set_var(USERPROFILE_ENV, v),
            None => env::remove_var(USERPROFILE_ENV),
        }
    }
}

#[test]
fn expand_tilde_replaces_home() {
    let _guard = test_env::lock();

    let tmp = test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);

    // SAFETY: serialized by the test environment mutex held in `_guard`.
    unsafe {
        env::set_var(HOME_ENV, tmp.path());
    }

    let path = expand_tilde("~/test");
    assert_eq!(path, tmp.path().join("test"));

    // SAFETY: this test still holds the environment mutex in `_guard`.
    unsafe {
        match prev_home {
            Some(v) => env::set_var(HOME_ENV, v),
            None => env::remove_var(HOME_ENV),
        }
    }
}

#[test]
fn is_single_path_component_rejects_path_escapes() {
    assert!(is_single_path_component("screenshot_%Y-%m-%d_%H%M%S"));
    assert!(is_single_path_component("shot.png"));
    assert!(!is_single_path_component(""));
    assert!(!is_single_path_component("."));
    assert!(!is_single_path_component(".."));
    assert!(!is_single_path_component("../evil"));
    assert!(!is_single_path_component("foo/bar"));
    assert!(!is_single_path_component("/abs"));
    assert!(!is_single_path_component("foo\\bar"));
    assert!(!is_single_path_component("foo\0bar"));
}
