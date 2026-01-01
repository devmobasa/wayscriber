use super::*;
use std::env;
use std::sync::Mutex;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
#[cfg(unix)]
fn tray_action_prefers_runtime_dir_when_set() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let prev = env::var_os("XDG_RUNTIME_DIR");
    // SAFETY: serialised via ENV_MUTEX
    unsafe {
        env::set_var("XDG_RUNTIME_DIR", tmp.path());
    }

    let path = tray_action_file();
    assert!(path.starts_with(tmp.path()));

    if let Some(prev) = prev {
        unsafe {
            env::set_var("XDG_RUNTIME_DIR", prev);
        }
    } else {
        unsafe {
            env::remove_var("XDG_RUNTIME_DIR");
        }
    }
}

#[test]
fn config_dir_prefers_xdg_config_home_when_set() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let tmp = tempfile::tempdir().unwrap();
    let prev_home = env::var_os("HOME");
    let prev_userprofile = env::var_os("USERPROFILE");
    let prev_xdg = env::var_os("XDG_CONFIG_HOME");

    unsafe {
        env::set_var("XDG_CONFIG_HOME", tmp.path());
        env::remove_var("HOME");
        env::remove_var("USERPROFILE");
    }

    let dir = config_dir().expect("config_dir should resolve from XDG_CONFIG_HOME");
    assert_eq!(dir, tmp.path());

    match prev_xdg {
        Some(v) => unsafe { env::set_var("XDG_CONFIG_HOME", v) },
        None => unsafe { env::remove_var("XDG_CONFIG_HOME") },
    }
    match prev_home {
        Some(v) => unsafe { env::set_var("HOME", v) },
        None => unsafe { env::remove_var("HOME") },
    }
    match prev_userprofile {
        Some(v) => unsafe { env::set_var("USERPROFILE", v) },
        None => unsafe { env::remove_var("USERPROFILE") },
    }
}

#[test]
fn config_dir_falls_back_to_home_config() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let tmp = tempfile::tempdir().unwrap();
    let prev_home = env::var_os("HOME");
    let prev_userprofile = env::var_os("USERPROFILE");
    let prev_xdg = env::var_os("XDG_CONFIG_HOME");

    unsafe {
        env::set_var("HOME", tmp.path());
        env::remove_var("USERPROFILE");
        env::remove_var("XDG_CONFIG_HOME");
    }

    let dir = config_dir().expect("config_dir should resolve from HOME");
    assert_eq!(dir, tmp.path().join(".config"));

    match prev_xdg {
        Some(v) => unsafe { env::set_var("XDG_CONFIG_HOME", v) },
        None => unsafe { env::remove_var("XDG_CONFIG_HOME") },
    }
    match prev_home {
        Some(v) => unsafe { env::set_var("HOME", v) },
        None => unsafe { env::remove_var("HOME") },
    }
    match prev_userprofile {
        Some(v) => unsafe { env::set_var("USERPROFILE", v) },
        None => unsafe { env::remove_var("USERPROFILE") },
    }
}

#[test]
fn data_dir_prefers_xdg_data_home_when_set() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let tmp = tempfile::tempdir().unwrap();
    let prev_home = env::var_os("HOME");
    let prev_userprofile = env::var_os("USERPROFILE");
    let prev_xdg = env::var_os("XDG_DATA_HOME");

    unsafe {
        env::set_var("XDG_DATA_HOME", tmp.path());
        env::remove_var("HOME");
        env::remove_var("USERPROFILE");
    }

    let dir = data_dir().expect("data_dir should resolve from XDG_DATA_HOME");
    assert_eq!(dir, tmp.path());

    match prev_xdg {
        Some(v) => unsafe { env::set_var("XDG_DATA_HOME", v) },
        None => unsafe { env::remove_var("XDG_DATA_HOME") },
    }
    match prev_home {
        Some(v) => unsafe { env::set_var("HOME", v) },
        None => unsafe { env::remove_var("HOME") },
    }
    match prev_userprofile {
        Some(v) => unsafe { env::set_var("USERPROFILE", v) },
        None => unsafe { env::remove_var("USERPROFILE") },
    }
}

