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

/// Where the side-palette functions live.
///
/// `Pill` (the default) is the supported layout: the standalone side palette is
/// fully retired and every pane has a concrete new home in the top toolbar. The
/// Draw pane's drawing properties live in the top strip's contextual style pill
/// (colors included); canvas management lives in the "Canvas…" overflow popover,
/// the bottom-right zoom chip, and the status-bar board picker (boards & pages);
/// presets live in the top-strip presets island; and the Session/Settings panes
/// live in popovers opened from the top strip's overflow menu. `Panel` is the
/// deprecated legacy escape hatch that restores the classic four-pane side
/// palette (`side_active_pane`, `collapsed_sections`, `side_pinned`,
/// `side_minimized` all apply); it is deprecated and planned for removal one
/// release after the pill default lands. Panel-mode users see a once-per-session
/// notice pointing at these new homes.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ToolbarSideLayout {
    /// No side palette surface; its functions live in the style pill, the
    /// "Canvas…" overflow popover, the bottom-right zoom chip, the status-bar
    /// board picker, the top-strip presets island, and the Session/Settings
    /// overflow popovers (default; the supported layout).
    #[default]
    Pill,
    /// The classic side palette (legacy escape hatch, deprecated; removal
    /// planned one release after the pill default).
    Panel,
}
