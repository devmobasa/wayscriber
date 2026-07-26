pub fn version() -> &'static str {
    option_env!("WAYSCRIBER_RELEASE_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// Short git hash this binary was built from, or `"unknown"` outside a
/// checkout (release tarballs, vendored distro builds).
pub fn commit_hash() -> &'static str {
    env!("WAYSCRIBER_GIT_HASH")
}

/// Commit date (`YYYY-MM-DD`) of that build, when it could be determined.
pub fn build_date() -> Option<&'static str> {
    let date = env!("WAYSCRIBER_BUILD_DATE");
    (!date.is_empty()).then_some(date)
}

/// How this copy was installed, as declared by the packager at build time
/// (`apt`, `rpm`, `aur`, `nix`, `tarball`, `source`, ...).
pub fn install_source() -> Option<&'static str> {
    crate::update_check::install_source()
}
