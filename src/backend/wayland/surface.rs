//! Layer-surface management for the Wayland backend.
//!
//! This module owns the wl_surface/layer surface handle and the shm slot
//! pool. WaylandState asks SurfaceState for buffers and size information
//! instead of juggling the raw objects directly.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use log::info;
use smithay_client_toolkit::{
    shell::{WaylandSurface, wlr_layer::LayerSurface, xdg::window::Window},
    shm::{
        Shm,
        slot::{Buffer, Slot, SlotPool},
    },
};
use wayland_client::{
    Proxy,
    protocol::{wl_output, wl_shm, wl_surface},
};

const XDG_FROZEN_FULLSCREEN_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum XdgFrozenFullscreenState {
    #[default]
    Inactive,
    PendingConfigure,
    Active,
}

#[derive(Debug, Default)]
pub(in crate::backend::wayland) struct XdgFrozenFullscreen {
    state: XdgFrozenFullscreenState,
    requested_at: Option<Instant>,
}

impl XdgFrozenFullscreen {
    pub(in crate::backend::wayland) fn request(&mut self, now: Instant) {
        self.state = XdgFrozenFullscreenState::PendingConfigure;
        self.requested_at = Some(now);
    }

    pub(in crate::backend::wayland) fn activate(&mut self) {
        self.state = XdgFrozenFullscreenState::Active;
        self.requested_at = None;
    }

    pub(in crate::backend::wayland) fn finish(&mut self) {
        self.state = XdgFrozenFullscreenState::Inactive;
        self.requested_at = None;
    }

    pub(in crate::backend::wayland) fn timeout(&self, now: Instant) -> Option<Duration> {
        if !self.pending_configure() {
            return None;
        }
        Some(
            self.requested_at
                .and_then(|requested_at| requested_at.checked_add(XDG_FROZEN_FULLSCREEN_TIMEOUT))
                .map(|deadline| deadline.saturating_duration_since(now))
                .unwrap_or(Duration::ZERO),
        )
    }

    pub(in crate::backend::wayland) fn pending_configure(&self) -> bool {
        self.state == XdgFrozenFullscreenState::PendingConfigure
    }

    pub(in crate::backend::wayland) fn requested(&self) -> bool {
        self.state != XdgFrozenFullscreenState::Inactive
    }
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct SurfacePlacement {
    preferred_output_identity: Option<String>,
    xdg_fullscreen: bool,
    main_surface_uses_overlay_layer: bool,
    xdg_frozen: XdgFrozenFullscreen,
}

impl SurfacePlacement {
    pub(in crate::backend::wayland) fn new(
        preferred_output_identity: Option<String>,
        xdg_fullscreen: bool,
        main_surface_uses_overlay_layer: bool,
    ) -> Self {
        Self {
            preferred_output_identity,
            xdg_fullscreen,
            main_surface_uses_overlay_layer,
            xdg_frozen: XdgFrozenFullscreen::default(),
        }
    }

    pub(in crate::backend::wayland) fn preferred_output_identity(&self) -> Option<&str> {
        self.preferred_output_identity.as_deref()
    }

    pub(in crate::backend::wayland) fn xdg_fullscreen(&self) -> bool {
        self.xdg_fullscreen
    }

    pub(in crate::backend::wayland) fn layer(
        &self,
    ) -> smithay_client_toolkit::shell::wlr_layer::Layer {
        if self.main_surface_uses_overlay_layer {
            smithay_client_toolkit::shell::wlr_layer::Layer::Overlay
        } else {
            smithay_client_toolkit::shell::wlr_layer::Layer::Top
        }
    }

    pub(in crate::backend::wayland) fn xdg_frozen(&self) -> &XdgFrozenFullscreen {
        &self.xdg_frozen
    }

