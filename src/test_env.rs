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
    with_env_vars(&[(key, value)], body)
}

/// Sets several variables under one hold of the environment lock, restoring
/// all of them afterwards, including when `body` panics.
#[cfg(test)]
pub(crate) fn with_env_vars<T>(
    vars: &[(&'static str, Option<&std::ffi::OsStr>)],
    body: impl FnOnce() -> T,
) -> T {
    let _guard = lock();
    let _saved = SavedEnv(
        vars.iter()
            .map(|(key, _)| (*key, std::env::var_os(key)))
            .collect(),
    );

    for (key, value) in vars {
        // SAFETY: serialized by the environment lock held above.
        unsafe {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
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

/// Runs `body` in a fresh copy of this test binary whose language environment
/// the test owns: no `WAYSCRIBER_LANGUAGE`, no locale variables, and an empty
/// catalog override directory, so the embedded catalogs are all it reads.
///
/// Catalogs load once per process and the override variable beats every
/// configured language, so a test that asserts embedded translations cannot
/// share a process with an environment it does not control. `test_path` is
/// `concat!(module_path!(), "::", <test name>)`.
pub(crate) fn in_language_isolated_child(test_path: &str, body: impl FnOnce()) {
    let empty = tempfile::tempdir().expect("empty locale override directory");
    in_language_isolated_child_with_overrides(test_path, empty.path(), body);
}

/// [`in_language_isolated_child`], with `locale_dir` as the child's catalog
/// override directory.
pub(crate) fn in_language_isolated_child_with_overrides(
    test_path: &str,
    locale_dir: &std::path::Path,
    body: impl FnOnce(),
) {
    const CHILD_ENV: &str = "WAYSCRIBER_TEST_LANGUAGE_ISOLATED_CHILD";

    if std::env::var_os(CHILD_ENV).is_some() {
        body();
        return;
    }

    let test_name = test_path
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .expect("test path starts with the crate name");
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    command
        .args([test_name, "--exact", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .env(crate::env_vars::LOCALE_DIR_ENV, locale_dir)
        .env_remove(crate::env_vars::LANGUAGE_OVERRIDE_ENV);
    for key in crate::env_vars::LOCALE_ENV_KEYS {
        command.env_remove(key);
    }

    let output = command.output().expect("run the language-isolated child");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "language-isolated child failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // A filter that matched nothing would pass without running the body.
    assert!(
        stdout.contains("1 passed;"),
        "language-isolated child ran no test:\n{stdout}"
    );
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