#[test]
fn data_dir_falls_back_to_home_share() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let tmp = tempfile::tempdir().unwrap();
    let prev_home = env::var_os("HOME");
    let prev_userprofile = env::var_os("USERPROFILE");
    let prev_xdg = env::var_os("XDG_DATA_HOME");

    unsafe {
        env::set_var("HOME", tmp.path());
        env::remove_var("USERPROFILE");
        env::remove_var("XDG_DATA_HOME");
    }

    let dir = data_dir().expect("data_dir should resolve from HOME");
    assert_eq!(dir, tmp.path().join(".local").join("share"));

    match prev_xdg {
        Some(v) => unsafe { env::set_var("XDG_DATA_HOME", v) },
        None => unsafe { env::remove_var("XDG_DATA_HOME") },
    }
    match prev_home {
        Some(v) => unsafe { env::set_var("HOME", v) },
        None => unsafe { env::remove_var("HOME") },
    }
    match prev_userprofile {
        Some(v) => unsafe { env::set_var("USERPROFILE", v) },
        None => unsafe { env::remove_var("USERPROFILE") },
    }
}

#[test]
fn pictures_dir_prefers_xdg_when_set() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let tmp = tempfile::tempdir().unwrap();
    let prev_home = env::var_os("HOME");
    let prev_userprofile = env::var_os("USERPROFILE");
    let prev_xdg = env::var_os("XDG_PICTURES_DIR");

    unsafe {
        env::set_var("XDG_PICTURES_DIR", tmp.path());
        env::remove_var("HOME");
        env::remove_var("USERPROFILE");
    }

    let dir = pictures_dir().expect("pictures_dir should resolve from XDG_PICTURES_DIR");
    assert_eq!(dir, tmp.path());

    match prev_xdg {
        Some(v) => unsafe { env::set_var("XDG_PICTURES_DIR", v) },
        None => unsafe { env::remove_var("XDG_PICTURES_DIR") },
    }
    match prev_home {
        Some(v) => unsafe { env::set_var("HOME", v) },
        None => unsafe { env::remove_var("HOME") },
    }
    match prev_userprofile {
        Some(v) => unsafe { env::set_var("USERPROFILE", v) },
        None => unsafe { env::remove_var("USERPROFILE") },
    }
}

#[test]
fn pictures_dir_falls_back_to_home() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let tmp = tempfile::tempdir().unwrap();
    let prev_home = env::var_os("HOME");
    let prev_userprofile = env::var_os("USERPROFILE");
    let prev_xdg = env::var_os("XDG_PICTURES_DIR");

    unsafe {
        env::set_var("HOME", tmp.path());
        env::remove_var("USERPROFILE");
        env::remove_var("XDG_PICTURES_DIR");
    }

    let dir = pictures_dir().expect("pictures_dir should resolve from HOME");
    assert_eq!(dir, tmp.path().join("Pictures"));

    match prev_xdg {
        Some(v) => unsafe { env::set_var("XDG_PICTURES_DIR", v) },
        None => unsafe { env::remove_var("XDG_PICTURES_DIR") },
    }
    match prev_home {
        Some(v) => unsafe { env::set_var("HOME", v) },
        None => unsafe { env::remove_var("HOME") },
    }
    match prev_userprofile {
        Some(v) => unsafe { env::set_var("USERPROFILE", v) },
        None => unsafe { env::remove_var("USERPROFILE") },
    }
}

#[test]
fn expand_tilde_replaces_home() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let tmp = tempfile::tempdir().unwrap();
    let prev_home = env::var_os("HOME");

    unsafe {
        env::set_var("HOME", tmp.path());
    }

    let path = expand_tilde("~/test");
    assert_eq!(path, tmp.path().join("test"));

    match prev_home {
        Some(v) => unsafe { env::set_var("HOME", v) },
        None => unsafe { env::remove_var("HOME") },
    }
}
