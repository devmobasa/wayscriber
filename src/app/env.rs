pub(crate) fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|value| parse_env_flag(&value))
}

fn parse_env_flag(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::parse_env_flag;

    #[test]
    fn env_flag_enabled_accepts_truthy_values() {
        for value in ["1", "true", "yes", "on", "TrUe"] {
            assert!(
                parse_env_flag(value),
                "expected '{value}' to be treated as truthy"
            );
        }
    }

    #[test]
    fn env_flag_enabled_rejects_non_truthy_values() {
        for value in ["0", "false", "no", "off", "", "random"] {
            assert!(
                !parse_env_flag(value),
                "expected '{value}' to be treated as falsey"
            );
        }
    }
}
