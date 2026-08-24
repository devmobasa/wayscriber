/// The version this binary reports.
///
/// Packaging-only hotfixes build with `WAYSCRIBER_RELEASE_VERSION=X.Y.Z.N` while
/// the Cargo version stays `X.Y.Z` (see the hotfix policy in `tools/README.md`),
/// so `CARGO_PKG_VERSION` alone is wrong for those artifacts. A `const` rather
/// than only a function because the CLI spec needs it in a const position, and
/// two ideas of the version is exactly the drift this avoids.
pub const VERSION: &str = match option_env!("WAYSCRIBER_RELEASE_VERSION") {
    Some(release_version) => release_version,
    None => env!("CARGO_PKG_VERSION"),
};

pub fn version() -> &'static str {
    VERSION
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
