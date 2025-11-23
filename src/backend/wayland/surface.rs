//! Layer-surface management for the Wayland backend.
//!
//! This module owns the wl_surface/layer surface handle and the shm slot
//! pool. WaylandState asks SurfaceState for buffers and size information
//! instead of juggling the raw objects directly.

use anyhow::{Context, Result};
use log::info;
use smithay_client_toolkit::{
    shell::{WaylandSurface, wlr_layer::LayerSurface, xdg::window::Window},
    shm::{Shm, slot::SlotPool},
};
use wayland_client::protocol::{wl_output, wl_surface};

/// The active shell role for the surface.
pub enum SurfaceKind {
    Layer(LayerSurface),
    Xdg {
        #[allow(dead_code)]
        window: Window,
    },
}

/// Tracks the active layer surface, buffer pool, and associated sizing state.
pub struct SurfaceState {
    kind: Option<SurfaceKind>,
    wl_surface: Option<wl_surface::WlSurface>,
    pool: Option<SlotPool>,
    current_output: Option<wl_output::WlOutput>,
    width: u32,
    height: u32,
    scale: i32,
    configured: bool,
    frame_callback_pending: bool,
}

impl SurfaceState {
    /// Creates a new, unconfigured surface state.
    pub fn new() -> Self {
        Self {
            kind: None,
            wl_surface: None,
            pool: None,
            current_output: None,
            width: 0,
            height: 0,
            scale: 1,
            configured: false,
            frame_callback_pending: false,
        }
    }

    /// Assigns the layer surface produced during startup.
    pub fn set_layer_surface(&mut self, surface: LayerSurface) {
        self.wl_surface = Some(surface.wl_surface().clone());
        self.kind = Some(SurfaceKind::Layer(surface));
    }

    /// Assigns an xdg-shell window produced during startup.
    pub fn set_xdg_window(&mut self, window: Window) {
        self.wl_surface = Some(window.wl_surface().clone());
        self.kind = Some(SurfaceKind::Xdg { window });
    }

    /// Returns the active wl_surface, if initialized.
    pub fn wl_surface(&self) -> Option<&wl_surface::WlSurface> {
        self.wl_surface.as_ref()
    }

    /// Returns the mutable layer surface, if initialized.
    pub fn layer_surface_mut(&mut self) -> Option<&mut LayerSurface> {
        match &mut self.kind {
            Some(SurfaceKind::Layer(layer)) => Some(layer),
            _ => None,
        }
    }

    /// Returns true if the active surface is an xdg-shell window.
    pub fn is_xdg_window(&self) -> bool {
        matches!(self.kind, Some(SurfaceKind::Xdg { .. }))
    }

    /// Records the most recent output the surface entered.
    pub fn set_current_output(&mut self, output: wl_output::WlOutput) {
        self.current_output = Some(output);
    }

    /// Clears the current output if it matches the provided handle.
    pub fn clear_output(&mut self, output: &wl_output::WlOutput) {
        if self.current_output.as_ref() == Some(output) {
            self.current_output = None;
        }
    }

    /// Returns the last known output for this surface, if any.
    pub fn current_output(&self) -> Option<wl_output::WlOutput> {
        self.current_output.clone()
    }

    /// Updates the surface dimensions, returning `true` if the size changed.
    ///
    /// When the size changes, any existing buffer pool becomes invalid and is dropped.
    pub fn update_dimensions(&mut self, width: u32, height: u32) -> bool {
        let changed = self.width != width || self.height != height;
        self.width = width;
        self.height = height;
        if changed {
            self.pool = None;
        }
        changed
    }

    /// Updates the buffer scale (defaults to 1). Drops the pool when scale changes.
    pub fn set_scale(&mut self, scale: i32) {
        let scale = scale.max(1);
        if self.scale != scale {
            self.scale = scale;
            self.pool = None;
            if let Some(layer_surface) = self.layer_surface_mut() {
                let _ = layer_surface.set_buffer_scale(scale as u32);
            } else if let Some(wl_surface) = self.wl_surface() {
                wl_surface.set_buffer_scale(scale);
            }
        }
    }

    /// Returns current buffer scale.
    pub fn scale(&self) -> i32 {
        self.scale
    }

    /// Returns physical dimensions (logical * scale).
    pub fn physical_dimensions(&self) -> (u32, u32) {
        (
            self.width.saturating_mul(self.scale as u32),
            self.height.saturating_mul(self.scale as u32),
        )
    }

    /// Current surface width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Current surface height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Marks the surface as configured by the compositor.
    pub fn set_configured(&mut self, configured: bool) {
        self.configured = configured;
    }

    /// Returns whether the surface has completed its initial configure.
    pub fn is_configured(&self) -> bool {
        self.configured
    }

    /// Sets the frame callback pending flag.
    pub fn set_frame_callback_pending(&mut self, pending: bool) {
        self.frame_callback_pending = pending;
    }

    /// Returns whether a frame callback is currently outstanding.
    pub fn frame_callback_pending(&self) -> bool {
        self.frame_callback_pending
    }

    /// Ensures a shared memory pool of the appropriate size exists.
    pub fn ensure_pool(&mut self, shm: &Shm, buffer_count: usize) -> Result<&mut SlotPool> {
        if self.pool.is_none() {
            let (phys_w, phys_h) = self.physical_dimensions();
            let buffer_size = (phys_w * phys_h * 4) as usize;
            let pool_size = buffer_size * buffer_count;
            info!(
                "Creating new SlotPool ({}x{} @ scale {}, {} bytes, {} buffers)",
                phys_w, phys_h, self.scale, pool_size, buffer_count
            );
            let pool = SlotPool::new(pool_size, shm).context("Failed to create slot pool")?;
            self.pool = Some(pool);
        }

        self.pool
            .as_mut()
            .context("Buffer pool not initialized despite previous check")
    }
}
