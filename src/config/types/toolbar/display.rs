use serde::{Deserialize, Serialize};

/// Display state of the top toolbar strip.
///
/// `Full` and `Micro` are persisted forms of the strip; `Hidden` is the
/// runtime-only state the cycle action (`cycle_toolbar_display`, default
/// F2) reaches between `Micro` and `Full`. Hidden is never written back to
/// config — like the plain visibility toggle (F9), startup visibility is
/// governed by `top_pinned`.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TopDisplayMode {
    /// The regular pill-island strip.
    #[default]
    Full,
    /// One 44px round chip: active tool glyph inside a ring stroked in the
    /// current color (ring width follows stroke thickness).
    Micro,
    /// The strip is not shown (runtime-only; not persisted).
    Hidden,
}

impl TopDisplayMode {
    /// The form persisted to config: `Hidden` collapses to `Full` so a
    /// cycle-hidden strip comes back visible on the next start (visibility
    /// at startup stays `top_pinned`'s job).
    pub fn persisted(self) -> Self {
        match self {
            Self::Hidden => Self::Full,
            other => other,
        }
    }
}
