use std::time::Instant;

use smithay_client_toolkit::shell::wlr_layer::Anchor;

use crate::backend::wayland::toolbar::events::ToolbarCursorHint;
use crate::backend::wayland::toolbar::surfaces::ToolbarSurface;
use crate::ui::toolbar::ToolbarSnapshot;

/// Tracks the lifetime and visibility of the top toolbar surface.
#[derive(Debug)]
pub struct ToolbarSurfaceManager {
    /// Whether the top toolbar is visible
    pub(super) top_visible: bool,
    pub(super) suppressed: bool,
    pub(super) top: ToolbarSurface,
    pub(super) top_hover: Option<(f64, f64)>,
    /// Timestamp when top hover started (for tooltip delay).
    pub(super) top_hover_start: Option<Instant>,
    pub(super) last_snapshot: Option<ToolbarSnapshot>,
}

impl Default for ToolbarSurfaceManager {
    fn default() -> Self {
        Self {
            top_visible: false,
            suppressed: false,
            // Anchor the top toolbar to both axes it offsets along so margins take effect.
            top: ToolbarSurface::new(
                "wayscriber-toolbar-top",
                Anchor::TOP | Anchor::LEFT,
                (12, 12, 0, 12),
            ),
            top_hover: None,
            top_hover_start: None,
            last_snapshot: None,
        }
    }
}

impl ToolbarSurfaceManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get cursor hint for the top toolbar when hovered.
    pub fn cursor_hint(&self) -> Option<ToolbarCursorHint> {
        if self.top_hover.is_some() {
            return self.top.cursor_hint();
        }
        None
    }
}
