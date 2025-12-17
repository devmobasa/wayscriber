#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

use log::info;
use smithay_client_toolkit::{
    compositor::CompositorState,
    shell::wlr_layer::{Anchor, LayerShell, LayerSurfaceConfigure},
    shm::Shm,
};
use wayland_client::{
    QueueHandle,
    protocol::{wl_output, wl_surface},
};

use crate::backend::wayland::toolbar::surfaces::ToolbarSurface;
use crate::backend::wayland::toolbar_intent::ToolbarIntent;
use crate::ui::toolbar::ToolbarSnapshot;

use crate::backend::wayland::state::WaylandState;

/// Tracks the lifetime and visibility of the top + side toolbar surfaces.
#[derive(Debug)]
pub struct ToolbarSurfaceManager {
    /// Combined visibility flag (true when any toolbar visible)
    visible: bool,
    /// Whether the top toolbar is visible
    top_visible: bool,
    /// Whether the side toolbar is visible
    side_visible: bool,
    top: ToolbarSurface,
    side: ToolbarSurface,
    top_hover: Option<(f64, f64)>,
    side_hover: Option<(f64, f64)>,
    last_snapshot: Option<ToolbarSnapshot>,
}

impl Default for ToolbarSurfaceManager {
    fn default() -> Self {
        Self {
            visible: false,
            top_visible: false,
            side_visible: false,
            top: ToolbarSurface::new("wayscriber-toolbar-top", Anchor::TOP, (12, 12, 0, 12)),
            side: ToolbarSurface::new("wayscriber-toolbar-side", Anchor::LEFT, (24, 0, 24, 24)),
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

    pub fn top_created(&self) -> bool {
        self.top.layer_surface.is_some()
    }

    pub fn side_created(&self) -> bool {
        self.side.layer_surface.is_some()
    }

    /// Returns true if any toolbar is visible
    pub fn is_visible(&self) -> bool {
        self.visible || self.top_visible || self.side_visible
    }

    /// Returns true if the top toolbar is visible
    pub fn is_top_visible(&self) -> bool {
        self.top_visible
    }

    /// Returns true if the side toolbar is visible
    pub fn is_side_visible(&self) -> bool {
        self.side_visible
    }

    /// Set combined visibility (shows/hides both toolbars)
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if visible {
            self.top_visible = true;
            self.side_visible = true;
        } else {
            self.top_visible = false;
            self.side_visible = false;
            self.top.destroy();
            self.side.destroy();
            self.top_hover = None;
            self.side_hover = None;
        }
    }

    /// Set visibility of the top toolbar only
    pub fn set_top_visible(&mut self, visible: bool) {
        self.top_visible = visible;
        if !visible {
            self.top.destroy();
            self.top_hover = None;
        }
        // Update combined flag: any toolbar visible keeps overlays alive.
        self.visible = self.top_visible || self.side_visible;
    }

    /// Set visibility of the side toolbar only
    pub fn set_side_visible(&mut self, visible: bool) {
        self.side_visible = visible;
        if !visible {
            self.side.destroy();
            self.side_hover = None;
        }
        // Update combined flag: any toolbar visible keeps overlays alive.
        self.visible = self.top_visible || self.side_visible;
    }

    pub fn is_toolbar_surface(&self, surface: &wl_surface::WlSurface) -> bool {
        self.top.is_surface(surface) || self.side.is_surface(surface)
    }

    pub fn set_top_margin_left(&mut self, left: i32) {
        self.top.set_left_margin(left);
    }

    pub fn set_side_margin_top(&mut self, top: i32) {
        self.side.set_top_margin(top);
    }

    pub fn top_layer_surface(
        &self,
    ) -> Option<&smithay_client_toolkit::shell::wlr_layer::LayerSurface> {
        self.top.layer_surface.as_ref()
    }

    pub fn side_layer_surface(
        &self,
    ) -> Option<&smithay_client_toolkit::shell::wlr_layer::LayerSurface> {
        self.side.layer_surface.as_ref()
    }

    pub fn destroy_all(&mut self) {
        self.top.destroy();
        self.side.destroy();
        self.top_hover = None;
        self.side_hover = None;
    }

    pub fn is_toolbar_layer(
        &self,
        layer: &smithay_client_toolkit::shell::wlr_layer::LayerSurface,
    ) -> bool {
        self.top.is_layer(layer) || self.side.is_layer(layer)
    }

    pub fn configured_states(&self) -> (bool, bool) {
        (self.top.configured, self.side.configured)
    }

    pub fn ensure_created(
        &mut self,
        qh: &QueueHandle<WaylandState>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        scale: i32,
        snapshot: &ToolbarSnapshot,
    ) {
        let top_size = crate::backend::wayland::toolbar::top_size(snapshot);
        let side_size = crate::backend::wayland::toolbar::side_size(snapshot);

        if self.is_top_visible() {
            if self.top.layer_surface.is_none() {
                info!(
                    "Ensuring top toolbar surface exists at logical size {:?}, scale {}",
                    top_size, scale
                );
            } else if self.top.logical_size != (0, 0) && self.top.logical_size != top_size {
                info!(
                    "Top toolbar size change: {:?} -> {:?} (scale {})",
                    self.top.logical_size, top_size, scale
                );
            }
            if self.top.logical_size != (0, 0) && self.top.logical_size != top_size {
                self.top.destroy();
            }
            if self.top.logical_size == (0, 0) || self.top.logical_size != top_size {
                self.top.set_logical_size(top_size);
            }
            self.top.ensure_created(qh, compositor, layer_shell, scale);
        }

        if self.is_side_visible() {
            if self.side.layer_surface.is_none() {
                info!(
                    "Ensuring side toolbar surface exists at logical size {:?}, scale {}",
                    side_size, scale
                );
            } else if self.side.logical_size != (0, 0) && self.side.logical_size != side_size {
                info!(
                    "Side toolbar size change: {:?} -> {:?} (scale {})",
                    self.side.logical_size, side_size, scale
                );
            }
            if self.side.logical_size != (0, 0) && self.side.logical_size != side_size {
                self.side.destroy();
            }
            if self.side.logical_size == (0, 0) || self.side.logical_size != side_size {
                self.side.set_logical_size(side_size);
            }
            self.side.ensure_created(qh, compositor, layer_shell, scale);
        }
    }

    pub fn handle_configure(
        &mut self,
        configure: &LayerSurfaceConfigure,
        layer: &smithay_client_toolkit::shell::wlr_layer::LayerSurface,
    ) -> bool {
        if self.top.is_layer(layer) {
            return self.top.handle_configure(configure);
        }
        if self.side.is_layer(layer) {
            return self.side.handle_configure(configure);
        }
        false
    }

    pub fn render(&mut self, shm: &Shm, snapshot: &ToolbarSnapshot, hover: Option<(f64, f64)>) {
        // Render top toolbar if visible
        if self.is_top_visible() {
            let top_hover = hover.or(self.top_hover);
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
            let side_hover = hover.or(self.side_hover);
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

    pub fn maybe_update_scale(&mut self, output: Option<&wl_output::WlOutput>, scale: i32) {
        self.top.maybe_update_scale(output, scale);
        self.side.maybe_update_scale(output, scale);
    }

    pub fn mark_dirty(&mut self) {
        self.top.mark_dirty();
        self.side.mark_dirty();
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

    pub fn pointer_press(
        &mut self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> Option<(ToolbarIntent, bool)> {
        if self.top.is_surface(surface) {
            return self.top.hit_at(position.0, position.1);
        }
        if self.side.is_surface(surface) {
            return self.side.hit_at(position.0, position.1);
        }
        None
    }

    pub fn pointer_motion(
        &mut self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> Option<ToolbarIntent> {
        if self.top.is_surface(surface) {
            if self.top_hover != Some(position) {
                self.top_hover = Some(position);
                self.top.mark_dirty();
            }
            return self.top.drag_at(position.0, position.1);
        }
        if self.side.is_surface(surface) {
            if self.side_hover != Some(position) {
                self.side_hover = Some(position);
                self.side.mark_dirty();
            }
            return self.side.drag_at(position.0, position.1);
        }
        None
    }

    pub fn pointer_leave(&mut self, surface: &wl_surface::WlSurface) {
        if self.top.is_surface(surface) {
            self.top_hover = None;
            self.top.mark_dirty();
        } else if self.side.is_surface(surface) {
            self.side_hover = None;
            self.side.mark_dirty();
        }
    }
}
