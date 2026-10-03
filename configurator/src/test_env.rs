use std::sync::{Mutex, MutexGuard};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

pub(crate) fn lock() -> MutexGuard<'static, ()> {
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
