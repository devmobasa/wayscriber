//! Drawing-only surfaces left on outputs while the single editor moves elsewhere.
use anyhow::{Context, Result, bail};
use cairo::ImageSurface;
use smithay_client_toolkit::compositor::Region;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{KeyboardInteractivity, LayerSurface};
use smithay_client_toolkit::shm::Shm;
use std::mem;
use std::slice;
use wayland_client::QueueHandle;
use wayland_client::protocol::{wl_output, wl_surface};

use crate::backend::wayland::surface::{MainSurfaceFrameCallback, SurfaceState};
use crate::canvas_export::{self, CanvasExportSnapshot, CanvasExportViewport};
use crate::draw::Shape;
use crate::i18n::tr;
use crate::input::state::{Toast, ToastPriority};

use super::WaylandState;

mod lifecycle;
#[cfg(test)]
mod tests;

/// Bounds the display buffers held by all passive outputs together.
const MAX_RETAINED_BYTES: u64 = 256 * 1024 * 1024;

/// A passive view repaints only to hide for a capture, to reappear, or after a
/// geometry change, so one buffer can stay on screen while the other is painted.
const PASSIVE_BUFFER_COUNT: usize = 2;

/// [`PASSIVE_BUFFER_COUNT`] as a multiplier of one frame's bytes.
const PASSIVE_BUFFER_FRAMES: u64 = PASSIVE_BUFFER_COUNT as u64;

#[derive(Default)]
pub(in crate::backend::wayland) struct LightOutputOverlays {
    outputs: Vec<LightOutputOverlay>,
    suppressed: bool,
}

struct LightOutputOverlay {
    surface: SurfaceState,
    snapshot: CanvasExportSnapshot,
    /// The drawing rendered ahead of its next paint. It is dropped once copied
    /// into a display buffer and rendered again from `snapshot` when needed.
    raster: Option<ImageSurface>,
    dirty: bool,
    hidden: bool,
}

impl LightOutputOverlay {
    fn bytes(&self) -> u64 {
        let (width, height) = self.surface.physical_dimensions();
        u64::from(width) * u64::from(height) * 4 * PASSIVE_BUFFER_FRAMES
    }

    fn rerasterize(&mut self, viewport: CanvasExportViewport) -> Result<()> {
        let mut snapshot = self.snapshot.clone();
        snapshot.viewport = viewport;
        let raster = canvas_export::render_canvas_surface(&snapshot)?;

        self.snapshot = snapshot;
        self.raster = Some(raster);
        self.surface
            .update_dimensions(viewport.logical_width, viewport.logical_height);
        self.surface.set_scale(viewport.scale);
        self.surface.set_configured(true);
        self.surface.clear_frame_callback_pending();
        self.hidden = false;
        self.dirty = true;
        Ok(())
    }

    fn paint(&mut self, context: &PassivePaintContext<'_>) -> Result<()> {
        if !self.dirty || !self.surface.is_configured() || self.surface.frame_callback_pending() {
            return Ok(());
        }

        let (width, height) = self.surface.physical_dimensions();
        let stride = i32::try_from(width.checked_mul(4).context("passive stride overflow")?)?;
        let Some(acquired) = self.surface.acquire_buffer(
            context.shm,
            PASSIVE_BUFFER_COUNT,
            i32::try_from(width)?,
            i32::try_from(height)?,
            stride,
        )?
        else {
            return Ok(());
        };

        let raster = if context.suppressed {
            None
        } else if let Some(raster) = self.raster.take() {
            Some(raster)
        } else {
            Some(canvas_export::render_canvas_surface(&self.snapshot)?)
        };

        let byte_len = usize::try_from(height)? * usize::try_from(stride)?;
        // SAFETY: acquire_buffer reserves an exclusive writable slot of these
        // dimensions. The temporary slice ends before the buffer is attached.
        let pixels = unsafe { slice::from_raw_parts_mut(acquired.canvas_ptr as *mut u8, byte_len) };
        if let Some(raster) = raster {
            let mut copied = false;
            raster.with_data(|data| {
                if data.len() == pixels.len() {
                    pixels.copy_from_slice(data);
                    copied = true;
                }
            })?;
            if !copied {
                bail!("passive drawing does not match its display buffer");
            }
        } else {
            pixels.fill(0);
        }

        let surface = self
            .surface
            .wl_surface()
            .context("passive surface missing")?
            .clone();
        surface.set_buffer_scale(self.surface.scale());
        acquired.buffer.attach_to(&surface)?;
        surface.damage_buffer(0, 0, i32::try_from(width)?, i32::try_from(height)?);
        let callback = self.surface.begin_frame_callback(surface.clone(), None);
        surface.frame(context.qh, callback);
        surface.commit();

        self.hidden = context.suppressed;
        self.dirty = false;
        Ok(())
    }
}

