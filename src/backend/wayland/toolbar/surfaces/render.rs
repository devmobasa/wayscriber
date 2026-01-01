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
            if self.scale > 1 {
                ctx.scale(self.scale as f64, self.scale as f64);
            }
            render_fn(
                &ctx,
                self.width as f64,
                self.height as f64,
                snapshot,
                &mut self.hit_regions,
                hover,
            )?;
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