    pub(in crate::backend::wayland) fn xdg_frozen_mut(&mut self) -> &mut XdgFrozenFullscreen {
        &mut self.xdg_frozen
    }
}

/// A buffer handed out for one frame, plus the pool identity the damage
/// tracker needs to tell slot reuse from pool reallocation.
pub struct AcquiredBuffer {
    pub buffer: Buffer,
    pub canvas_ptr: usize,
    pub pool_generation: u64,
    pub pool_size: usize,
}

/// The active shell role for the surface.
pub enum SurfaceKind {
    Layer(LayerSurface),
    Xdg {
        #[allow(dead_code)]
        window: Window,
    },
}

#[derive(Debug, Default)]
struct FrameCallbackTracker {
    next_token: u64,
    pending_token: Option<u64>,
}

impl FrameCallbackTracker {
    fn begin(&mut self) -> u64 {
        self.next_token = self.next_token.wrapping_add(1).max(1);
        self.pending_token = Some(self.next_token);
        self.next_token
    }

    fn complete(&mut self, token: u64) -> bool {
        if self.pending_token != Some(token) {
            return false;
        }
        self.pending_token = None;
        true
    }

    fn clear(&mut self) {
        self.pending_token = None;
    }

    fn is_pending(&self) -> bool {
        self.pending_token.is_some()
    }
}

#[derive(Debug, Clone)]
pub(super) struct MainSurfaceFrameCallback {
    pub(super) surface: wl_surface::WlSurface,
    pub(super) token: u64,
    pub(super) capture_generation: Option<u64>,
}

/// Tracks the active layer surface, buffer pool, and associated sizing state.
pub struct SurfaceState {
    placement: SurfacePlacement,
    kind: Option<SurfaceKind>,
    wl_surface: Option<wl_surface::WlSurface>,
    pool: Option<SlotPool>,
    /// The `buffer_count` slots backing the swapchain, held for the pool's
    /// lifetime so it stays bounded: a slot's memory only returns to the
    /// pool's free list when the pool itself is dropped, never mid-frame.
    slots: Vec<Slot>,
    /// Generation counter incremented when pool is recreated.
    /// Used by damage tracker to detect pool reallocation.
    pool_generation: u64,
    /// Last known pool size, used to detect pool growth.
    pool_size: usize,
    current_output: Option<wl_output::WlOutput>,
    width: u32,
    height: u32,
    scale: i32,
    configured: bool,
    frame_callbacks: FrameCallbackTracker,
}

impl SurfaceState {
    /// Creates a new, unconfigured surface state.
    pub(in crate::backend::wayland) fn new(placement: SurfacePlacement) -> Self {
        Self {
            placement,
            kind: None,
            wl_surface: None,
            pool: None,
            slots: Vec::new(),
            pool_generation: 0,
            pool_size: 0,
            current_output: None,
            width: 0,
            height: 0,
            scale: 1,
            configured: false,
            frame_callbacks: FrameCallbackTracker::default(),
        }
    }

    pub(in crate::backend::wayland) fn placement(&self) -> &SurfacePlacement {
        &self.placement
    }

    pub(in crate::backend::wayland) fn placement_mut(&mut self) -> &mut SurfacePlacement {
        &mut self.placement
    }

    /// Assigns the layer surface produced during startup.
    pub fn set_layer_surface(&mut self, surface: LayerSurface) {
        self.wl_surface = Some(surface.wl_surface().clone());
        self.kind = Some(SurfaceKind::Layer(surface));
        // A new shell surface invalidates current buffer resources/state.
        self.drop_pool();
        self.current_output = None;
        self.configured = false;
        self.frame_callbacks.clear();
    }

    /// Assigns an xdg-shell window produced during startup.
    pub fn set_xdg_window(&mut self, window: Window) {
        self.wl_surface = Some(window.wl_surface().clone());
        self.kind = Some(SurfaceKind::Xdg { window });
        // A new shell surface invalidates current buffer resources/state.
        self.drop_pool();
        self.current_output = None;
        self.configured = false;
        self.frame_callbacks.clear();
    }

    /// Returns the active wl_surface, if initialized.
    pub fn wl_surface(&self) -> Option<&wl_surface::WlSurface> {
        self.wl_surface.as_ref()
    }

