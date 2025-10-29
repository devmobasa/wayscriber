// Holds the live Wayland protocol state shared by the backend loop and the handler
// submodules; provides rendering, capture routing, and overlay helpers used across them.
use anyhow::{Context, Result};
use log::debug;
use smithay_client_toolkit::{
    compositor::CompositorState, output::OutputState, registry::RegistryState, seat::SeatState,
    shell::{WaylandSurface, wlr_layer::LayerShell}, shm::Shm,
};
use wayland_client::{
    QueueHandle,
    protocol::{wl_output, wl_shm},
};

use crate::{
    capture::{
        CaptureDestination, CaptureManager,
        file::{FileSaveConfig, expand_tilde},
        types::CaptureType,
    },
    config::{Action, Config},
    input::{DrawingState, InputState},
    session::SessionOptions,
    util::Rect,
};

use super::surface::SurfaceState;

/// Internal Wayland state shared across modules.
pub(super) struct WaylandState {
    // Wayland protocol objects
    pub(super) registry_state: RegistryState,
    pub(super) compositor_state: CompositorState,
    pub(super) layer_shell: LayerShell,
    pub(super) shm: Shm,
    pub(super) output_state: OutputState,
    pub(super) seat_state: SeatState,

    // Surface and buffer management
    pub(super) surface: SurfaceState,

    // Configuration
    pub(super) config: Config,

    // Input state
    pub(super) input_state: InputState,
    pub(super) current_mouse_x: i32,
    pub(super) current_mouse_y: i32,

    // Capture manager
    pub(super) capture_manager: CaptureManager,

    // Capture state tracking
    pub(super) capture_in_progress: bool,
    pub(super) overlay_hidden_for_capture: bool,

    // Session persistence
    pub(super) session_options: Option<SessionOptions>,
    pub(super) session_loaded: bool,
    pub(super) last_loaded_identity: Option<String>,

    // Tokio runtime handle for async operations
    pub(super) tokio_handle: tokio::runtime::Handle,
}

