use serde::{Deserialize, Serialize};

/// Clipboard paste behavior.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardConfig {
    /// Browsers rasterize "Copy image" on an animated GIF into a static PNG
    /// snapshot, but also put the image's source URL on the clipboard. With
    /// this on, pasting such a copy downloads the `.gif` from that URL
    /// (HTTPS only, size-capped, via the system `curl`/`wget`) so the
    /// animation survives; any failure quietly falls back to the snapshot.
    /// Set to `false` to keep pasting fully offline.
    #[serde(default = "default_true")]
    pub fetch_gif_from_url: bool,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            fetch_gif_from_url: true,
        }
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_enable_the_gif_url_fetch() {
        assert!(ClipboardConfig::default().fetch_gif_from_url);
        let parsed: ClipboardConfig = toml::from_str("").expect("empty section parses");
        assert!(parsed.fetch_gif_from_url);
        let parsed: ClipboardConfig =
            toml::from_str("fetch_gif_from_url = false").expect("explicit value parses");
        assert!(!parsed.fetch_gif_from_url);
    }
}
