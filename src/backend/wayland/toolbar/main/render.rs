use smithay_client_toolkit::shm::Shm;

use super::structs::ToolbarSurfaceManager;
use crate::ui::toolbar::ToolbarSnapshot;

impl ToolbarSurfaceManager {
    pub fn render(&mut self, shm: &Shm, snapshot: &ToolbarSnapshot, hover: Option<(f64, f64)>) {
        // Render top toolbar if visible
        if self.is_top_visible() {
            let top_hover = hover.or(self.top_hover).or(self.top.focused_hover());
            if let Err(err) =
                self.top
                    .render(shm, snapshot, top_hover, |ctx, w, h, snap, hits, hov| {
                        crate::backend::wayland::toolbar::render_top_strip(
                            ctx, w, h, snap, hits, hov,
                        )
                    })
            {
                log::warn!("Failed to render top toolbar: {}", err);
            }
        }

        // Render side toolbar if visible
        if self.is_side_visible() {
            let side_hover = hover.or(self.side_hover).or(self.side.focused_hover());
            if let Err(err) =
                self.side
                    .render(shm, snapshot, side_hover, |ctx, w, h, snap, hits, hov| {
                        crate::backend::wayland::toolbar::render_side_palette(
                            ctx, w, h, snap, hits, hov,
                        )
                    })
            {
                log::warn!("Failed to render side toolbar: {}", err);
            }
        }
    }

    pub fn mark_dirty(&mut self) {
        self.top.mark_dirty();
        self.side.mark_dirty();
    }

    pub fn needs_render(&self) -> bool {
        (self.top_visible && self.top.needs_render())
            || (self.side_visible && self.side.needs_render())
    }

    /// Store the latest snapshot and report whether it differs from the previous one.
    pub fn update_snapshot(&mut self, snapshot: &ToolbarSnapshot) -> bool {
        let changed = self
            .last_snapshot
            .as_ref()
            .map(|prev| prev != snapshot)
            .unwrap_or(true);
        self.last_snapshot = Some(snapshot.clone());
        changed
    }

    pub fn last_snapshot(&self) -> Option<&ToolbarSnapshot> {
        self.last_snapshot.as_ref()
    }
}