struct PassivePaintContext<'state> {
    shm: &'state Shm,
    qh: &'state QueueHandle<WaylandState>,
    suppressed: bool,
}

impl LightOutputOverlays {
    pub(in crate::backend::wayland) fn ready_for_capture(&self) -> bool {
        self.outputs.iter().all(|output| {
            output.hidden && !output.dirty && !output.surface.frame_callback_pending()
        })
    }

    pub(in crate::backend::wayland) fn needs_render(&self) -> bool {
        self.outputs.iter().any(|output| {
            output.dirty
                && output.surface.is_configured()
                && !output.surface.frame_callback_pending()
                && output.surface.has_available_buffer(PASSIVE_BUFFER_COUNT)
        })
    }

    pub(in crate::backend::wayland) fn set_suppressed(&mut self, suppressed: bool) {
        self.suppressed = suppressed;
        for output in &mut self.outputs {
            output.dirty = true;
            output.hidden = false;
            // A previous normal/hidden callback cannot acknowledge this transition.
            output.surface.clear_frame_callback_pending();
        }
    }

    pub(in crate::backend::wayland) fn remove_output(&mut self, output: &wl_output::WlOutput) {
        self.outputs
            .retain(|retained| retained.surface.current_output().as_ref() != Some(output));
    }

    fn index_of(&self, surface: &wl_surface::WlSurface) -> Option<usize> {
        self.outputs
            .iter()
            .position(|output| output.surface.is_surface(surface))
    }

    pub(in crate::backend::wayland) fn close_layer(&mut self, layer: &LayerSurface) -> bool {
        let Some(index) = self.index_of(layer.wl_surface()) else {
            return false;
        };

        self.outputs.remove(index);
        true
    }

    pub(in crate::backend::wayland) fn configure_layer(
        &mut self,
        layer: &LayerSurface,
        width: u32,
        height: u32,
    ) -> bool {
        let Some(index) = self.index_of(layer.wl_surface()) else {
            return false;
        };

        let previous = self.outputs[index].snapshot.viewport;
        let viewport = CanvasExportViewport {
            logical_width: if width == 0 {
                previous.logical_width
            } else {
                width
            },
            logical_height: if height == 0 {
                previous.logical_height
            } else {
                height
            },
            ..previous
        };
        self.resize_or_close(index, viewport);
        true
    }

    pub(in crate::backend::wayland) fn scale_surface(
        &mut self,
        surface: &wl_surface::WlSurface,
        scale: i32,
    ) -> bool {
        let Some(index) = self.index_of(surface) else {
            return false;
        };

        let viewport = CanvasExportViewport {
            scale: scale.max(1),
            ..self.outputs[index].snapshot.viewport
        };
        self.resize_or_close(index, viewport);
        true
    }

    fn resize_or_close(&mut self, index: usize, viewport: CanvasExportViewport) {
        // A repeated configure keeps the drawing already on screen.
        if viewport == self.outputs[index].snapshot.viewport {
            return;
        }

        let others: u64 = self
            .outputs
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != index)
            .map(|(_, output)| output.bytes())
            .sum();
        let resized = check_budget(
            others,
            viewport.logical_width,
            viewport.logical_height,
            viewport.scale,
        )
        .and_then(|()| self.outputs[index].rerasterize(viewport));

        if let Err(error) = resized {
            log::warn!("Closing passive overlay after its geometry changed: {error:#}");
            self.outputs.remove(index);
        }
    }

    pub(in crate::backend::wayland) fn complete_frame(
        &mut self,
        data: &MainSurfaceFrameCallback,
    ) -> bool {
        let Some(index) = self.index_of(&data.surface) else {
            return false;
        };

        self.outputs[index]
            .surface
            .complete_frame_callback(data.token);
        true
    }
}

fn check_budget(retained: u64, width: u32, height: u32, scale: i32) -> Result<()> {
    let scale = u64::try_from(scale.max(1))?;
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| {
            pixels
                .checked_mul(scale)?
                .checked_mul(scale)?
                .checked_mul(4)?
                .checked_mul(PASSIVE_BUFFER_FRAMES)
        })
        .and_then(|bytes| bytes.checked_add(retained));
    if width == 0 || height == 0 || bytes.is_none_or(|bytes| bytes > MAX_RETAINED_BYTES) {
        bail!("passive overlays exceed the 256 MiB display budget or have invalid dimensions");
    }

    Ok(())
}

