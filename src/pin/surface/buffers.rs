//! A strictly bounded two-buffer SHM swapchain.

use anyhow::{Context, Result};
use smithay_client_toolkit::shm::{
    Shm,
    slot::{Buffer, Slot, SlotPool},
};
use wayland_client::protocol::wl_shm;

const SLOT_COUNT: usize = 2;

fn allocate_exact_slots<T, E>(mut allocate: impl FnMut() -> Result<T, E>) -> Result<Vec<T>, E> {
    let mut slots = Vec::with_capacity(SLOT_COUNT);
    for _ in 0..SLOT_COUNT {
        slots.push(allocate()?);
    }
    Ok(slots)
}

pub(crate) struct AcquiredPinBuffer {
    pub(crate) buffer: Buffer,
    pub(crate) canvas_ptr: usize,
    pub(crate) canvas_len: usize,
    pub(crate) pool_generation: u64,
}

pub(crate) struct PinBuffers {
    pool: Option<SlotPool>,
    slots: Vec<Slot>,
    dimensions: Option<(u32, u32, i32)>,
    generation: Option<u64>,
    pool_len: usize,
    retired: Vec<RetiredPool>,
}

struct RetiredPool {
    _pool: SlotPool,
    slots: Vec<Slot>,
}

impl Default for PinBuffers {
    fn default() -> Self {
        Self {
            pool: None,
            slots: Vec::new(),
            dimensions: None,
            generation: Some(0),
            pool_len: 0,
            retired: Vec::new(),
        }
    }
}

impl PinBuffers {
    pub(crate) fn clear(&mut self) -> Result<()> {
        self.clear_and_report_retention().map(|_| ())
    }

    /// Retire a current pool without deallocating compositor-owned slots.
    /// Returns true when the caller must retain the old surface ledger charge.
    pub(crate) fn clear_and_report_retention(&mut self) -> Result<bool> {
        let generation = self
            .generation
            .and_then(|generation| generation.checked_add(1))
            .context("pin SHM pool generation exhausted")?;
        let retained = self.slots.iter().any(Slot::has_active_buffers);
        if let Some(pool) = self.pool.take() {
            let slots = std::mem::take(&mut self.slots);
            if retained {
                self.retired.push(RetiredPool { _pool: pool, slots });
            }
        } else {
            self.slots.clear();
        }
        self.dimensions = None;
        self.pool_len = 0;
        self.generation = Some(generation);
        Ok(retained)
    }

    #[cfg(test)]
    pub(crate) fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Drop retired pools whose compositor releases have all arrived.
    pub(crate) fn reap_released_indices(&mut self) -> Vec<usize> {
        let released: Vec<_> = self
            .retired
            .iter()
            .enumerate()
            .filter_map(|(index, pool)| {
                (!pool.slots.iter().any(Slot::has_active_buffers)).then_some(index)
            })
            .collect();
        for index in released.iter().rev().copied() {
            self.retired.remove(index);
        }
        released
    }

    pub(crate) fn is_current(&self, generation: u64) -> bool {
        self.generation == Some(generation)
    }

    pub(crate) fn acquire(
        &mut self,
        shm: &Shm,
        width: u32,
        height: u32,
    ) -> Result<Option<AcquiredPinBuffer>> {
        let width_i32 = i32::try_from(width).context("pin width exceeds SHM limits")?;
        let height_i32 = i32::try_from(height).context("pin height exceeds SHM limits")?;
        let stride = width_i32
            .checked_mul(4)
            .context("pin SHM stride overflow")?;
        let slot_len = usize::try_from(height_i32)
            .ok()
            .and_then(|height| height.checked_mul(usize::try_from(stride).ok()?))
            .map(|len| len.next_multiple_of(64))
            .context("pin SHM slot length overflow")?;

        let dimensions = (width, height, stride);
        if self.dimensions != Some(dimensions) {
            if self.pool.is_some() {
                anyhow::bail!("pin buffer dimensions changed without an admitted replacement");
            }
            // Pool creation is all-or-nothing. Replacements are retired above,
            // retaining their pool and ledger charge until every compositor-owned
            // buffer is released; new drawing can never alias their slots.
            let initial_len = slot_len
                .checked_mul(SLOT_COUNT)
                .context("pin SHM pool length overflow")?;
            let mut pool = SlotPool::new(initial_len, shm).context("create pin SHM pool")?;
            let slots =
                allocate_exact_slots(|| pool.new_slot(slot_len).context("allocate pin SHM slot"))?;
            self.pool_len = pool.len();
            self.slots = slots;
            self.pool = Some(pool);
            self.dimensions = Some(dimensions);
        }

        let Some(slot_index) = self
            .slots
            .iter()
            .position(|slot| !slot.has_active_buffers())
        else {
            return Ok(None);
        };
        let pool = self.pool.as_mut().context("pin SHM pool disappeared")?;
        let slot = &self.slots[slot_index];
        let buffer = pool
            .create_buffer_in(
                slot,
                width_i32,
                height_i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .context("create pin SHM buffer")?;
        debug_assert_eq!(self.slots.len(), SLOT_COUNT);
        debug_assert_eq!(pool.len(), self.pool_len);
        let canvas = pool.raw_data_mut(slot);
        Ok(Some(AcquiredPinBuffer {
            buffer,
            canvas_ptr: canvas.as_mut_ptr() as usize,
            canvas_len: slot_len,
            pool_generation: self
                .generation
                .context("pin SHM pool generation exhausted")?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_count_is_a_fixed_contract() {
        assert_eq!(SLOT_COUNT, 2);
        assert_eq!(PinBuffers::default().slot_count(), 0);
    }

    #[test]
    fn partial_slot_allocation_is_never_published() {
        let mut attempt = 0;
        let result = allocate_exact_slots(|| {
            attempt += 1;
            (attempt == 1)
                .then_some(attempt)
                .ok_or("second slot failed")
        });
        assert_eq!(result, Err("second slot failed"));
        assert_eq!(attempt, 2);
    }
}
