use super::*;
use crate::env_vars::{
    HOME_ENV, USERPROFILE_ENV, XDG_CONFIG_HOME_ENV, XDG_DATA_HOME_ENV, XDG_PICTURES_DIR_ENV,
    XDG_RUNTIME_DIR_ENV,
};
use std::env;

#[test]
#[cfg(unix)]
fn tray_action_prefers_runtime_dir_when_set() {
    let _guard = crate::test_env::lock();
    let tmp = crate::test_temp::tempdir().unwrap();
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
        unsafe {
            env::set_var(XDG_RUNTIME_DIR_ENV, prev);
        }
    } else {
        unsafe {
            env::remove_var(XDG_RUNTIME_DIR_ENV);
        }
    }
}

#[test]
fn config_dir_prefers_xdg_config_home_when_set() {
    let _guard = crate::test_env::lock();

    let tmp = crate::test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_CONFIG_HOME_ENV);

    unsafe {
        env::set_var(XDG_CONFIG_HOME_ENV, tmp.path());
        env::remove_var(HOME_ENV);
        env::remove_var(USERPROFILE_ENV);
    }

    let dir = config_dir()
        .unwrap_or_else(|| panic!("config_dir should resolve from {XDG_CONFIG_HOME_ENV}"));
    assert_eq!(dir, tmp.path());

    match prev_xdg {
        Some(v) => unsafe { env::set_var(XDG_CONFIG_HOME_ENV, v) },
        None => unsafe { env::remove_var(XDG_CONFIG_HOME_ENV) },
    }
    match prev_home {
        Some(v) => unsafe { env::set_var(HOME_ENV, v) },
        None => unsafe { env::remove_var(HOME_ENV) },
    }
    match prev_userprofile {
        Some(v) => unsafe { env::set_var(USERPROFILE_ENV, v) },
        None => unsafe { env::remove_var(USERPROFILE_ENV) },
    }
}

#[test]
fn config_dir_falls_back_to_home_config() {
    let _guard = crate::test_env::lock();

    let tmp = crate::test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_CONFIG_HOME_ENV);

    unsafe {
        env::set_var(HOME_ENV, tmp.path());
        env::remove_var(USERPROFILE_ENV);
        env::remove_var(XDG_CONFIG_HOME_ENV);
    }

    let dir = config_dir().expect("config_dir should resolve from HOME");
    assert_eq!(dir, tmp.path().join(".config"));

    match prev_xdg {
        Some(v) => unsafe { env::set_var(XDG_CONFIG_HOME_ENV, v) },
        None => unsafe { env::remove_var(XDG_CONFIG_HOME_ENV) },
    }
    match prev_home {
        Some(v) => unsafe { env::set_var(HOME_ENV, v) },
        None => unsafe { env::remove_var(HOME_ENV) },
    }
    match prev_userprofile {
        Some(v) => unsafe { env::set_var(USERPROFILE_ENV, v) },
        None => unsafe { env::remove_var(USERPROFILE_ENV) },
    }
}

#[test]
fn data_dir_prefers_xdg_data_home_when_set() {
    let _guard = crate::test_env::lock();

    let tmp = crate::test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_DATA_HOME_ENV);

    unsafe {
        env::set_var(XDG_DATA_HOME_ENV, tmp.path());
        env::remove_var(HOME_ENV);
        env::remove_var(USERPROFILE_ENV);
    }

    let dir =
        data_dir().unwrap_or_else(|| panic!("data_dir should resolve from {XDG_DATA_HOME_ENV}"));
    assert_eq!(dir, tmp.path());

    match prev_xdg {
        Some(v) => unsafe { env::set_var(XDG_DATA_HOME_ENV, v) },
        None => unsafe { env::remove_var(XDG_DATA_HOME_ENV) },
    }
    match prev_home {
        Some(v) => unsafe { env::set_var(HOME_ENV, v) },
        None => unsafe { env::remove_var(HOME_ENV) },
    }
    match prev_userprofile {
        Some(v) => unsafe { env::set_var(USERPROFILE_ENV, v) },
        None => unsafe { env::remove_var(USERPROFILE_ENV) },
    }
}

