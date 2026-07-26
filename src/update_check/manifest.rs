//! The release manifest published at <https://wayscriber.com/latest.json>.
//!
//! The manifest is a static file rsynced with the website during a release, so
//! the check needs no API, no token, and no rate-limit handling. Every URL it
//! carries is eventually handed to `xdg-open`, so links are accepted only when
//! they point at wayscriber.com over HTTPS; anything else falls back to the
//! compiled-in defaults.

use serde::Deserialize;

/// Static manifest describing the newest published release.
pub const MANIFEST_URL: &str = "https://wayscriber.com/latest.json";

/// Update instructions, used when the manifest omits or misdeclares its own.
pub const DEFAULT_UPDATE_URL: &str = "https://wayscriber.com/docs/getting-started/updating.html";

/// Release notes, used under the same fallback rule.
pub const DEFAULT_NOTES_URL: &str = "https://wayscriber.com/docs/release-notes.html";

/// Hosts whose HTTPS URLs may be opened from manifest data.
const TRUSTED_HOSTS: [&str; 2] = ["wayscriber.com", "www.wayscriber.com"];

/// Largest manifest we will parse. The real file is well under 1 KiB.
pub(crate) const MAX_MANIFEST_BYTES: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
struct RawManifest {
    version: String,
    #[serde(default)]
    released: Option<String>,
    #[serde(default)]
    notes_url: Option<String>,
    #[serde(default)]
    update_url: Option<String>,
}

/// A validated manifest: the version parses, and both links are trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseManifest {
    pub version: String,
    pub released: Option<String>,
    pub notes_url: String,
    pub update_url: String,
}

/// Parse and validate manifest JSON.
pub(crate) fn parse_manifest(raw: &str) -> Result<ReleaseManifest, String> {
    if raw.len() > MAX_MANIFEST_BYTES {
        return Err("release manifest is implausibly large".to_string());
    }
    let parsed: RawManifest =
        serde_json::from_str(raw).map_err(|err| format!("invalid release manifest: {err}"))?;

    let version = parsed
        .version
        .trim()
        .trim_start_matches(['v', 'V'])
        .to_string();
    if super::version::Version::parse(&version).is_none() {
        return Err(format!("unrecognized version in manifest: {version:?}"));
    }

    Ok(ReleaseManifest {
        version,
        released: parsed
            .released
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        notes_url: trusted_url(parsed.notes_url.as_deref(), DEFAULT_NOTES_URL),
        update_url: trusted_url(parsed.update_url.as_deref(), DEFAULT_UPDATE_URL),
    })
}

/// Return `candidate` when it is an HTTPS wayscriber.com URL, else `fallback`.
fn trusted_url(candidate: Option<&str>, fallback: &str) -> String {
    candidate
        .map(str::trim)
        .filter(|url| is_trusted_url(url))
        .unwrap_or(fallback)
        .to_string()
}

/// HTTPS + exact host match. The host must be followed by a path, query,
/// fragment, or end of string so `wayscriber.com.example/` cannot pass.
pub(crate) fn is_trusted_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    TRUSTED_HOSTS.iter().any(|host| {
        rest.strip_prefix(host)
            .is_some_and(|tail| tail.is_empty() || tail.starts_with(['/', '?', '#']))
    })
}

/// Append the docs anchor for this build's install source, when one is known.
/// Package builds set `WAYSCRIBER_INSTALL_SOURCE` so the update page opens on
/// the section that actually applies (apt users should not read the Nix steps).
pub(crate) fn update_url_for_install_source(update_url: &str) -> String {
    match install_source_anchor() {
        Some(anchor) if !update_url.contains('#') => format!("{update_url}#{anchor}"),
        _ => update_url.to_string(),
    }
}

/// Build-time install source, normalized to a docs anchor.
pub(crate) fn install_source_anchor() -> Option<&'static str> {
    match install_source()? {
        "apt" | "deb" | "debian" | "ubuntu" => Some("ubuntu--debian"),
        "rpm" | "dnf" | "fedora" => Some("fedora--rhel"),
        "aur" | "arch" | "pacman" => Some("arch-linux-aur"),
        "nix" | "nixos" | "nixpkgs" => Some("nixos--nix"),
        "tarball" | "binary" => Some("tarball"),
        "source" | "cargo" => Some("build-from-source"),
        _ => None,
    }
}

/// Raw build-time install source label, if the packager set one.
pub fn install_source() -> Option<&'static str> {
    option_env!("WAYSCRIBER_INSTALL_SOURCE")
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_complete_manifest() {
        let raw = r#"{
            "version": "0.9.23",
            "released": "2026-07-20",
            "notes_url": "https://wayscriber.com/docs/release-notes.html",
            "update_url": "https://wayscriber.com/docs/getting-started/updating.html"
        }"#;

        let manifest = parse_manifest(raw).unwrap();
        assert_eq!(manifest.version, "0.9.23");
        assert_eq!(manifest.released.as_deref(), Some("2026-07-20"));
        assert_eq!(manifest.update_url, DEFAULT_UPDATE_URL);
    }

    #[test]
    fn fills_in_defaults_for_missing_links() {
        let manifest = parse_manifest(r#"{"version": "v1.0.0"}"#).unwrap();
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.released, None);
        assert_eq!(manifest.notes_url, DEFAULT_NOTES_URL);
        assert_eq!(manifest.update_url, DEFAULT_UPDATE_URL);
    }

    #[test]
    fn rejects_untrusted_links_without_failing_the_check() {
        let raw = r#"{
            "version": "1.0.0",
            "update_url": "https://evil.example/pwn",
            "notes_url": "http://wayscriber.com/docs"
        }"#;

        let manifest = parse_manifest(raw).unwrap();
        assert_eq!(manifest.update_url, DEFAULT_UPDATE_URL);
        assert_eq!(manifest.notes_url, DEFAULT_NOTES_URL);
    }

    #[test]
    fn rejects_malformed_manifests() {
        assert!(parse_manifest("not json").is_err());
        assert!(parse_manifest(r#"{"released": "2026-07-20"}"#).is_err());
        assert!(parse_manifest(r#"{"version": "tomorrow"}"#).is_err());
    }

    #[test]
    fn trusted_url_requires_https_and_an_exact_host() {
        assert!(is_trusted_url("https://wayscriber.com"));
        assert!(is_trusted_url("https://wayscriber.com/docs/x.html#apt"));
        assert!(is_trusted_url("https://www.wayscriber.com/docs"));

        assert!(!is_trusted_url("http://wayscriber.com/docs"));
        assert!(!is_trusted_url("https://wayscriber.com.example.org/docs"));
        assert!(!is_trusted_url("https://notwayscriber.com/docs"));
        assert!(!is_trusted_url("file:///etc/passwd"));
        assert!(!is_trusted_url(""));
    }

    #[test]
    fn anchor_is_only_appended_when_absent() {
        let already_anchored = "https://wayscriber.com/docs/updating.html#apt";
        assert_eq!(
            update_url_for_install_source(already_anchored),
            already_anchored
        );
    }
}
