use smithay_client_toolkit::shell::wlr_layer::Anchor;

use crate::backend::wayland::toolbar::surfaces::ToolbarSurface;
use crate::ui::toolbar::ToolbarSnapshot;

/// Tracks the lifetime and visibility of the top + side toolbar surfaces.
#[derive(Debug)]
pub struct ToolbarSurfaceManager {
    /// Combined visibility flag (true when any toolbar visible)
    pub(super) visible: bool,
    /// Whether the top toolbar is visible
    pub(super) top_visible: bool,
    /// Whether the side toolbar is visible
    pub(super) side_visible: bool,
    pub(super) suppressed: bool,
    pub(super) top: ToolbarSurface,
    pub(super) side: ToolbarSurface,
    pub(super) top_hover: Option<(f64, f64)>,
    pub(super) side_hover: Option<(f64, f64)>,
    pub(super) last_snapshot: Option<ToolbarSnapshot>,
}

impl Default for ToolbarSurfaceManager {
    fn default() -> Self {
        Self {
            visible: false,
            top_visible: false,
            side_visible: false,
            suppressed: false,
            // Anchor top/side toolbars to both axes they offset along so margins take effect.
            top: ToolbarSurface::new(
                "wayscriber-toolbar-top",
                Anchor::TOP | Anchor::LEFT,
                (12, 12, 0, 12),
            ),
            side: ToolbarSurface::new(
                "wayscriber-toolbar-side",
                Anchor::LEFT | Anchor::TOP,
                (24, 0, 24, 24),
            ),
            top_hover: None,
            side_hover: None,
            last_snapshot: None,
        }
    }
}

impl ToolbarSurfaceManager {
    pub fn new() -> Self {
        Self::default()
    }
}
