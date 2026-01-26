use std::time::Instant;

use anyhow::Result;
use log::{debug, warn};
use smithay_client_toolkit::{
    shell::WaylandSurface,
    shm::{Shm, slot::SlotPool},
};

use super::structs::ToolbarSurface;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::ui::toolbar::ToolbarSnapshot;

impl ToolbarSurface {
    /// Render helper used by the manager; keeps render impl closer to surface.
    pub fn render<F>(
        &mut self,
        shm: &Shm,
        snapshot: &ToolbarSnapshot,
        hover: Option<(f64, f64)>,
        hover_start: Option<Instant>,
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

        if self.pool.is_none() {
            let buffer_size = (phys_w * phys_h * 4) as usize;
            if let Ok(pool) = SlotPool::new(buffer_size, shm) {
                self.pool = Some(pool);
            } else {
                return Ok(());
            }
        }

        let pool = match self.pool.as_mut() {
            Some(p) => p,
            None => return Ok(()),
        };
        let (buffer, canvas) = match pool.create_buffer(
            phys_w as i32,
            phys_h as i32,
            (phys_w * 4) as i32,
            wayland_client::protocol::wl_shm::Format::Argb8888,
        ) {
            Ok(buf) => buf,
            Err(_) => return Ok(()),
        };

        let surface = match unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                canvas.as_mut_ptr(),
                cairo::Format::ARgb32,
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
            )
        } {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        let ctx = match cairo::Context::new(&surface) {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };

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

        if let Some(layer) = self.layer_surface.as_ref() {
            let wl_surface = layer.wl_surface();
            wl_surface.set_buffer_scale(self.scale);
            if let Err(err) = buffer.attach_to(wl_surface) {
                warn!(
                    "Failed to attach toolbar buffer for '{}': {}",
                    self.name, err
                );
                return Ok(());
            }
            wl_surface.damage_buffer(0, 0, phys_w as i32, phys_h as i32);
            wl_surface.commit();
        }

        self.dirty = false;
        Ok(())
    }
}
