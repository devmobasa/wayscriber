//! Clipboard completion, retained-buffer cleanup, and pin close transactions.

use std::sync::Arc;
use std::time::Instant;

use wayland_client::Proxy;

use super::{COPY_NOTICE_DURATION, PinHost};
use crate::pin::PinId;
use crate::pin::surface::CopyVisual;

impl PinHost {
    pub(crate) fn close_pin(&mut self, id: PinId) {
        let Some(mut pin) = self.pins.remove(&id) else {
            return;
        };
        self.discard_timing(id);
        if let Some(surface) = pin.shell.wl_surface.as_ref() {
            self.by_wl_surface.remove(&surface.id());
        }
        pin.cancel_interaction();
        pin.shell.destroy();
        if let Some(charge) = self.pin_charges.remove(&id) {
            let release = if self.clipboard.active_pin() == Some(id) {
                let retained = charge.retained_png_charge();
                if !self.clipboard.retain_charge(id, retained) {
                    log::error!("Pin {id} clipboard charge transfer failed");
                    self.should_exit = true;
                }
                charge.without_retained_png()
            } else {
                charge.combined()
            };
            if let Err(error) = release.and_then(|charge| self.memory.release(charge)) {
                log::error!("Pin {id} memory ledger release failed: {error}");
                self.should_exit = true;
            }
        }
        self.unlock_pointer_for(id);
        if self.pins.is_empty() {
            self.shutdown_armed = true;
        }
        log::debug!(
            "Pin {id} closed: resident={} peak={}",
            self.memory.resident_bytes(),
            self.memory.peak_bytes()
        );
        self.maybe_finish();
    }

    pub(super) fn begin_copy(&mut self, id: PinId) {
        if self.clipboard.is_active() {
            if let Some(pin) = self.pins.get_mut(&id) {
                pin.visual.copy = CopyVisual::Copying;
                pin.dirty = true;
            }
            return;
        }
        let Some(pin) = self.pins.get_mut(&id) else {
            return;
        };
        let Some(generation) = self.copy_generation.checked_add(1) else {
            pin.visual.copy = CopyVisual::Failed {
                until: Instant::now() + COPY_NOTICE_DURATION,
            };
            pin.dirty = true;
            return;
        };
        self.copy_generation = generation;
        match self
            .clipboard
            .start(id, generation, Arc::clone(&pin.model.image.png))
        {
            Ok(_) => pin.visual.copy = CopyVisual::Copying,
            Err(error) => {
                log::warn!("Pin clipboard worker failed to start: {error}");
                pin.visual.copy = CopyVisual::Failed {
                    until: Instant::now() + COPY_NOTICE_DURATION,
                };
            }
        }
        pin.dirty = true;
    }

    pub(crate) fn poll_workers(&mut self) {
        if let Some(completion) = self.clipboard.poll() {
            if let Some(charge) = completion.retained_charge
                && let Err(error) = self.memory.release(charge)
            {
                log::error!("Pin clipboard memory release failed: {error}");
                self.should_exit = true;
            }
            for pin in self.pins.values_mut() {
                if matches!(pin.visual.copy, CopyVisual::Copying) {
                    pin.visual.copy = CopyVisual::Idle;
                    pin.dirty = true;
                }
            }
            if let Some(pin) = self.pins.get_mut(&completion.pin_id)
                && completion.generation == self.copy_generation
            {
                pin.visual.copy = match completion.result {
                    Ok(()) => CopyVisual::Succeeded {
                        until: Instant::now() + COPY_NOTICE_DURATION,
                    },
                    Err(error) => {
                        log::warn!("Pin clipboard publication failed: {error}");
                        CopyVisual::Failed {
                            until: Instant::now() + COPY_NOTICE_DURATION,
                        }
                    }
                };
                pin.dirty = true;
            }
        }
        let now = Instant::now();
        for pin in self.pins.values_mut() {
            if matches!(pin.visual.copy, CopyVisual::Succeeded { until } | CopyVisual::Failed { until } if now >= until)
            {
                pin.visual.copy = CopyVisual::Idle;
                pin.dirty = true;
            }
        }
        let ids: Vec<_> = self.pins.keys().copied().collect();
        for id in ids {
            let released = self
                .pins
                .get_mut(&id)
                .map_or_else(Vec::new, |pin| pin.buffers.reap_released_indices());
            for index in released.into_iter().rev() {
                let charge = self.pin_charges.get_mut(&id).and_then(|charges| {
                    (index < charges.retired_surfaces.len())
                        .then(|| charges.retired_surfaces.remove(index))
                });
                match charge {
                    Some(charge) => {
                        if let Err(error) = self.memory.release(charge) {
                            log::error!("Pin {id} retired SHM release failed: {error}");
                            self.should_exit = true;
                        }
                    }
                    None => {
                        log::error!("Pin {id} retired SHM charge disappeared");
                        self.should_exit = true;
                    }
                }
            }
        }
        self.maybe_finish();
    }
}

#[cfg(test)]
mod tests {
    use super::super::PinCharge;
    use crate::pin::PinMemoryCharge;

    #[test]
    fn retired_surface_charges_remain_resident_until_release() {
        let image = PinMemoryCharge::from_parts(100, 200, 0, 0, 4);
        let current = PinMemoryCharge::from_parts(0, 0, 300, 600, 0);
        let retired = PinMemoryCharge::from_parts(0, 0, 150, 300, 0);
        let mut charge = PinCharge::new(image, current);
        charge.retired_surfaces.push(retired);
        assert_eq!(charge.combined().unwrap().total().unwrap(), 1_654);
    }

    #[test]
    fn repeated_replacement_with_blocked_releases_never_exceeds_the_ledger_cap() {
        let surface = PinMemoryCharge::from_parts(0, 0, 20, 20, 0);
        let mut ledger = crate::pin::PinMemoryLedger::with_limit(100);
        ledger.try_reserve(surface).unwrap();
        ledger.try_reserve(surface).unwrap();
        assert_eq!(ledger.resident_bytes(), 80);
        assert_eq!(
            ledger.try_reserve(surface),
            Err(crate::pin::PinRefusal::MemoryLimit)
        );
        assert_eq!(ledger.resident_bytes(), 80);
        assert_eq!(ledger.peak_bytes(), 80);
        ledger.release(surface).unwrap();
        ledger.try_reserve(surface).unwrap();
        assert_eq!(ledger.peak_bytes(), 80);
    }

    #[test]
    fn closing_during_copy_splits_only_the_retained_png() {
        let image = PinMemoryCharge::from_parts(100, 200, 0, 0, 4);
        let surface = PinMemoryCharge::from_parts(0, 0, 300, 600, 0);
        let charge = PinCharge::new(image, surface);
        assert_eq!(charge.retained_png_charge().total().unwrap(), 100);
        assert_eq!(
            charge.without_retained_png().unwrap().total().unwrap(),
            1_104
        );
    }
}
