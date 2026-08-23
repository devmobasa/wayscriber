use std::fmt;

use super::PinRefusal;

pub(crate) const MAX_PINS: usize = 8;
pub(crate) const MAX_PIN_PNG_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_PIN_SOURCE_PIXELS: u64 = 48_000_000;
pub(crate) const MAX_PIN_DIMENSION: u32 = 16_384;
pub(crate) const MAX_PIN_SURFACE_PIXELS: u64 = 16_000_000;
pub(crate) const MAX_TOTAL_PIN_RESIDENT_BYTES: u64 = 512 * 1024 * 1024;
pub(crate) const PIN_METADATA_BYTES: u64 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PinMemoryCharge {
    pub retained_png: u64,
    pub decoded_source: u64,
    pub scaled_raster: u64,
    pub shm_slots: u64,
    pub metadata: u64,
}

impl PinMemoryCharge {
    pub(crate) const fn from_parts(
        retained_png: u64,
        decoded_source: u64,
        scaled_raster: u64,
        shm_slots: u64,
        metadata: u64,
    ) -> Self {
        Self {
            retained_png,
            decoded_source,
            scaled_raster,
            shm_slots,
            metadata,
        }
    }

    pub(crate) fn for_image(png_len: usize, width: u32, height: u32) -> Result<Self, PinRefusal> {
        validate_source(png_len, width, height)?;
        let decoded_source = aligned_argb_len(width, height)?;
        Ok(Self::from_parts(
            u64::try_from(png_len).map_err(|_| PinRefusal::LimitExceeded)?,
            decoded_source,
            0,
            0,
            PIN_METADATA_BYTES,
        ))
    }

    pub(crate) fn for_surface(width: u32, height: u32, scale: u32) -> Result<Self, PinRefusal> {
        let physical_width = width.checked_mul(scale).ok_or(PinRefusal::LimitExceeded)?;
        let physical_height = height.checked_mul(scale).ok_or(PinRefusal::LimitExceeded)?;
        validate_surface(physical_width, physical_height)?;
        let raster = aligned_argb_len(physical_width, physical_height)?;
        let slot = raster
            .checked_add(63)
            .map(|length| length & !63)
            .ok_or(PinRefusal::LimitExceeded)?;
        Ok(Self::from_parts(
            0,
            0,
            raster,
            slot.checked_mul(2).ok_or(PinRefusal::LimitExceeded)?,
            0,
        ))
    }

    pub(crate) fn checked_combined(self, other: Self) -> Result<Self, PinRefusal> {
        Ok(Self {
            retained_png: checked_add(self.retained_png, other.retained_png)?,
            decoded_source: checked_add(self.decoded_source, other.decoded_source)?,
            scaled_raster: checked_add(self.scaled_raster, other.scaled_raster)?,
            shm_slots: checked_add(self.shm_slots, other.shm_slots)?,
            metadata: checked_add(self.metadata, other.metadata)?,
        })
    }

