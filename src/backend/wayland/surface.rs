//! Manages layer-surface state and shared memory buffers for the Wayland backend.

use anyhow::{Context, Result};
use log::info;
use smithay_client_toolkit::{
    shell::wlr_layer::LayerSurface,
    shm::{Shm, slot::SlotPool},
};

/// Tracks the active layer surface, buffer pool, and associated sizing state.
pub struct SurfaceState {
    layer_surface: Option<LayerSurface>,
    pool: Option<SlotPool>,
    width: u32,
    height: u32,
    configured: bool,
    frame_callback_pending: bool,
}

impl SurfaceState {
    /// Creates a new, unconfigured surface state.
    pub fn new() -> Self {
        Self {
            layer_surface: None,
            pool: None,
            width: 0,
            height: 0,
            configured: false,
            frame_callback_pending: false,
        }
    }

    /// Assigns the layer surface produced during startup.
    pub fn set_layer_surface(&mut self, surface: LayerSurface) {
        self.layer_surface = Some(surface);
    }

    /// Returns the current layer surface, if initialized.
    pub fn layer_surface(&self) -> Option<&LayerSurface> {
        self.layer_surface.as_ref()
    }

    /// Returns the mutable layer surface, if initialized.
    pub fn layer_surface_mut(&mut self) -> Option<&mut LayerSurface> {
        self.layer_surface.as_mut()
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
            let buffer_size = (self.width * self.height * 4) as usize;
            let pool_size = buffer_size * buffer_count;
            info!(
                "Creating new SlotPool ({}x{}, {} bytes, {} buffers)",
                self.width, self.height, pool_size, buffer_count
            );
            let pool = SlotPool::new(pool_size, shm).context("Failed to create slot pool")?;
            self.pool = Some(pool);
        }

        self.pool
            .as_mut()
            .context("Buffer pool not initialized despite previous check")
    }
}
