use std::path::Path;

use crate::test_temp::TempDir;

pub(crate) fn with_temp_config_home<F, T>(f: F) -> T
where
    F: FnOnce(&Path) -> T,
{
    let _guard = crate::test_env::lock();
    let temp = TempDir::new().expect("tempdir");
    let original = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: tests serialize process environment access and restore the previous value.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", temp.path());
    }
    let result = f(temp.path());
    match original {
        Some(value) => unsafe { std::env::set_var("XDG_CONFIG_HOME", value) },
        None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
    }
    result
}
