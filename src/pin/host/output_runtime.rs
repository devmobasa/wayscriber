//! Output hotplug, configure, and scale transactions with memory admission.

use anyhow::{Context, Result};
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::{Proxy, QueueHandle, protocol::wl_output};

use super::PinHost;
use crate::pin::{PinFrame, PinId, PinMemoryCharge, PinRefusal, geometry};

impl PinHost {
    pub(super) fn refresh_output(&mut self, output: wl_output::WlOutput, qh: &QueueHandle<Self>) {
        let previous = self.outputs.by_proxy(&output).cloned();
        let Some(info) = self.output_state.info(&output) else {
            return;
        };
        let Some(current) = self.outputs.update(output.clone(), &info).cloned() else {
            return;
        };

        let ids: Vec<_> = self
            .pins
            .iter()
            .filter_map(|(id, pin)| {
                (pin.model.output == current.connector_name || pin.shell.layer_surface.is_none())
                    .then_some(*id)
            })
            .collect();
        for id in ids {
            let (old_size, image_size, frame, dormant) = {
                let pin = &self.pins[&id];
                (
                    pin.model.output_size,
                    (pin.model.image.width, pin.model.image.height),
                    pin.model.frame,
                    pin.shell.layer_surface.is_none(),
                )
            };
            let unchanged = previous.as_ref().is_some_and(|old| {
                old.logical_size == current.logical_size
                    && old.scale == current.scale
                    && old.transform == current.transform
            });
            if !dormant && unchanged {
                continue;
            }
            let next = match super::output::migrate_frame(
                frame,
                image_size,
                old_size,
                current.logical_size,
                current.scale.max(1) as u32,
            ) {
                Ok(frame) => frame,
                Err(error) => {
                    log::warn!("Pin {id} could not migrate after output update: {error}");
                    continue;
                }
            };
            if let Err(error) = self.recreate_pin_on_output(id, &current, next, qh) {
                log::error!("Pin {id} output recreation failed: {error:#}");
                self.close_pin(id);
            }
        }
    }

    pub(super) fn remove_output(&mut self, output: wl_output::WlOutput, qh: &QueueHandle<Self>) {
        let Some(removed) = self.outputs.remove(&output) else {
            return;
        };
        let target = self.outputs.deterministic_first().cloned();
        let ids: Vec<_> = self
            .pins
            .iter()
            .filter_map(|(id, pin)| (pin.model.output == removed.connector_name).then_some(*id))
            .collect();
        for id in ids {
            if let Some(pin) = self.pins.get_mut(&id) {
                pin.cancel_interaction();
            }
            self.unlock_pointer_for(id);
            let Some(target) = target.as_ref() else {
                if let Err(error) = self.make_dormant(id) {
                    log::error!("Pin {id} could not become dormant: {error:#}");
                    self.close_pin(id);
                }
                continue;
            };
            let pin = &self.pins[&id];
            let next = super::output::migrate_frame(
                pin.model.frame,
                (pin.model.image.width, pin.model.image.height),
                removed.logical_size,
                target.logical_size,
                target.scale.max(1) as u32,
            );
            match next.and_then(|frame| {
                self.recreate_pin_on_output(id, target, frame, qh)
                    .map_err(|_| PinRefusal::LimitExceeded)
            }) {
                Ok(()) => {}
                Err(error) => {
                    log::error!("Pin {id} output migration failed: {error}");
                    self.close_pin(id);
                }
            }
        }
    }

    fn recreate_pin_on_output(
        &mut self,
        id: PinId,
        output: &super::output::HostOutput,
        frame: PinFrame,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        self.replace_surface_allocation(id, frame, output.scale.max(1))?;
        self.remove_proxy_routes(id);
        {
            let pin = self
                .pins
                .get_mut(&id)
                .context("pin vanished during migration")?;
            pin.replace_shell()
                .map_err(|_| anyhow::anyhow!("pin shell generation exhausted"))?;
            pin.model.output = output.connector_name.clone();
            pin.model.output_size = output.logical_size;
            pin.model.frame = frame;
            pin.shell.scale = output.scale.max(1);
        }
        self.create_shell(id, &output.proxy, output.scale, qh)
    }

    fn make_dormant(&mut self, id: PinId) -> Result<()> {
        self.drop_surface_allocation(id)?;
        self.remove_proxy_routes(id);
        let pin = self
            .pins
            .get_mut(&id)
            .context("pin vanished before Dormant")?;
        pin.replace_shell()
            .map_err(|_| anyhow::anyhow!("pin shell generation exhausted"))?;
        Ok(())
    }

    fn drop_surface_allocation(&mut self, id: PinId) -> Result<()> {
        let retained = if let Some(pin) = self.pins.get_mut(&id) {
            pin.buffers.clear_and_report_retention()?
        } else {
            false
        };
        let charges = self
            .pin_charges
            .get_mut(&id)
            .context("pin allocation charge vanished")?;
        let old = std::mem::take(&mut charges.surface);
        if retained {
            charges.retired_surfaces.push(old);
        } else {
            self.memory
                .release(old)
                .map_err(|error| anyhow::anyhow!(error))?;
        }
        if let Some(pin) = self.pins.get_mut(&id) {
            pin.raster = None;
        }
        Ok(())
    }

    pub(super) fn remove_proxy_routes(&mut self, id: PinId) {
        let Some(pin) = self.pins.get(&id) else {
            return;
        };
        if let Some(surface) = pin.shell.wl_surface.as_ref() {
            self.by_wl_surface.remove(&surface.id());
        }
    }

