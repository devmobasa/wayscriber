use anyhow::{Context, Result};
use smithay_client_toolkit::shell::WaylandSurface;

use super::AboutWindowState;

mod draw;
mod text;
mod widgets;

impl AboutWindowState {
    pub(super) fn render(&mut self) -> Result<()> {
        if !self.configured {
            return Ok(());
        }

        let (phys_w, phys_h) = (
            self.width.saturating_mul(self.scale as u32),
            self.height.saturating_mul(self.scale as u32),
        );

        if self.pool.is_none() {
            let buffer_size = (phys_w * phys_h * 4) as usize;
            let pool = super::SlotPool::new(buffer_size, &self.shm)
                .context("Failed to create about window buffer pool")?;
            self.pool = Some(pool);
        }

        let pool = match self.pool.as_mut() {
            Some(pool) => pool,
            None => return Ok(()),
        };
        let (buffer, canvas) = pool
            .create_buffer(
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
                wayland_client::protocol::wl_shm::Format::Argb8888,
            )
            .context("Failed to create about window buffer")?;

        let surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                canvas.as_mut_ptr(),
                cairo::Format::ARgb32,
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
            )
        }
        .context("Failed to create Cairo surface")?;
        let ctx = cairo::Context::new(&surface).context("Failed to create Cairo context")?;

        ctx.set_operator(cairo::Operator::Clear);
        let _ = ctx.paint();
        ctx.set_operator(cairo::Operator::Over);
        if self.scale > 1 {
            ctx.scale(self.scale as f64, self.scale as f64);
        }

        self.link_regions.clear();
        draw::draw_about(
            &ctx,
            self.width as f64,
            self.height as f64,
            &mut self.link_regions,
            self.hover_index,
        );

        surface.flush();

        let wl_surface = self.window.wl_surface();
        wl_surface.set_buffer_scale(self.scale);
        wl_surface.attach(Some(buffer.wl_buffer()), 0, 0);
        wl_surface.damage_buffer(0, 0, phys_w as i32, phys_h as i32);
        wl_surface.commit();

        self.needs_redraw = false;
        Ok(())
    }
}