    /// Returns true if the provided wl_surface belongs to this overlay surface state.
    pub fn is_surface(&self, surface: &wl_surface::WlSurface) -> bool {
        self.wl_surface
            .as_ref()
            .map(|current| current.id() == surface.id())
            .unwrap_or(false)
    }

    /// Returns the mutable layer surface, if initialized.
    pub fn layer_surface_mut(&mut self) -> Option<&mut LayerSurface> {
        match &mut self.kind {
            Some(SurfaceKind::Layer(layer)) => Some(layer),
            _ => None,
        }
    }

    /// Returns the xdg-shell window, if initialized.
    pub fn xdg_window(&self) -> Option<&Window> {
        match &self.kind {
            Some(SurfaceKind::Xdg { window }) => Some(window),
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
            self.drop_pool();
        }
        changed
    }

    /// Updates the buffer scale (defaults to 1). Drops the pool when scale changes.
    pub fn set_scale(&mut self, scale: i32) {
        let scale = scale.max(1);
        if self.scale != scale {
            self.scale = scale;
            self.drop_pool();
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

    /// Creates the identity carried by a newly requested main-surface callback.
    pub(super) fn begin_frame_callback(
        &mut self,
        surface: wl_surface::WlSurface,
        capture_generation: Option<u64>,
    ) -> MainSurfaceFrameCallback {
        MainSurfaceFrameCallback {
            surface,
            token: self.frame_callbacks.begin(),
            capture_generation,
        }
    }

    /// Clears the render throttle only when this is still the newest callback.
    pub(super) fn complete_frame_callback(&mut self, token: u64) -> bool {
        self.frame_callbacks.complete(token)
    }

    /// Retires the current throttle token without assuming its callback vanished.
    pub(super) fn clear_frame_callback_pending(&mut self) {
        self.frame_callbacks.clear();
    }

    /// Returns whether a frame callback is currently outstanding.
    pub fn frame_callback_pending(&self) -> bool {
        self.frame_callbacks.is_pending()
    }

    /// Updates the stored pool size and returns true if it grew.
    ///
    /// Call this after `create_buffer` to detect if the pool grew during allocation.
    pub fn update_pool_size(&mut self, new_size: usize) -> bool {
        let grew = new_size > self.pool_size;
        if grew {
            info!(
                "Pool grew during create_buffer: {} -> {} bytes",
                self.pool_size, new_size
            );
        }
        self.pool_size = new_size;
        grew
    }

    /// Releases the pool and every slot held for it.
    ///
    /// Dropping the slots is what returns their memory to the pool's free
    /// list, so this must run whenever the pool itself is replaced.
    fn drop_pool(&mut self) {
        self.pool = None;
        self.pool_size = 0;
        self.slots.clear();
    }

    /// Ensures a shared memory pool of the appropriate size exists.
    ///
    /// The generation counter is incremented when a new pool is created, which
    /// lets the damage tracker detect pool reallocation (all previous canvas
    /// pointers become invalid).
    fn ensure_pool(&mut self, shm: &Shm, buffer_count: usize, slot_len: usize) -> Result<()> {
        if self.pool.is_some() {
            return Ok(());
        }
        let (phys_w, phys_h) = self.physical_dimensions();
        let initial_pool_size = slot_len * buffer_count;
        info!(
            "Creating new SlotPool ({}x{} @ scale {}, {} bytes, {} buffers, gen {})",
            phys_w,
            phys_h,
            self.scale,
            initial_pool_size,
            buffer_count,
            self.pool_generation + 1
        );
        let pool = SlotPool::new(initial_pool_size, shm).context("Failed to create slot pool")?;
        self.pool_size = pool.len();
        self.pool = Some(pool);
        self.pool_generation += 1;
        self.slots.clear();
        Ok(())
    }

    /// Hands out a buffer for this frame, or `None` while the compositor still
    /// owns every slot.
    ///
    /// The pool holds exactly `buffer_count` slots for its whole lifetime, and
    /// a slot is only drawn into while it has no active buffers - that is,
    /// after the compositor sent `wl_buffer.release` for the frame that used
    /// it last. Allocating a fresh slot per frame instead would let sctk grow
    /// the pool without bound whenever rendering outruns the compositor, which
    /// no-vsync rendering (especially `max_fps_no_vsync = 0`) does easily.
    pub fn acquire_buffer(
        &mut self,
        shm: &Shm,
        buffer_count: usize,
        width: i32,
        height: i32,
        stride: i32,
    ) -> Result<Option<AcquiredBuffer>> {
        let buffer_count = buffer_count.max(1);
        // sctk rounds slot lengths up to 64 bytes; size the pool the same way
        // so the last slot does not trigger a growth on the first frame.
        let slot_len = ((height as usize) * (stride as usize)).next_multiple_of(64);
        // Slots are never dropped individually: an in-flight buffer still
        // references its slot, so clearing them piecemeal would strand that
        // memory and let the next allocation grow the pool. Outgrowing the
        // slots rebuilds the pool wholesale instead, which resets the damage
        // tracker through the generation counter.
        if self.slots.iter().any(|slot| slot.len() < slot_len) {
            self.drop_pool();
        }
        self.ensure_pool(shm, buffer_count, slot_len)?;

        let pool_generation = self.pool_generation;
        let Self { pool, slots, .. } = self;
        let pool = pool
            .as_mut()
            .context("Buffer pool not initialized despite previous check")?;

        for _ in slots.len()..buffer_count {
            slots.push(pool.new_slot(slot_len).context("Failed to allocate slot")?);
        }

        let Some(slot) = slots.iter().find(|slot| !slot.has_active_buffers()) else {
            return Ok(None);
        };

        let buffer = pool
            .create_buffer_in(slot, width, height, stride, wl_shm::Format::Argb8888)
            .context("Failed to create buffer")?;
        let canvas_ptr = pool.raw_data_mut(slot).as_mut_ptr() as usize;
        let pool_size = pool.len();

        Ok(Some(AcquiredBuffer {
            buffer,
            canvas_ptr,
            pool_generation,
            pool_size,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_fullscreen_deadline_uses_injected_time() {
        let start = Instant::now();
        let mut state = XdgFrozenFullscreen::default();
        state.request(start);

        assert_eq!(state.timeout(start), Some(XDG_FROZEN_FULLSCREEN_TIMEOUT));
        assert_eq!(
            state.timeout(start + XDG_FROZEN_FULLSCREEN_TIMEOUT),
            Some(Duration::ZERO)
        );
        assert!(state.pending_configure());
        assert!(state.requested());
    }

    #[test]
    fn frozen_fullscreen_activate_and_finish_clear_pending_timeout() {
        let start = Instant::now();
        let mut state = XdgFrozenFullscreen::default();
        state.request(start);
        state.activate();

        assert!(state.requested());
        assert!(!state.pending_configure());
        assert_eq!(state.timeout(start + XDG_FROZEN_FULLSCREEN_TIMEOUT), None);

        state.finish();
        assert!(!state.requested());
        assert_eq!(state.timeout(start), None);
    }

    #[test]
    fn placement_keeps_output_fullscreen_and_layer_policy_together() {
        let placement = SurfacePlacement::new(Some("DP-1".to_owned()), true, true);

        assert_eq!(placement.preferred_output_identity(), Some("DP-1"));
        assert!(placement.xdg_fullscreen());
        assert_eq!(
            placement.layer(),
            smithay_client_toolkit::shell::wlr_layer::Layer::Overlay
        );
    }

    #[test]
    fn retired_callback_cannot_clear_a_newer_render_throttle() {
        let mut callbacks = FrameCallbackTracker::default();
        let timed_out = callbacks.begin();
        callbacks.clear();
        let recovery = callbacks.begin();

        assert!(!callbacks.complete(timed_out));
        assert!(callbacks.is_pending());
        assert!(callbacks.complete(recovery));
        assert!(!callbacks.is_pending());
    }
}
