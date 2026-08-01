use std::sync::{Mutex, MutexGuard};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

pub(crate) fn lock() -> MutexGuard<'static, ()> {
    ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct SavedEnv(Vec<(&'static str, Option<std::ffi::OsString>)>);

impl Drop for SavedEnv {
    fn drop(&mut self) {
        for (key, value) in self.0.drain(..) {
            // SAFETY: every caller creates this guard while holding
            // `ENV_MUTEX`, and it is dropped before that mutex guard.
            unsafe {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn with_env_var<T>(
    key: &'static str,
    value: Option<&std::ffi::OsStr>,
    body: impl FnOnce() -> T,
) -> T {
    let _guard = lock();
    let _saved = SavedEnv(vec![(key, std::env::var_os(key))]);
    // SAFETY: serialized by the environment lock held above.
    unsafe {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
    body()
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
    let saved: Vec<(&'static str, Option<std::ffi::OsString>)> = crate::env_vars::DESKTOP_ENV_KEYS
        .iter()
        .map(|key| (*key, std::env::var_os(key)))
        .collect();
    for (key, _) in &saved {
        // SAFETY: serialized by the environment lock held above.
        unsafe { std::env::remove_var(key) };
    }
    let _saved = SavedEnv(saved);
    body()
}

#[cfg(test)]
mod tests {
    #[test]
    fn environment_is_restored_when_the_body_panics() {
        const KEY: &str = "WAYSCRIBER_TEST_ENV_PANIC_RESTORE";
        // SAFETY: this key is private to this test.
        unsafe { std::env::set_var(KEY, "before") };

        let result = std::panic::catch_unwind(|| {
            super::with_env_var(KEY, Some(std::ffi::OsStr::new("during")), || {
                panic!("exercise unwind restoration");
            });
        });

        assert!(result.is_err());
        assert_eq!(
            std::env::var_os(KEY).as_deref(),
            Some(std::ffi::OsStr::new("before"))
        );
        // SAFETY: this key is private to this test.
        unsafe { std::env::remove_var(KEY) };
    }
}
