use super::plan::FramePlan;
use super::profile::{PixelBuffer, ProfileMode};
use super::*;
use crate::backend::wayland::surface::AcquiredBuffer;

impl WaylandState {
    pub(super) fn paint_frame(
        &mut self,
        plan: &FramePlan,
        acquired: &AcquiredBuffer,
        breakdown: &mut Option<PerfRenderBreakdown>,
    ) -> Result<()> {
        let canvas_ptr = acquired.canvas_ptr;
        let FrameGeometry {
            width,
            height,
            scale,
            physical_width: phys_width,
            physical_height: phys_height,
            ..
        } = plan.geometry;
        let damage_screen = &plan.damage.screen;
        let render_canvas = plan.render_canvas;
        let render_ui = plan.render_ui;
        // The acquired SHM slot supplies width * height * 4 bytes in ARgb32.
        // It stays owned by this render attempt until submission. All Cairo
        // handles are dropped before attach_to marks the slot in flight.
        let draw_start = std::time::Instant::now();
        let (cairo_surface, ctx) = record_stage!(breakdown, cairo_surface, {
            // SAFETY: the unsubmitted slot owns the backing memory throughout
            // painting; geometry and stride match its acquisition parameters.
            let cairo_surface = unsafe {
                cairo::ImageSurface::create_for_data_unsafe(
                    canvas_ptr as *mut u8,
                    cairo::Format::ARgb32,
                    phys_width as i32,
                    phys_height as i32,
                    plan.geometry.stride,
                )
                .context("Failed to create Cairo surface")
            };
            cairo_surface.and_then(|cairo_surface| {
                let ctx = cairo::Context::new(&cairo_surface)
                    .context("Failed to create Cairo context")?;
                Ok((cairo_surface, ctx))
            })
        })?;

        record_stage!(breakdown, clear_clip, {
            // Optimization: Clip drawing to the damage regions.
            // This dramatically reduces CPU fill rate pressure on high-res screens by
            // avoiding redraws of static content (which is preserved in the back-buffer).
            // Note: Cairo works in logical coordinates if we scale it, but here we are
            // pre-scale (identity transform). We must scale the logical damage rects to pixels.
            if !damage_screen.is_empty() {
                for rect in damage_screen {
                    // Scale logical rect to physical pixels
                    let x = rect.x as f64 * scale as f64;
                    let y = rect.y as f64 * scale as f64;
                    let w = rect.width as f64 * scale as f64;
                    let h = rect.height as f64 * scale as f64;
                    ctx.rectangle(x, y, w, h);
                }
                ctx.clip();
            }

            // Clear with fully transparent background (only clears within clip)
            debug!("Clearing background");
            ctx.set_operator(cairo::Operator::Clear);
            ctx.paint().context("Failed to clear background")?;
            ctx.set_operator(cairo::Operator::Over);
            Ok::<(), anyhow::Error>(())
        })?;

        if render_canvas {
            self.render_canvas_layer(
                &canvas::CanvasRenderCtx {
                    cairo: &ctx,
                    geometry: &plan.geometry,
                    canvas: &plan.canvas,
                    damage_world: &plan.damage.world,
                    now: plan.now,
                },
                breakdown.as_mut(),
            )?;
        }

        record_stage!(breakdown, render_profile, {
            if plan.profile.needs_before_ui(render_ui) {
                cairo_surface.flush();
                // SAFETY: the acquired slot remains exclusive to this paint pass.
                // Cairo is flushed and the temporary slice ends before UI painting.
                let data = unsafe {
                    std::slice::from_raw_parts_mut(canvas_ptr as *mut u8, plan.geometry.byte_len)
                };
                let rewritten = plan.profile.before_ui(
                    PixelBuffer {
                        data,
                        width: phys_width as i32,
                        height: phys_height as i32,
                        stride: plan.geometry.stride,
                        damage: &plan.damage.buffer,
                    },
                    self.render.profile_ui_baseline_mut(),
                    render_ui,
                );
                if rewritten {
                    cairo_surface.mark_dirty();
                }
            }
        });

        record_stage!(breakdown, ui, {
            self.render_ui_layer(&ctx, width, height, scale, render_ui);
        });

        // Flush Cairo
        debug!("Flushing Cairo surface");
        cairo_surface.flush();
        drop(ctx);
        drop(cairo_surface);

        record_stage!(breakdown, render_profile, {
            if matches!(
                plan.profile.mode(),
                ProfileMode::Ui | ProfileMode::CanvasAndUi
            ) {
                // SAFETY: Cairo is flushed and dropped, and the slot has not been
                // attached. This is the only access to its pixel bytes.
                let data = unsafe {
                    std::slice::from_raw_parts_mut(canvas_ptr as *mut u8, plan.geometry.byte_len)
                };
                plan.profile.after_ui(
                    PixelBuffer {
                        data,
                        width: phys_width as i32,
                        height: phys_height as i32,
                        stride: plan.geometry.stride,
                        damage: &plan.damage.buffer,
                    },
                    self.render.profile_ui_baseline(),
                    render_ui,
                );
            }
        });

        let draw_duration = draw_start.elapsed();
        if self.input_state.region_is_active()
            && self
                .input_state
                .region_state()
                .purpose()
                .is_some_and(|purpose| purpose.is_capture())
        {
            debug!(
                "Region picker frame: logical={}x{}, physical={}x{}, scale={}, cairo_draw={:?}",
                width, height, phys_width, phys_height, scale, draw_duration
            );
        }
        if draw_duration > std::time::Duration::from_millis(2) {
            debug!("Cairo draw took {:?}", draw_duration);
        }

        Ok(())
    }
}
