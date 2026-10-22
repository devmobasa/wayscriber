use std::sync::{Condvar, Mutex};

use std::ffi::{OsStr, OsString};
use std::process::Command;
use std::sync::Once;
use std::{env, iter, thread};
use wayscriber::domain::Language;
use wayscriber::{env_vars, i18n};

use crate::test_temp;

static ENV_LEASE: Lease = Lease::new();

/// The shared environment lock, taken after the language variables are
/// cleared: clearing them takes the lock itself, so it must never wait for a
/// test that already holds it.
pub(crate) fn lock() -> LeaseGuard {
    sanitize_language_environment();
    acquire()
}

fn acquire() -> LeaseGuard {
    ENV_LEASE.acquire()
}

/// Exclusive access to the process environment for one test.
///
/// A `MutexGuard` held across a test body is poisoned when that test panics,
/// and every later test would then either fail or reach inside the poisoned
/// lock. The lease's mutex only guards a flag in short sections that cannot
/// panic, and the guard hands the lease on while unwinding, so one failing
/// test does not fail the rest.
struct Lease {
    held: Mutex<bool>,
    released: Condvar,
}

/// Holds the environment lease until dropped, including when the holder
/// panics.
pub(crate) struct LeaseGuard {
    lease: &'static Lease,
}

impl Lease {
    const fn new() -> Self {
        Self {
            held: Mutex::new(false),
            released: Condvar::new(),
        }
    }

    fn acquire(&'static self) -> LeaseGuard {
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

impl Drop for LeaseGuard {
    fn drop(&mut self) {
        // Dropping must not panic while a failed test unwinds, and the flag's
        // mutex is never poisoned, so a failed lock only skips the release.
        if let Ok(mut held) = self.lease.held.lock() {
            *held = false;
        }
        self.lease.released.notify_one();
    }
}

/// Holds the shared environment lock until every changed variable is restored.
pub(crate) struct EnvGuard {
    previous: Vec<(&'static str, Option<OsString>)>,
    _lock: LeaseGuard,
}

impl EnvGuard {
    pub(crate) fn set(values: &[(&'static str, &OsStr)]) -> Self {
        let lock = lock();
        let previous = values
            .iter()
            .map(|(key, _)| (*key, env::var_os(key)))
            .collect();
        for (key, value) in values {
            // SAFETY: mutations are serialized by the shared test lock.
            unsafe {
                env::set_var(key, value);
            }
        }
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            // SAFETY: the lock remains held until restoration completes.
            unsafe {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }
}

/// Clears, for the rest of this test process, every variable that picks the
/// UI language: the `WAYSCRIBER_LANGUAGE` override, the locale chain `Auto`
/// follows, and the catalog override directory.
///
/// Config loads, Defaults and the language row apply the draft's language,
/// and this crate links the core without its test-only locale neutrality, so
/// without this a loaded `auto` would follow whoever runs the suite and an
/// explicit `en` would lose to an inherited override. Empty values count as
/// unset. The catalog directory points at a path that holds no catalogs, so
/// the embedded catalogs are the only ones read whenever they first load.
/// Tests that exercise the variables themselves set them with [`EnvGuard`].
///
/// This only clears variables. A test that selects a language other than
/// English runs in its own process ([`in_language_child`]), so the process
/// language every other test reads stays English, in parallel runs too.
pub(crate) fn sanitize_language_environment() {
    static SANITIZED: Once = Once::new();

    SANITIZED.call_once(|| {
        let _lock = acquire();
        let no_catalogs = concat!(env!("CARGO_MANIFEST_DIR"), "/no-locale-overrides-in-tests");
        let mut values = vec![
            (env_vars::LANGUAGE_OVERRIDE_ENV, ""),
            (env_vars::LOCALE_DIR_ENV, no_catalogs),
        ];
        values.extend(env_vars::LOCALE_ENV_KEYS.iter().map(|key| (*key, "")));
        for (key, value) in values {
            // SAFETY: mutations are serialized by the shared test lock.
            unsafe {
                env::set_var(key, value);
            }
        }
    });
}

/// Runs `body` in a fresh copy of this test binary, alone, for a test that
/// selects a UI language other than English.
///
/// The language is process-global: selected here, it would reach every test
/// running beside this one, and the catalogs load once per process. The
/// child starts with no language override, no locale variables and an empty
/// catalog override directory, and changes nothing the parent can see.
/// `test_path` is `concat!(module_path!(), "::", <test name>)`.
pub(crate) fn in_language_child(test_path: &str, body: impl FnOnce()) {
    const CHILD_ENV: &str = "WAYSCRIBER_CONFIGURATOR_LANGUAGE_CHILD";

    if env::var_os(CHILD_ENV).is_some() {
        body();
        return;
    }

    let test_name = test_path
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .expect("test path starts with the crate name");
    let locale_dir = test_temp::tempdir().expect("empty locale override directory");
    let mut command = Command::new(env::current_exe().expect("test binary"));
    command
        .args([test_name, "--exact", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .env(env_vars::LOCALE_DIR_ENV, locale_dir.path())
        .env_remove(env_vars::LANGUAGE_OVERRIDE_ENV);
    for key in env_vars::LOCALE_ENV_KEYS {
        command.env_remove(key);
    }

    let output = command.output().expect("run the language child");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "language child failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // A filter that matched nothing would pass without running the body.
    assert!(
        stdout.contains("1 passed;"),
        "language child ran no test:\n{stdout}"
    );
}

#[test]
fn guard_restores_environment_after_a_panic() {
    const KEY: &str = "WAYSCRIBER_CONFIGURATOR_TEST_PANIC_RESTORE";
    let before = env::var_os(KEY);
    let result = thread::scope(|scope| {
        scope
            .spawn(|| {
                let _env = EnvGuard::set(&[(KEY, OsStr::new("during"))]);
                assert_eq!(env::var_os(KEY).as_deref(), Some(OsStr::new("during")));
                panic!("exercise fixture unwind");
            })
            .join()
    });
    assert!(result.is_err());
    let _lock = lock();
    assert_eq!(env::var_os(KEY), before);
}

#[test]
fn a_language_child_runs_alone_with_the_language_variables_cleared() {
    in_language_child(
        concat!(
            module_path!(),
            "::a_language_child_runs_alone_with_the_language_variables_cleared"
        ),
        || {
            for key in iter::once(env_vars::LANGUAGE_OVERRIDE_ENV).chain(env_vars::LOCALE_ENV_KEYS)
            {
                assert_eq!(env::var_os(key), None, "{key} reached the child");
            }
            i18n::set_language(Language::SimplifiedChinese);
            assert_eq!(i18n::tr("Boards"), "画板");
        },
    );
}
