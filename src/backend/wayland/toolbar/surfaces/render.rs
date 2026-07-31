use std::time::Instant;

use anyhow::{Result, anyhow};
use log::debug;
use smithay_client_toolkit::{
    shell::WaylandSurface,
    shm::{Shm, slot::SlotPool},
};

use super::structs::ToolbarSurface;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::render_profiles::RenderColorProfile;
use crate::ui::toolbar::ToolbarSnapshot;

impl ToolbarSurface {
    /// Render helper used by the manager; keeps render impl closer to surface.
    pub fn render<F>(
        &mut self,
        shm: &Shm,
        snapshot: &ToolbarSnapshot,
        hover: Option<(f64, f64)>,
        hover_start: Option<Instant>,
        render_profile: Option<&RenderColorProfile>,
        render_fn: F,
    ) -> Result<()>
    where
        F: FnOnce(
            &cairo::Context,
            f64,
            f64,
            &ToolbarSnapshot,
            &mut Vec<HitRegion>,
            Option<(f64, f64)>,
            Option<Instant>,
        ) -> Result<()>,
    {
        if !self.configured || !self.dirty || self.width == 0 || self.height == 0 {
            debug!(
                "Skipping render for toolbar '{}' (configured={}, dirty={}, width={}, height={}, scale={})",
                self.name, self.configured, self.dirty, self.width, self.height, self.scale
            );
            return Ok(());
        }

        let (phys_w, phys_h) = (
            self.width.saturating_mul(self.scale as u32),
            self.height.saturating_mul(self.scale as u32),
        );

        // Every failure below leaves `dirty` set so the next frame retries, and
        // is reported rather than swallowed: returning Ok here used to leave
        // the toolbar permanently blank with nothing in the log to say why.
        if self.pool.is_none() {
            let buffer_size = (phys_w * phys_h * 4) as usize;
            match SlotPool::new(buffer_size, shm) {
                Ok(pool) => self.pool = Some(pool),
                Err(err) => {
                    return Err(anyhow!(
                        "failed to create a {buffer_size}-byte shm pool: {err}"
                    ));
                }
            }
        }

        let pool = match self.pool.as_mut() {
            Some(p) => p,
            None => return Err(anyhow!("shm pool is missing after creation")),
        };
        let (buffer, canvas) = pool
            .create_buffer(
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
                wayland_client::protocol::wl_shm::Format::Argb8888,
            )
            .map_err(|err| anyhow!("failed to create a {phys_w}x{phys_h} buffer: {err}"))?;

        let surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                canvas.as_mut_ptr(),
                cairo::Format::ARgb32,
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
            )
        }
        .map_err(|err| anyhow!("failed to wrap the buffer in a cairo surface: {err}"))?;
        let ctx = cairo::Context::new(&surface)
            .map_err(|err| anyhow!("failed to create a cairo context: {err}"))?;

        ctx.set_operator(cairo::Operator::Clear);
        let _ = ctx.paint();
        ctx.set_operator(cairo::Operator::Over);

        self.hit_regions.clear();
        if !self.suppressed {
            // Sanitize ui_scale: handle NaN/Inf and enforce bounds
            let ui_scale = if self.ui_scale.is_finite() {
                self.ui_scale.clamp(0.5, 3.0)
            } else {
                1.0
            };
            let (logical_w, logical_h) =
                (self.width as f64 / ui_scale, self.height as f64 / ui_scale);
            let hover_scaled = hover.map(|(x, y)| (x / ui_scale, y / ui_scale));
            if self.scale > 1 {
                ctx.scale(self.scale as f64, self.scale as f64);
            }
            if (ui_scale - 1.0).abs() > f64::EPSILON {
                ctx.scale(ui_scale, ui_scale);
            }
            render_fn(
                &ctx,
                logical_w,
                logical_h,
                snapshot,
                &mut self.hit_regions,
                hover_scaled,
                hover_start,
            )?;

            if (ui_scale - 1.0).abs() > f64::EPSILON {
                for hit in &mut self.hit_regions {
                    hit.rect.0 *= ui_scale;
                    hit.rect.1 *= ui_scale;
                    hit.rect.2 *= ui_scale;
                    hit.rect.3 *= ui_scale;
                }
            }
        }

        surface.flush();
        drop(ctx);
        drop(surface);
        if let Some(profile) = render_profile
            && let Some(full) = crate::util::Rect::new(0, 0, phys_w as i32, phys_h as i32)
        {
            profile.remap_argb8888_regions(
                canvas,
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
                &[full],
            );
        }

        if let Some(layer) = self.layer_surface.as_ref() {
            let wl_surface = layer.wl_surface();
            wl_surface.set_buffer_scale(self.scale);
            if let Err(err) = buffer.attach_to(wl_surface) {
                return Err(anyhow!("failed to attach the toolbar buffer: {err}"));
            }
            wl_surface.damage_buffer(0, 0, phys_w as i32, phys_h as i32);
            wl_surface.commit();
        }

        self.dirty = false;
        self.render_failures = 0;
        Ok(())
    }
}