impl WaylandState {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        registry_state: RegistryState,
        compositor_state: CompositorState,
        layer_shell: LayerShell,
        shm: Shm,
        output_state: OutputState,
        seat_state: SeatState,
        config: Config,
        input_state: InputState,
        capture_manager: CaptureManager,
        session_options: Option<SessionOptions>,
        tokio_handle: tokio::runtime::Handle,
    ) -> Self {
        Self {
            registry_state,
            compositor_state,
            layer_shell,
            shm,
            output_state,
            seat_state,
            surface: SurfaceState::new(),
            config,
            input_state,
            current_mouse_x: 0,
            current_mouse_y: 0,
            capture_manager,
            capture_in_progress: false,
            overlay_hidden_for_capture: false,
            session_options,
            session_loaded: false,
            last_loaded_identity: None,
            tokio_handle,
        }
    }

    pub(super) fn session_options(&self) -> Option<&SessionOptions> {
        self.session_options.as_ref()
    }

    pub(super) fn session_options_mut(&mut self) -> Option<&mut SessionOptions> {
        self.session_options.as_mut()
    }

    pub(super) fn output_identity_for(&self, output: &wl_output::WlOutput) -> Option<String> {
        let info = self.output_state.info(output)?;

        let mut components: Vec<String> = Vec::new();

        if let Some(name) = info.name.filter(|s| !s.is_empty()) {
            components.push(name);
        }

        if !info.make.is_empty() {
            components.push(info.make);
        }

        if !info.model.is_empty() {
            components.push(info.model);
        }

        if components.is_empty() {
            components.push(format!("id{}", info.id));
        }

        Some(components.join("-"))
    }

    pub(super) fn render(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        debug!("=== RENDER START ===");
        // Create pool if needed
        let buffer_count = self.config.performance.buffer_count as usize;
        let width = self.surface.width();
        let height = self.surface.height();

        // Get a buffer from the pool
        let (buffer, canvas) = {
            let pool = self.surface.ensure_pool(&self.shm, buffer_count)?;
            debug!("Requesting buffer from pool");
            let result = pool
                .create_buffer(
                    width as i32,
                    height as i32,
                    (width * 4) as i32,
                    wl_shm::Format::Argb8888,
                )
                .context("Failed to create buffer")?;
            debug!("Buffer acquired from pool");
            result
        };

        // SAFETY: This unsafe block creates a Cairo surface from raw memory buffer.
        // Safety invariants that must be maintained:
        // 1. `canvas` is a valid mutable slice from SlotPool with exactly (width * height * 4) bytes
        // 2. The buffer format ARgb32 matches the allocation (4 bytes per pixel: alpha, red, green, blue)
        // 3. The stride (width * 4) correctly represents the number of bytes per row
        // 4. `cairo_surface` and `ctx` are explicitly dropped before the buffer is committed to Wayland,
        //    ensuring Cairo doesn't access memory after ownership transfers
        // 5. No other references to this memory exist during Cairo's usage
        // 6. The buffer remains valid throughout Cairo's usage (enforced by Rust's borrow checker)
        let cairo_surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                canvas.as_mut_ptr(),
                cairo::Format::ARgb32,
                width as i32,
                height as i32,
                (width * 4) as i32,
            )
            .context("Failed to create Cairo surface")?
        };

        // Render using Cairo
        let ctx = cairo::Context::new(&cairo_surface).context("Failed to create Cairo context")?;

        // Clear with fully transparent background
        debug!("Clearing background");
        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().context("Failed to clear background")?;
        ctx.set_operator(cairo::Operator::Over);

        // Render board background if in board mode (whiteboard/blackboard)
        crate::draw::render_board_background(
            &ctx,
            self.input_state.board_mode(),
            &self.input_state.board_config,
        );

        // Render all completed shapes from active frame
        debug!(
            "Rendering {} completed shapes",
            self.input_state.canvas_set.active_frame().shapes.len()
        );
        crate::draw::render_shapes(&ctx, &self.input_state.canvas_set.active_frame().shapes);

        // Render provisional shape if actively drawing
        // Use optimized method that avoids cloning for freehand
        if self.input_state.render_provisional_shape(
            &ctx,
            self.current_mouse_x,
            self.current_mouse_y,
        ) {
            debug!("Rendered provisional shape");
        }

        // Render text cursor/buffer if in text mode
        if let DrawingState::TextInput { x, y, buffer } = &self.input_state.state {
            let preview_text = if buffer.is_empty() {
                "_".to_string() // Show cursor when buffer is empty
            } else {
                format!("{}_", buffer)
            };
            crate::draw::render_text(
                &ctx,
                *x,
                *y,
                &preview_text,
                self.input_state.current_color,
                self.input_state.current_font_size,
                &self.input_state.font_descriptor,
                self.input_state.text_background_enabled,
            );
        }

        // Render status bar if enabled
        if self.input_state.show_status_bar {
            crate::ui::render_status_bar(
                &ctx,
                &self.input_state,
                self.config.ui.status_bar_position,
                &self.config.ui.status_bar_style,
                width,
                height,
            );
        }

        // Render help overlay if toggled
        if self.input_state.show_help {
            crate::ui::render_help_overlay(&ctx, &self.config.ui.help_overlay_style, width, height);
        }

        // Flush Cairo
        debug!("Flushing Cairo surface");
        cairo_surface.flush();
        drop(ctx);
        drop(cairo_surface);

        // Attach buffer and commit
        debug!("Attaching buffer and committing surface");
        let wl_surface = self
            .surface
            .layer_surface()
            .context("Layer surface not created")?
            .wl_surface();
        wl_surface.attach(Some(buffer.wl_buffer()), 0, 0);

        let surface_width = width.min(i32::MAX as u32) as i32;
        let surface_height = height.min(i32::MAX as u32) as i32;

        let dirty_regions = resolve_damage_regions(
            surface_width,
            surface_height,
            self.input_state.take_dirty_regions(),
        );

        if dirty_regions.is_empty() {
            debug!("No valid dirty regions; skipping damage request");
        } else {
            for rect in &dirty_regions {
                debug!(
                    "Damaging buffer region x={} y={} w={} h={}",
                    rect.x, rect.y, rect.width, rect.height
                );
                wl_surface.damage_buffer(rect.x, rect.y, rect.width, rect.height);
            }
        }

        if self.config.performance.enable_vsync {
            debug!("Requesting frame callback (vsync enabled)");
            wl_surface.frame(qh, wl_surface.clone());
        } else {
            debug!("Skipping frame callback (vsync disabled - allows back-to-back renders)");
        }

        wl_surface.commit();
        debug!("=== RENDER COMPLETE ===");

        Ok(())
    }

    /// Restore the overlay after screenshot capture completes.
    ///
    /// Re-maps the layer surface to its original size and forces a redraw.
    pub(super) fn show_overlay(&mut self) {
        if !self.overlay_hidden_for_capture {
            log::warn!("Overlay was not hidden, nothing to restore");
            return;
        }

        log::info!("Restoring overlay after screenshot capture");

        let width = self.surface.width();
        let height = self.surface.height();
        if let Some(layer_surface) = self.surface.layer_surface_mut() {
            layer_surface.set_size(width, height);

            let wl_surface = layer_surface.wl_surface();
            wl_surface.commit();
        }

        self.overlay_hidden_for_capture = false;

        // Force a redraw to show the overlay again
        self.input_state.needs_redraw = true;
    }

    /// Handles capture actions by delegating to the CaptureManager.
    pub(super) fn handle_capture_action(&mut self, action: Action) {
        if !self.config.capture.enabled {
            log::warn!("Capture action triggered but capture is disabled in config");
            return;
        }

        if self.capture_in_progress {
            log::warn!(
                "Capture action {:?} requested while another capture is running; ignoring",
                action
            );
            return;
        }

        let default_destination = if self.config.capture.copy_to_clipboard {
            CaptureDestination::ClipboardAndFile
        } else {
            CaptureDestination::FileOnly
        };

        let (capture_type, destination) = match action {
            Action::CaptureFullScreen => (CaptureType::FullScreen, default_destination),
            Action::CaptureActiveWindow => (CaptureType::ActiveWindow, default_destination),
            Action::CaptureSelection => (
                CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                default_destination,
            ),
            Action::CaptureClipboardFull => {
                (CaptureType::FullScreen, CaptureDestination::ClipboardOnly)
            }
            Action::CaptureFileFull => (CaptureType::FullScreen, CaptureDestination::FileOnly),
            Action::CaptureClipboardSelection => (
                CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                CaptureDestination::ClipboardOnly,
            ),
            Action::CaptureFileSelection => (
                CaptureType::Selection {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
                CaptureDestination::FileOnly,
            ),
            Action::CaptureClipboardRegion => {
                log::info!("Region clipboard capture requested");
                (
                    CaptureType::Selection {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    },
                    CaptureDestination::ClipboardOnly,
                )
            }
            Action::CaptureFileRegion => {
                log::info!("Region file capture requested");
                (
                    CaptureType::Selection {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    },
                    CaptureDestination::FileOnly,
                )
            }
            _ => {
                log::error!(
                    "Non-capture action passed to handle_capture_action: {:?}",
                    action
                );
                return;
            }
        };

        // Build file save config from user config when needed
        let save_config = if matches!(destination, CaptureDestination::ClipboardOnly) {
            None
        } else {
            Some(FileSaveConfig {
                save_directory: expand_tilde(&self.config.capture.save_directory),
                filename_template: self.config.capture.filename_template.clone(),
                format: self.config.capture.format.clone(),
            })
        };

        // Hide overlay before capture to prevent capturing the overlay itself
        self.hide_overlay();
        self.capture_in_progress = true;

        // Request capture
        log::info!("Requesting {:?} capture", capture_type);
        if let Err(e) = self
            .capture_manager
            .request_capture(capture_type, destination, save_config)
        {
            log::error!("Failed to request capture: {}", e);

            // Restore overlay on error
            self.show_overlay();
            self.capture_in_progress = false;
        }
    }

    fn hide_overlay(&mut self) {
        if self.overlay_hidden_for_capture {
            log::warn!("Overlay already hidden for capture");
            return;
        }

        log::info!("Hiding overlay for screenshot capture");

        if let Some(layer_surface) = self.surface.layer_surface_mut() {
            layer_surface.set_size(0, 0);

            let wl_surface = layer_surface.wl_surface();
            wl_surface.commit();
        }

        self.overlay_hidden_for_capture = true;
    }
}

fn resolve_damage_regions(width: i32, height: i32, mut regions: Vec<Rect>) -> Vec<Rect> {
    regions.retain(Rect::is_valid);

    if regions.is_empty() && width > 0 && height > 0 {
        if let Some(full) = Rect::new(0, 0, width, height) {
            regions.push(full);
        }
    }

    regions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_damage_returns_full_when_empty() {
        let regions = resolve_damage_regions(1920, 1080, Vec::new());
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], Rect::new(0, 0, 1920, 1080).unwrap());
    }

    #[test]
    fn resolve_damage_filters_invalid_rects() {
        let regions = resolve_damage_regions(
            800,
            600,
            vec![
                Rect {
                    x: 10,
                    y: 10,
                    width: 50,
                    height: 40,
                },
                Rect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 10,
                },
            ],
        );

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], Rect::new(10, 10, 50, 40).unwrap());
    }

    #[test]
    fn resolve_damage_preserves_existing_regions() {
        let regions = resolve_damage_regions(
            800,
            600,
            vec![Rect {
                x: 5,
                y: 5,
                width: 20,
                height: 30,
            }],
        );

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], Rect::new(5, 5, 20, 30).unwrap());
    }
}
