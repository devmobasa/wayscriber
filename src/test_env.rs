use std::sync::{Condvar, Mutex};

use crate::env_vars;
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

static ENV_LEASE: Lease = Lease::new();

pub(crate) fn lock() -> LeaseGuard<'static> {
    ENV_LEASE.acquire()
}

/// Exclusive access to a process-wide test resource, such as the environment.
///
/// A `MutexGuard` held across a test body is poisoned when that test panics,
/// and every later test would then either fail or reach inside the poisoned
/// lock. The lease's mutex only guards a flag in short sections that cannot
/// panic, and the guard hands the lease on while unwinding, so one failing
/// test does not fail the rest.
pub(crate) struct Lease {
    held: Mutex<bool>,
    released: Condvar,
}

/// Holds a [`Lease`] until dropped, including when the holder panics.
pub(crate) struct LeaseGuard<'lease> {
    lease: &'lease Lease,
}

impl Lease {
    pub(crate) const fn new() -> Self {
        Self {
            held: Mutex::new(false),
            released: Condvar::new(),
        }
    }

    pub(crate) fn acquire(&self) -> LeaseGuard<'_> {
        let mut held = self
            .held
            .lock()
            .expect("the lease flag is never locked across a panic");
        while *held {
            held = self
                .released
                .wait(held)
                .expect("the lease flag is never locked across a panic");
        }
        *held = true;

        LeaseGuard { lease: self }
    }
}

impl Drop for LeaseGuard<'_> {
    fn drop(&mut self) {
        // Dropping must not panic while a failed test unwinds, and the flag's
        // mutex is never poisoned, so a failed lock only skips the release.
        if let Ok(mut held) = self.lease.held.lock() {
            *held = false;
        }
        self.lease.released.notify_one();
    }
}

struct SavedEnv(Vec<(&'static str, Option<OsString>)>);

impl Drop for SavedEnv {
    fn drop(&mut self) {
        for (key, value) in self.0.drain(..) {
            // SAFETY: every caller creates this guard while holding the
            // environment lease, and it is dropped before that lease guard.
            unsafe {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn with_env_var<T>(
    key: &'static str,
    value: Option<&OsStr>,
    body: impl FnOnce() -> T,
) -> T {
    with_env_vars(&[(key, value)], body)
}

/// Sets several variables under one hold of the environment lock, restoring
/// all of them afterwards, including when `body` panics.
#[cfg(test)]
pub(crate) fn with_env_vars<T>(
    vars: &[(&'static str, Option<&OsStr>)],
    body: impl FnOnce() -> T,
) -> T {
    let _guard = lock();
    let _saved = SavedEnv(
        vars.iter()
            .map(|(key, _)| (*key, env::var_os(key)))
            .collect(),
    );

    for (key, value) in vars {
        // SAFETY: serialized by the environment lock held above.
        unsafe {
            match value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
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
    let saved: Vec<(&'static str, Option<OsString>)> = env_vars::DESKTOP_ENV_KEYS
        .iter()
        .map(|key| (*key, env::var_os(key)))
        .collect();
    for (key, _) in &saved {
        // SAFETY: serialized by the environment lock held above.
        unsafe { env::remove_var(key) };
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
    locale_dir: &Path,
    body: impl FnOnce(),
) {
    const CHILD_ENV: &str = "WAYSCRIBER_TEST_LANGUAGE_ISOLATED_CHILD";

    if env::var_os(CHILD_ENV).is_some() {
        body();
        return;
    }

    let test_name = test_path
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .expect("test path starts with the crate name");
    let mut command = Command::new(env::current_exe().expect("test binary"));
    command
        .args([test_name, "--exact", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .env(env_vars::LOCALE_DIR_ENV, locale_dir)
        .env_remove(env_vars::LANGUAGE_OVERRIDE_ENV);
    for key in env_vars::LOCALE_ENV_KEYS {
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
    use std::ffi::OsStr;
    use std::{env, thread};

    #[test]
    fn environment_is_restored_when_the_body_panics() {
        const KEY: &str = "WAYSCRIBER_TEST_ENV_PANIC_RESTORE";
        // SAFETY: this key is private to this test.
        unsafe { env::set_var(KEY, "before") };

        let result = thread::scope(|scope| {
            scope
                .spawn(|| {
                    super::with_env_var(KEY, Some(OsStr::new("during")), || {
                        panic!("exercise unwind restoration");
                    });
                })
                .join()
        });

        assert!(result.is_err());
        assert_eq!(env::var_os(KEY).as_deref(), Some(OsStr::new("before")));
        // SAFETY: this key is private to this test.
        unsafe { env::remove_var(KEY) };
    }
}