    pub(super) fn accept_configure(
        &mut self,
        id: PinId,
        configured: (u32, u32),
        qh: &QueueHandle<Self>,
    ) -> Result<bool> {
        let pin = self.pins.get(&id).context("configured pin vanished")?;
        let requested = pin.shell.requested_size;
        let selected_surface = (
            if configured.0 == 0 {
                requested.0
            } else {
                configured.0
            },
            if configured.1 == 0 {
                requested.1
            } else {
                configured.1
            },
        );
        let chrome = crate::pin::surface::CHROME_PADDING.saturating_mul(2);
        let selected = (
            selected_surface.0.saturating_sub(chrome).max(1),
            selected_surface.1.saturating_sub(chrome).max(1),
        );
        let factor = f64::from(selected.0) / f64::from(pin.model.frame.width);
        let steps = factor.log2() * 10.0;
        let resized = geometry::resized_frame(
            pin.model.frame,
            (
                f64::from(pin.model.frame.width) / 2.0,
                f64::from(pin.model.frame.height) / 2.0,
            ),
            steps,
            (pin.model.image.width, pin.model.image.height),
            pin.model.output_size,
            pin.shell.scale.max(1) as u32,
        )
        .map_err(|error| anyhow::anyhow!(error))?;
        // A compositor-selected size is not a pointer-anchored resize. Keep
        // the already committed margins and only accept the aspect/limit-
        // clamped dimensions.
        let clamped = PinFrame::new(
            pin.model.frame.x,
            pin.model.frame.y,
            resized.width,
            resized.height,
        )
        .context("configured pin dimensions became empty")?;
        let limited = super::output::fit_frame_to_surface_limit(
            clamped,
            (pin.model.image.width, pin.model.image.height),
            pin.model.output_size,
            pin.shell.scale.max(1) as u32,
        )?;
        let clamped = PinFrame::new(
            pin.model.frame.x,
            pin.model.frame.y,
            limited.width,
            limited.height,
        )
        .context("configured pin limit dimensions became empty")?;
        if (clamped.width, clamped.height) != selected {
            let layer = pin
                .shell
                .layer_surface
                .as_ref()
                .context("configured pin lost layer role")?;
            let size = crate::pin::surface::surface_size(clamped)
                .context("configured pin chrome size overflow")?;
            layer.set_size(size.0, size.1);
            layer.wl_surface().commit();
            let pin = self.pins.get_mut(&id).context("configured pin vanished")?;
            pin.shell.requested_size = size;
            pin.shell.configured = false;
            return Ok(false);
        }
        let scale = pin.shell.scale;
        self.replace_surface_allocation(id, clamped, scale)?;
        let pin = self.pins.get_mut(&id).context("configured pin vanished")?;
        pin.model.frame = clamped;
        pin.shell.configured_size = Some(selected_surface);
        pin.shell.configured = true;
        pin.full_damage = true;
        pin.dirty = true;
        self.note_configured(id);
        let _ = qh;
        Ok(true)
    }

    pub(super) fn change_scale(&mut self, id: PinId, scale: i32) -> Result<()> {
        let pin = self.pins.get(&id).context("scaled pin vanished")?;
        let frame = super::output::fit_frame_to_surface_limit(
            pin.model.frame,
            (pin.model.image.width, pin.model.image.height),
            pin.model.output_size,
            scale.max(1) as u32,
        )?;
        self.replace_surface_allocation(id, frame, scale.max(1))?;
        let pin = self.pins.get_mut(&id).context("scaled pin vanished")?;
        let changed = pin.model.frame != frame;
        pin.model.frame = frame;
        pin.shell.scale = scale.max(1);
        if let Some(surface) = pin.shell.wl_surface.as_ref() {
            surface.set_buffer_scale(scale.max(1));
        }
        if changed {
            let size = crate::pin::surface::surface_size(frame)
                .context("scaled pin chrome size overflow")?;
            let origin = crate::pin::surface::surface_origin(frame);
            if let Some(layer) = pin.shell.layer_surface.as_ref() {
                layer.set_margin(origin.1, 0, 0, origin.0);
                layer.set_size(size.0, size.1);
                layer.wl_surface().commit();
            }
            pin.shell.requested_size = size;
            pin.shell.configured = false;
            pin.shell.pending_origin = Some((frame.x, frame.y));
        }
        pin.full_damage = true;
        pin.dirty = true;
        Ok(())
    }

    fn replace_surface_allocation(&mut self, id: PinId, frame: PinFrame, scale: i32) -> Result<()> {
        let size = crate::pin::surface::surface_size(frame).context("pin chrome size overflow")?;
        let replacement = PinMemoryCharge::for_surface(size.0, size.1, scale.max(1) as u32)
            .map_err(|error| anyhow::anyhow!(error))?;
        let current = self
            .pin_charges
            .get(&id)
            .context("pin allocation charge vanished")?
            .surface;
        if replacement == current {
            return Ok(());
        }
        self.memory
            .try_reserve(replacement)
            .map_err(|error| anyhow::anyhow!(error))?;
        let retained = if let Some(pin) = self.pins.get_mut(&id) {
            match pin.buffers.clear_and_report_retention() {
                Ok(retained) => retained,
                Err(error) => {
                    self.memory
                        .release(replacement)
                        .map_err(|error| anyhow::anyhow!(error))?;
                    return Err(error);
                }
            }
        } else {
            false
        };
        let charges = self
            .pin_charges
            .get_mut(&id)
            .context("pin allocation charge vanished")?;
        charges.surface = replacement;
        if retained {
            charges.retired_surfaces.push(current);
        } else {
            self.memory
                .release(current)
                .map_err(|error| anyhow::anyhow!(error))?;
        }
        if let Some(pin) = self.pins.get_mut(&id) {
            pin.raster = None;
        }
        log::debug!(
            "Pin {id} surface allocation replaced: resident={} peak={}",
            self.memory.resident_bytes(),
            self.memory.peak_bytes()
        );
        Ok(())
    }
}
