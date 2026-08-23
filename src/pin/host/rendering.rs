//! SHM rendering and first-commit readiness.

use anyhow::{Context, Result};
use wayland_client::QueueHandle;

use super::PinHost;
use crate::pin::PinId;
use crate::pin::surface::{Damage, ShellEventIdentity};

impl PinHost {
    pub(crate) fn render_dirty(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        let ids: Vec<_> = self
            .pins
            .iter()
            .filter_map(|(id, pin)| {
                (pin.dirty && pin.shell.configured && pin.shell.frame_callback.is_none())
                    .then_some(*id)
            })
            .collect();
        for id in ids {
            self.render_pin(id, qh)?;
        }
        Ok(())
    }

    fn render_pin(&mut self, id: PinId, qh: &QueueHandle<Self>) -> Result<()> {
        let pin = self.pins.get_mut(&id).context("dirty pin disappeared")?;
        debug_assert_eq!(pin.model.id, id);
        let logical = pin
            .shell
            .configured_size
            .unwrap_or(pin.shell.requested_size);
        let scale = pin.shell.scale.max(1);
        let physical = (
            logical
                .0
                .checked_mul(scale as u32)
                .context("pin physical width overflow")?,
            logical
                .1
                .checked_mul(scale as u32)
                .context("pin physical height overflow")?,
        );
        if pin
            .raster
            .as_ref()
            .is_none_or(|cache| (cache.width, cache.height) != physical)
        {
            pin.raster = Some(crate::pin::surface::build_static_raster(
                &pin.model.image,
                physical,
                scale,
            )?);
            pin.full_damage = true;
        }
        let Some(acquired) = pin.buffers.acquire(&self.shm, physical.0, physical.1)? else {
            return Ok(());
        };
        let cache = pin
            .raster
            .as_ref()
            .context("pin raster cache disappeared")?;
        let damage = crate::pin::surface::render_frame(
            cache,
            acquired.canvas_ptr,
            acquired.canvas_len,
            pin.model.frame,
            scale,
            &pin.visual,
            pin.full_damage,
        )?;
        if !pin.buffers.is_current(acquired.pool_generation) {
            anyhow::bail!("pin buffer pool changed before attach");
        }
        let surface = pin
            .shell
            .wl_surface
            .as_ref()
            .context("configured pin lost wl_surface")?;
        acquired
            .buffer
            .attach_to(surface)
            .map_err(|error| anyhow::anyhow!("attach pin buffer: {error}"))?;
        match damage {
            Damage::Full => {
                surface.damage_buffer(0, 0, i32::try_from(physical.0)?, i32::try_from(physical.1)?)
            }
            Damage::Controls {
                x,
                y,
                width,
                height,
            } => surface.damage_buffer(x, y, i32::try_from(width)?, i32::try_from(height)?),
        }
        let token = self
            .next_frame_token
            .and_then(|token| token.checked_add(1))
            .context("pin frame callback identity exhausted")?;
        self.next_frame_token = Some(token);
        surface.frame(
            qh,
            ShellEventIdentity {
                pin_id: id,
                shell_generation: pin.shell.generation,
                token,
            },
        );
        pin.shell.frame_callback = Some(token);
        surface.commit();
        pin.dirty = false;
        pin.full_damage = false;
        if !pin.first_commit_complete {
            pin.first_commit_complete = true;
            if !pin.ready_sent {
                self.newly_ready.push(id);
            }
            self.note_first_commit(id);
        }
        Ok(())
    }

    pub(crate) fn take_newly_ready(&mut self) -> Vec<PinId> {
        std::mem::take(&mut self.newly_ready)
    }
}