    pub(crate) fn total(self) -> Result<u64, PinRefusal> {
        [
            self.retained_png,
            self.decoded_source,
            self.scaled_raster,
            self.shm_slots,
            self.metadata,
        ]
        .into_iter()
        .try_fold(0_u64, checked_add)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PinMemoryLedger {
    resident_bytes: u64,
    peak_bytes: u64,
    limit: u64,
}

impl Default for PinMemoryLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl PinMemoryLedger {
    pub(crate) const fn new() -> Self {
        Self::with_limit(MAX_TOTAL_PIN_RESIDENT_BYTES)
    }

    pub(crate) const fn with_limit(limit: u64) -> Self {
        Self {
            resident_bytes: 0,
            peak_bytes: 0,
            limit,
        }
    }

    pub(crate) fn try_reserve(&mut self, charge: PinMemoryCharge) -> Result<(), PinRefusal> {
        let amount = charge.total()?;
        let next = self
            .resident_bytes
            .checked_add(amount)
            .ok_or(PinRefusal::LimitExceeded)?;
        if next > self.limit {
            log::debug!(
                "Pin ledger refused reserve: bytes={amount} resident={} peak={} limit={}",
                self.resident_bytes,
                self.peak_bytes,
                self.limit
            );
            return Err(PinRefusal::MemoryLimit);
        }
        self.resident_bytes = next;
        self.peak_bytes = self.peak_bytes.max(next);
        log::debug!(
            "Pin ledger reserved: bytes={amount} resident={} peak={} limit={}",
            self.resident_bytes,
            self.peak_bytes,
            self.limit
        );
        Ok(())
    }

    pub(crate) fn release(&mut self, charge: PinMemoryCharge) -> Result<(), PinRefusal> {
        let amount = charge.total()?;
        self.resident_bytes = self
            .resident_bytes
            .checked_sub(amount)
            .ok_or(PinRefusal::LimitExceeded)?;
        log::debug!(
            "Pin ledger released: bytes={amount} resident={} peak={} limit={}",
            self.resident_bytes,
            self.peak_bytes,
            self.limit
        );
        Ok(())
    }

    pub(crate) const fn resident_bytes(&self) -> u64 {
        self.resident_bytes
    }

    pub(crate) const fn peak_bytes(&self) -> u64 {
        self.peak_bytes
    }
}

pub(crate) fn validate_source(png_len: usize, width: u32, height: u32) -> Result<(), PinRefusal> {
    if png_len == 0 || png_len > MAX_PIN_PNG_BYTES {
        return Err(PinRefusal::InvalidImage);
    }
    validate_source_dimensions(width, height)
}

pub(crate) fn validate_source_dimensions(width: u32, height: u32) -> Result<(), PinRefusal> {
    if width == 0 || height == 0 {
        return Err(PinRefusal::InvalidImage);
    }
    if width > MAX_PIN_DIMENSION || height > MAX_PIN_DIMENSION {
        return Err(PinRefusal::LimitExceeded);
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(PinRefusal::LimitExceeded)?;
    if pixels > MAX_PIN_SOURCE_PIXELS {
        return Err(PinRefusal::LimitExceeded);
    }
    Ok(())
}

pub(crate) fn validate_surface(width: u32, height: u32) -> Result<(), PinRefusal> {
    if width == 0 || height == 0 {
        return Err(PinRefusal::InvalidPlacement);
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(PinRefusal::LimitExceeded)?;
    if pixels > MAX_PIN_SURFACE_PIXELS {
        return Err(PinRefusal::LimitExceeded);
    }
    Ok(())
}

fn aligned_argb_len(width: u32, height: u32) -> Result<u64, PinRefusal> {
    let stride = u64::from(width)
        .checked_mul(4)
        .and_then(|stride| stride.checked_add(3))
        .map(|stride| stride & !3)
        .ok_or(PinRefusal::LimitExceeded)?;
    stride
        .checked_mul(u64::from(height))
        .ok_or(PinRefusal::LimitExceeded)
}

fn checked_add(left: u64, right: u64) -> Result<u64, PinRefusal> {
    left.checked_add(right).ok_or(PinRefusal::LimitExceeded)
}

impl fmt::Display for PinMemoryCharge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.total() {
            Ok(total) => write!(formatter, "{total} bytes"),
            Err(_) => formatter.write_str("overflowed pin memory charge"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_and_surface_multiplication_is_checked_before_allocation() {
        assert_eq!(
            validate_source(1, MAX_PIN_DIMENSION + 1, 1),
            Err(PinRefusal::LimitExceeded)
        );
        assert_eq!(
            PinMemoryCharge::for_surface(u32::MAX, u32::MAX, 2),
            Err(PinRefusal::LimitExceeded)
        );
    }

    #[test]
    fn surface_charge_contains_raster_and_exactly_two_slots() {
        let charge = PinMemoryCharge::for_surface(100, 50, 2).unwrap();
        assert_eq!(charge.scaled_raster, 80_000);
        assert_eq!(charge.shm_slots, 160_000);
    }

    #[test]
    fn surface_slots_charge_the_same_sixty_four_byte_alignment_as_shm() {
        let charge = PinMemoryCharge::for_surface(101, 1, 1).unwrap();
        assert_eq!(charge.scaled_raster, 404);
        assert_eq!(charge.shm_slots, 896);
    }

    #[test]
    fn ledger_reserves_new_memory_before_old_memory_is_released() {
        let old = PinMemoryCharge::from_parts(20, 0, 0, 0, 0);
        let replacement = PinMemoryCharge::from_parts(90, 0, 0, 0, 0);
        let mut ledger = PinMemoryLedger::with_limit(100);
        ledger.try_reserve(old).unwrap();

        assert_eq!(
            ledger.try_reserve(replacement),
            Err(PinRefusal::MemoryLimit)
        );
        assert_eq!(ledger.resident_bytes(), 20);
        ledger.release(old).unwrap();
        ledger.try_reserve(replacement).unwrap();
        assert_eq!(ledger.peak_bytes(), 90);
    }

    #[test]
    fn ledger_rejects_double_release_instead_of_hiding_it() {
        let charge = PinMemoryCharge::from_parts(20, 0, 0, 0, 0);
        let mut ledger = PinMemoryLedger::with_limit(100);
        ledger.try_reserve(charge).unwrap();
        ledger.release(charge).unwrap();
        assert_eq!(ledger.release(charge), Err(PinRefusal::LimitExceeded));
    }

    #[test]
    fn combined_charge_reports_addition_overflow() {
        let huge = PinMemoryCharge::from_parts(u64::MAX, 0, 0, 0, 0);
        let one = PinMemoryCharge::from_parts(1, 0, 0, 0, 0);
        assert_eq!(huge.checked_combined(one), Err(PinRefusal::LimitExceeded));
    }
}
