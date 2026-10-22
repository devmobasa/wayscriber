use std::env;

pub(crate) fn env_flag_enabled(name: &str) -> bool {
    if let Ok(val) = env::var(name) {
        matches!(
            val.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::env_flag_enabled;
    use crate::test_env;
    use std::env;

    const TEST_FLAG_ENV: &str = "WAYSCRIBER_TEST_FLAG";

    #[test]
    fn env_flag_enabled_accepts_truthy_values() {
        let _guard = test_env::lock();

        for value in ["1", "true", "yes", "on", "TrUe"] {
            // SAFETY: the environment lease held above serializes environment access.
            unsafe {
                env::set_var(TEST_FLAG_ENV, value);
            }
            assert!(
                env_flag_enabled(TEST_FLAG_ENV),
                "expected '{value}' to be treated as truthy"
            );
        }

        // SAFETY: serialized via ENV_MUTEX, still held by `_guard`, with the
        // only other test that writes this key.
        unsafe {
            env::remove_var(TEST_FLAG_ENV);
        }
    }

    #[test]
    fn env_flag_enabled_rejects_non_truthy_values() {
        let _guard = test_env::lock();

        for value in ["0", "false", "no", "off", "", "random"] {
            // SAFETY: the environment lease held above serializes environment access.
            unsafe {
                env::set_var(TEST_FLAG_ENV, value);
            }
            assert!(
                !env_flag_enabled(TEST_FLAG_ENV),
                "expected '{value}' to be treated as falsey"
            );
        }

        // SAFETY: serialized via ENV_MUTEX, still held by `_guard`, with the
        // only other test that writes this key.
        unsafe {
            env::remove_var(TEST_FLAG_ENV);
        }
    }
}
