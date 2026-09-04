use super::plan::FramePlan;
use super::*;
use crate::backend::wayland::surface::AcquiredBuffer;

impl WaylandState {
    pub(super) fn submit_frame(
        &mut self,
        qh: &QueueHandle<Self>,
        acquired: AcquiredBuffer,
        plan: &FramePlan,
        breakdown: &mut Option<PerfRenderBreakdown>,
    ) -> Result<()> {
        let width = plan.geometry.width;
        let height = plan.geometry.height;
        let scale = plan.geometry.scale;
        let scaled_damage = &plan.damage.buffer;
        let buffer = acquired.buffer;
        record_stage!(breakdown, damage_commit, {
            // Attach buffer and commit
            debug!("Attaching buffer and committing surface");
            let wl_surface = self
                .surface
                .wl_surface()
                .cloned()
                .context("Surface not created")?;
            wl_surface.set_buffer_scale(scale);
            // `attach_to` marks the slot active until the compositor releases the
            // buffer. Attaching the raw `wl_buffer()` instead leaves the slot free,
            // so the pool hands the same memory back on the next frame and the next
            // paint lands in the buffer the compositor is still reading - the whole
            // swapchain collapses to one slot and partial damage resurfaces stale or
            // half-drawn pixels.
            buffer
                .attach_to(&wl_surface)
                .map_err(|err| anyhow::anyhow!("failed to attach the overlay buffer: {err}"))?;

            if debug_damage_logging_enabled() {
                debug!(
                    "Damage (scaled): count={}, {}",
                    scaled_damage.len(),
                    damage_summary(scaled_damage)
                );
            }

            // Apply per-buffer damage regions for correct incremental rendering.
            // Each buffer tracks damage since it was last displayed, avoiding stale pixels.
            for region in scaled_damage {
                wl_surface.damage_buffer(region.x, region.y, region.width, region.height);
            }

            let capture_generation = self.suppression.barrier.begin_main_surface_submission();
            if self.config.performance.enable_vsync {
                debug!("Requesting frame callback (vsync enabled)");
                let callback = self
                    .surface
                    .begin_frame_callback(wl_surface.clone(), capture_generation);
                wl_surface.frame(qh, callback);
            } else if capture_generation.is_some() {
                debug!("Requesting frame callback (preflight)");
                let callback = self
                    .surface
                    .begin_frame_callback(wl_surface.clone(), capture_generation);
                wl_surface.frame(qh, callback);
            } else {
                debug!("Skipping frame callback (vsync disabled - allows back-to-back renders)");
            }

            self.commit_perf_frame(
                PerfFrameDamageContext {
                    damage_screen: &plan.damage.screen,
                    logical_width: width,
                    logical_height: height,
                    damage_rects: scaled_damage.len(),
                    force_full_reason: plan.damage.full_reason,
                    diagnostics: plan.damage.diagnostics,
                },
                Instant::now(),
            );
            wl_surface.commit();
            Ok::<(), anyhow::Error>(())
        })?;
        debug!("=== RENDER COMPLETE ===");

        // Render toolbar overlays if visible, only when state/hover changed.
        record_stage!(breakdown, toolbar, {
            self.render_layer_toolbars_if_needed();
        });
        if let Some(breakdown) = breakdown.take() {
            self.record_perf_render_breakdown(breakdown);
        }

        if self.suppression.capture_suppressed() {
            self.capture.mark_preflight_rendered();
        }
        Ok(())
    }
}