#[test]
fn data_dir_falls_back_to_home_share() {
    let _guard = crate::test_env::lock();

    let tmp = crate::test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_DATA_HOME_ENV);

    unsafe {
        env::set_var(HOME_ENV, tmp.path());
        env::remove_var(USERPROFILE_ENV);
        env::remove_var(XDG_DATA_HOME_ENV);
    }

    let dir = data_dir().expect("data_dir should resolve from HOME");
    assert_eq!(dir, tmp.path().join(".local").join("share"));

    match prev_xdg {
        Some(v) => unsafe { env::set_var(XDG_DATA_HOME_ENV, v) },
        None => unsafe { env::remove_var(XDG_DATA_HOME_ENV) },
    }
    match prev_home {
        Some(v) => unsafe { env::set_var(HOME_ENV, v) },
        None => unsafe { env::remove_var(HOME_ENV) },
    }
    match prev_userprofile {
        Some(v) => unsafe { env::set_var(USERPROFILE_ENV, v) },
        None => unsafe { env::remove_var(USERPROFILE_ENV) },
    }
}

#[test]
fn pictures_dir_prefers_xdg_when_set() {
    let _guard = crate::test_env::lock();

    let tmp = crate::test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_PICTURES_DIR_ENV);

    unsafe {
        env::set_var(XDG_PICTURES_DIR_ENV, tmp.path());
        env::remove_var(HOME_ENV);
        env::remove_var(USERPROFILE_ENV);
    }

    let dir = pictures_dir()
        .unwrap_or_else(|| panic!("pictures_dir should resolve from {XDG_PICTURES_DIR_ENV}"));
    assert_eq!(dir, tmp.path());

    match prev_xdg {
        Some(v) => unsafe { env::set_var(XDG_PICTURES_DIR_ENV, v) },
        None => unsafe { env::remove_var(XDG_PICTURES_DIR_ENV) },
    }
    match prev_home {
        Some(v) => unsafe { env::set_var(HOME_ENV, v) },
        None => unsafe { env::remove_var(HOME_ENV) },
    }
    match prev_userprofile {
        Some(v) => unsafe { env::set_var(USERPROFILE_ENV, v) },
        None => unsafe { env::remove_var(USERPROFILE_ENV) },
    }
}

#[test]
fn pictures_dir_falls_back_to_home() {
    let _guard = crate::test_env::lock();

    let tmp = crate::test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);
    let prev_userprofile = env::var_os(USERPROFILE_ENV);
    let prev_xdg = env::var_os(XDG_PICTURES_DIR_ENV);

    unsafe {
        env::set_var(HOME_ENV, tmp.path());
        env::remove_var(USERPROFILE_ENV);
        env::remove_var(XDG_PICTURES_DIR_ENV);
    }

    let dir = pictures_dir().expect("pictures_dir should resolve from HOME");
    assert_eq!(dir, tmp.path().join("Pictures"));

    match prev_xdg {
        Some(v) => unsafe { env::set_var(XDG_PICTURES_DIR_ENV, v) },
        None => unsafe { env::remove_var(XDG_PICTURES_DIR_ENV) },
    }
    match prev_home {
        Some(v) => unsafe { env::set_var(HOME_ENV, v) },
        None => unsafe { env::remove_var(HOME_ENV) },
    }
    match prev_userprofile {
        Some(v) => unsafe { env::set_var(USERPROFILE_ENV, v) },
        None => unsafe { env::remove_var(USERPROFILE_ENV) },
    }
}

#[test]
fn expand_tilde_replaces_home() {
    let _guard = crate::test_env::lock();

    let tmp = crate::test_temp::tempdir().unwrap();
    let prev_home = env::var_os(HOME_ENV);

    unsafe {
        env::set_var(HOME_ENV, tmp.path());
    }

    let path = expand_tilde("~/test");
    assert_eq!(path, tmp.path().join("test"));

    match prev_home {
        Some(v) => unsafe { env::set_var(HOME_ENV, v) },
        None => unsafe { env::remove_var(HOME_ENV) },
    }
}
