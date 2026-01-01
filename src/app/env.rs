pub(crate) fn env_flag_enabled(name: &str) -> bool {
    if let Ok(val) = std::env::var(name) {
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
    use std::env;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn env_flag_enabled_accepts_truthy_values() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        for value in ["1", "true", "yes", "on", "TrUe"] {
            // SAFETY: serialized via ENV_MUTEX
            unsafe {
                env::set_var("WAYSCRIBER_TEST_FLAG", value);
            }
            assert!(
                env_flag_enabled("WAYSCRIBER_TEST_FLAG"),
                "expected '{value}' to be treated as truthy"
            );
        }

        unsafe {
            env::remove_var("WAYSCRIBER_TEST_FLAG");
        }
    }

    #[test]
    fn env_flag_enabled_rejects_non_truthy_values() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        for value in ["0", "false", "no", "off", "", "random"] {
            // SAFETY: serialized via ENV_MUTEX
            unsafe {
                env::set_var("WAYSCRIBER_TEST_FLAG", value);
            }
            assert!(
                !env_flag_enabled("WAYSCRIBER_TEST_FLAG"),
                "expected '{value}' to be treated as falsey"
            );
        }

        unsafe {
            env::remove_var("WAYSCRIBER_TEST_FLAG");
        }
    }
}
