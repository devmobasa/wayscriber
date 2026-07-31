use std::sync::{Mutex, MutexGuard};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

pub(crate) fn lock() -> MutexGuard<'static, ()> {
    ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Runs `body` with the desktop-environment variables cleared, restoring them
/// afterwards, and holds the environment lock throughout.
///
/// Defaults that consult the desktop environment at call time (page
/// navigation) otherwise make a comparison against checked-in values depend on
/// the machine running the test rather than on what the code ships.
#[cfg(test)]
pub(crate) fn with_scrubbed_desktop_env<T>(body: impl FnOnce() -> T) -> T {
    let _guard = lock();
    let saved: Vec<(&str, Option<std::ffi::OsString>)> = crate::env_vars::DESKTOP_ENV_KEYS
        .iter()
        .map(|key| (*key, std::env::var_os(key)))
        .collect();
    for (key, _) in &saved {
        // SAFETY: serialized by the environment lock held above.
        unsafe { std::env::remove_var(key) };
    }
    let result = body();
    for (key, value) in saved {
        // SAFETY: serialized by the environment lock held above.
        unsafe {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
    result
}
