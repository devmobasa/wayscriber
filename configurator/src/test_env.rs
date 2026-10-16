use std::sync::{Mutex, MutexGuard};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// The shared environment lock, taken after the language variables are
/// cleared: clearing them takes the lock itself, so it must never wait for a
/// test that already holds it.
pub(crate) fn lock() -> MutexGuard<'static, ()> {
    sanitize_language_environment();
    acquire()
}

fn acquire() -> MutexGuard<'static, ()> {
    ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Holds the shared environment lock until every changed variable is restored.
pub(crate) struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvGuard {
    pub(crate) fn set(values: &[(&'static str, &std::ffi::OsStr)]) -> Self {
        let lock = lock();
        let previous = values
            .iter()
            .map(|(key, _)| (*key, std::env::var_os(key)))
            .collect();
        for (key, value) in values {
            // SAFETY: mutations are serialized by the shared test lock.
            unsafe {
                std::env::set_var(key, value);
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
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
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
    static SANITIZED: std::sync::Once = std::sync::Once::new();

    SANITIZED.call_once(|| {
        let _lock = acquire();
        let no_catalogs = concat!(env!("CARGO_MANIFEST_DIR"), "/no-locale-overrides-in-tests");
        let mut values = vec![
            (wayscriber::env_vars::LANGUAGE_OVERRIDE_ENV, ""),
            (wayscriber::env_vars::LOCALE_DIR_ENV, no_catalogs),
        ];
        values.extend(
            wayscriber::env_vars::LOCALE_ENV_KEYS
                .iter()
                .map(|key| (*key, "")),
        );
        for (key, value) in values {
            // SAFETY: mutations are serialized by the shared test lock.
            unsafe {
                std::env::set_var(key, value);
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

    if std::env::var_os(CHILD_ENV).is_some() {
        body();
        return;
    }

    let test_name = test_path
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .expect("test path starts with the crate name");
    let locale_dir = crate::test_temp::tempdir().expect("empty locale override directory");
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    command
        .args([test_name, "--exact", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .env(wayscriber::env_vars::LOCALE_DIR_ENV, locale_dir.path())
        .env_remove(wayscriber::env_vars::LANGUAGE_OVERRIDE_ENV);
    for key in wayscriber::env_vars::LOCALE_ENV_KEYS {
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
    let before = std::env::var_os(KEY);
    let result = std::panic::catch_unwind(|| {
        let _env = EnvGuard::set(&[(KEY, std::ffi::OsStr::new("during"))]);
        assert_eq!(
            std::env::var_os(KEY).as_deref(),
            Some(std::ffi::OsStr::new("during"))
        );
        panic!("exercise fixture unwind");
    });
    assert!(result.is_err());
    let _lock = lock();
    assert_eq!(std::env::var_os(KEY), before);
}

#[test]
fn a_language_child_runs_alone_with_the_language_variables_cleared() {
    in_language_child(
        concat!(
            module_path!(),
            "::a_language_child_runs_alone_with_the_language_variables_cleared"
        ),
        || {
            for key in std::iter::once(wayscriber::env_vars::LANGUAGE_OVERRIDE_ENV)
                .chain(wayscriber::env_vars::LOCALE_ENV_KEYS)
            {
                assert_eq!(std::env::var_os(key), None, "{key} reached the child");
            }
            wayscriber::i18n::set_language(wayscriber::domain::Language::SimplifiedChinese);
            assert_eq!(wayscriber::i18n::tr("Boards"), "画板");
        },
    );
}
