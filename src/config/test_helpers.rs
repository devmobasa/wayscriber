use std::path::Path;

use crate::env_vars::XDG_CONFIG_HOME_ENV;
use crate::test_temp::TempDir;

pub(crate) fn with_temp_config_home<F, T>(f: F) -> T
where
    F: FnOnce(&Path) -> T,
{
    let _guard = crate::test_env::lock();
    let temp = TempDir::new().expect("tempdir");
    let original = std::env::var_os(XDG_CONFIG_HOME_ENV);
    // SAFETY: tests serialize process environment access and restore the previous value.
    unsafe {
        std::env::set_var(XDG_CONFIG_HOME_ENV, temp.path());
    }
    let result = f(temp.path());
    match original {
        Some(value) => unsafe { std::env::set_var(XDG_CONFIG_HOME_ENV, value) },
        None => unsafe { std::env::remove_var(XDG_CONFIG_HOME_ENV) },
    }
    result
}