impl WaylandState {
    pub(in crate::backend::wayland) fn light_output_overlays_enabled(&self) -> bool {
        self.config.ui.keep_light_overlays_on_all_outputs
            && self.config.ui.multi_monitor_enabled
            && self.protocol.layer_shell().is_some()
            && self.config.session.per_output
            && self.config.session.persist_transparent
            && self.session_options().is_some_and(|options| {
                options.per_output && options.persist_transparent && !options.is_named_file()
            })
    }

    /// The editor may paint a session only after it belongs to this surface's
    /// output. A deferred/failed save keeps the old content on its passive host.
    pub(in crate::backend::wayland) fn light_output_session_ready(&self) -> bool {
        let Some(output) = self.surface.current_output() else {
            return false;
        };
        let Some(options) = self.session_options() else {
            return false;
        };

        self.session.is_loaded()
            && self.session.pending_output_transition().is_none()
            && options.targets_output_identity(self.output_identity_for(&output).as_deref())
    }

    /// A newly placed editor must not edit the previous output's live session
    /// while its own session is still loading or waiting for a save receipt.
    pub(in crate::backend::wayland) fn light_output_session_pending(&self) -> bool {
        self.light_output_overlays_enabled() && !self.light_output_session_ready()
    }

    pub(in crate::backend::wayland) fn retain_light_output(&mut self) -> Result<()> {
        if !self.light_output_overlays_enabled()
            || !self.input_state.light_mode_passthrough()
            || !self.input_state.board_is_transparent()
            || !self.surface.is_configured()
        {
            return Ok(());
        }
        let output = self
            .surface
            .current_output()
            .context("active output is not known yet")?;
        if self.session_transaction.is_some() || !self.light_output_session_ready() {
            bail!("the previous output session transition has not finished");
        }

        let retained: u64 = self
            .light_outputs
            .outputs
            .iter()
            .filter(|retained| retained.surface.current_output().as_ref() != Some(&output))
            .map(LightOutputOverlay::bytes)
            .sum();
        check_budget(
            retained,
            self.surface.width(),
            self.surface.height(),
            self.surface.scale(),
        )?;

        let mut snapshot = self.canvas_export_snapshot();
        // This is a display snapshot: PNG export can deliberately use a
        // different profile (or none) from the currently visible canvas.
        snapshot.render_profile = self.input_state.active_canvas_render_profile().cloned();
        // Like the live transparent canvas, retain ordinary spotlight openings
        // when no desktop pixels are available to magnify. Only this display
        // copy changes; the session keeps each spotlight's requested factor.
        for drawn in &mut snapshot.board.frame.shapes {
            if let Shape::Spotlight { magnification, .. } = &mut drawn.shape {
                *magnification = 1.0;
            }
        }
        let raster = canvas_export::render_canvas_surface(&snapshot)?;

        let layer = self
            .surface
            .layer_surface_mut()
            .context("light overlay requires layer-shell")?;
        let region = Region::new(self.protocol.compositor())?;
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer
            .wl_surface()
            .set_input_region(Some(region.wl_region()));
        layer.commit();

        self.light_outputs.remove_output(&output);
        let placement = self.surface.placement().clone();
        let mut surface = mem::replace(&mut self.surface, SurfaceState::new(placement));
        // The editor's pool holds `performance.buffer_count` slots. Passive
        // views allocate their own smaller pool on their first paint.
        surface.release_buffer_pool();
        // The original callback belongs to the now-passive host. Give its clean
        // drawing-only frame its own acknowledgement, independent of the editor.
        surface.clear_frame_callback_pending();
        self.light_outputs.outputs.push(LightOutputOverlay {
            surface,
            snapshot,
            raster: Some(raster),
            dirty: true,
            hidden: false,
        });
        Ok(())
    }

    /// Paints every passive view that changed. A view that cannot be painted
    /// is closed so the editor and the other monitors keep working.
    pub(in crate::backend::wayland) fn render_light_outputs(&mut self, qh: &QueueHandle<Self>) {
        if !self.light_output_overlays_enabled() {
            self.light_outputs.outputs.clear();
        }

        let context = PassivePaintContext {
            shm: self.protocol.shm(),
            qh,
            suppressed: self.light_outputs.suppressed,
        };
        let before = self.light_outputs.outputs.len();
        self.light_outputs
            .outputs
            .retain_mut(|output| match output.paint(&context) {
                Ok(()) => true,
                Err(error) => {
                    log::warn!("Closing passive overlay that could not be painted: {error:#}");
                    false
                }
            });

        if self.light_outputs.outputs.len() < before {
            self.input_state.push_toast(
                ToastPriority::Critical,
                "light.outputs",
                Toast::warning(tr(
                    "Could not keep showing the drawings on another monitor.",
                )),
            );
        }
        self.begin_ready_overlay_capture(qh);
    }
}
