pub fn version() -> &'static str {
    option_env!("WAYSCRIBER_RELEASE_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
}
