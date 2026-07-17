//! Per-buffer damage tracking for correct incremental rendering.
//!
//! With multi-buffering, each buffer may be reused after the compositor releases it.
//! When buffer B is rendered, we must report ALL regions that changed since B was
//! last displayed - not just regions from the current frame.
//!
//! This module tracks damage per buffer slot, identified by the canvas memory address.
//! SlotPool reuses the same memory regions for released buffers, so the pointer
//! serves as a stable slot identifier across buffer reuse.
//!
//! A pool generation counter handles pool recreation (due to resize, scale change,
//! or pool growth). When the generation changes, all tracking is reset since
//! previous canvas pointers become invalid.

use std::collections::HashMap;
use std::fmt;

use crate::util::Rect;

/// Maximum number of buffer slots to track (prevents unbounded growth).
const MAX_TRACKED_BUFFERS: usize = 8;
const MAX_DAMAGE_REGIONS_BEFORE_FULL: usize = 256;
const DAMAGE_MERGE_MARGIN: i32 = 1;
const DAMAGE_MERGE_ALIGNMENT_TOLERANCE: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::backend::wayland) enum FullDamageReason {
    InitialFrame,
    NewBufferSlot,
    PoolGenerationChanged,
    PoolGrew,
    ScaleChanged,
    SurfaceResized,
    OutputChanged,
    LayerSurfaceRecreated,
    OverlaySuppression,
    OverlayRestored,
    Zoom,
    BoardPan,
    CanvasClear,
    FirstRunOnboarding,
    EmptyDamageFallback,
    DamageRegionsCoverSurface,
    DamageRegionLimit,
    Unknown,
}

impl FullDamageReason {
    pub(in crate::backend::wayland) fn as_str(self) -> &'static str {
        match self {
            Self::InitialFrame => "initial_frame",
            Self::NewBufferSlot => "new_buffer_slot",
            Self::PoolGenerationChanged => "pool_generation_changed",
            Self::PoolGrew => "pool_grew",
            Self::ScaleChanged => "scale_changed",
            Self::SurfaceResized => "surface_resized",
            Self::OutputChanged => "output_changed",
            Self::LayerSurfaceRecreated => "layer_surface_recreated",
            Self::OverlaySuppression => "overlay_suppression",
            Self::OverlayRestored => "overlay_restored",
            Self::Zoom => "zoom",
            Self::BoardPan => "board_pan",
            Self::CanvasClear => "canvas_clear",
            Self::FirstRunOnboarding => "first_run_onboarding",
            Self::EmptyDamageFallback => "empty_damage_fallback",
            Self::DamageRegionsCoverSurface => "damage_regions_cover_surface",
            Self::DamageRegionLimit => "damage_region_limit",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for FullDamageReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Damage state for a single buffer slot.
#[derive(Debug, Default)]
struct BufferDamage {
    /// Dirty regions accumulated since this buffer was last rendered.
    regions: Vec<Rect>,
    /// Force full damage on next render (e.g., new buffer or after resize).
    force_full: bool,
    /// Reason the next render for this slot must use full damage.
    force_full_reason: Option<FullDamageReason>,
    /// Frame counter for LRU eviction.
    last_used_frame: u64,
}

pub(in crate::backend::wayland) struct BufferDamageReport {
    pub(in crate::backend::wayland) regions: Vec<Rect>,
    pub(in crate::backend::wayland) full_reason: Option<FullDamageReason>,
    pub(in crate::backend::wayland) regions_before_merge: usize,
    pub(in crate::backend::wayland) regions_after_merge: usize,
}

/// Tracks dirty regions for each buffer slot independently.
///
/// Buffers are identified by their canvas memory address, which is stable
/// across SlotPool buffer reuse (the same memory slot gets the same pointer).
/// Pool identity (generation + size) detects when the pool is recreated or
/// grows, which invalidates all previous canvas pointers.
///
/// When new damage occurs, it's added to all tracked buffers.
/// When a buffer renders, only its damage is drained.
pub struct BufferDamageTracker {
    /// Damage state per canvas memory address (used as slot identifier).
    buffers: HashMap<usize, BufferDamage>,
    /// Global frame counter for LRU eviction.
    frame_counter: u64,
    /// When true, all buffers (including new ones) get full damage.
    global_full_damage: bool,
    /// Reason all buffers are forced to full damage.
    global_full_reason: Option<FullDamageReason>,
    /// Pool generation - when this changes, all tracking is invalidated.
    pool_generation: u64,
    /// Pool size - when this increases, pool grew and tracking is invalidated.
    pool_size: usize,
}

impl BufferDamageTracker {
    /// Creates a new tracker.
    pub fn new(_buffer_count: usize) -> Self {
        Self {
            buffers: HashMap::with_capacity(4),
            frame_counter: 0,
            global_full_damage: true, // First frame always needs full damage
            global_full_reason: Some(FullDamageReason::InitialFrame),
            pool_generation: 0,
            pool_size: 0,
        }
    }

    /// Checks pool identity and resets tracking if pool was recreated or grew.
    ///
    /// When the SlotPool is recreated (generation changes) or grows (size increases),
    /// all previous canvas pointers become invalid due to memory remapping.
    fn check_pool_identity(&mut self, generation: u64, pool_size: usize) {
        let pool_changed = generation != self.pool_generation;
        let pool_grew = pool_size > self.pool_size;

        if pool_changed || pool_grew {
            let first_pool_identity = self.pool_generation == 0
                && self.pool_size == 0
                && self.buffers.is_empty()
                && self.global_full_damage;
            let reason = if first_pool_identity {
                self.global_full_reason
                    .unwrap_or(FullDamageReason::InitialFrame)
            } else if pool_changed {
                FullDamageReason::PoolGenerationChanged
            } else {
                FullDamageReason::PoolGrew
            };
            if pool_changed {
                log::debug!(
                    "Pool generation changed {} -> {}, resetting damage tracking",
                    self.pool_generation,
                    generation
                );
            } else {
                log::debug!(
                    "Pool grew {} -> {} bytes, resetting damage tracking",
                    self.pool_size,
                    pool_size
                );
            }
            self.pool_generation = generation;
            self.pool_size = pool_size;
            self.buffers.clear();
            self.global_full_damage = true;
            self.global_full_reason = Some(reason);
        } else if pool_size != self.pool_size {
            // Pool shrunk (shouldn't happen normally, but track it)
            self.pool_size = pool_size;
        }
    }

    /// Marks a region as dirty across all tracked buffers.
    pub fn mark_rect(&mut self, rect: Rect) {
        if !rect.is_valid() {
            return;
        }
        for damage in self.buffers.values_mut() {
            if !damage.force_full {
                damage.regions.push(rect);
            }
        }
    }

    /// Marks the entire surface as dirty for all buffers (including future new ones).
    pub fn mark_all_full(&mut self, reason: FullDamageReason) {
        self.global_full_damage = true;
        self.global_full_reason = Some(reason);
        for damage in self.buffers.values_mut() {
            damage.force_full = true;
            damage.force_full_reason = Some(reason);
            damage.regions.clear();
        }
    }

    /// Adds dirty regions from the input state to all tracked buffers.
    pub fn add_regions(&mut self, regions: Vec<Rect>) {
        for rect in regions {
            self.mark_rect(rect);
        }
    }

    /// Takes damage regions for the buffer identified by its canvas pointer.
    ///
    /// The canvas pointer (memory address of the first byte) serves as a stable
    /// slot identifier - SlotPool reuses the same memory for released buffers.
    ///
    /// Pool identity (generation + size) is checked first - if the pool was
    /// recreated or grew, all tracking is reset since previous pointers are invalid.
    ///
    /// If this is a new slot (or global_full_damage is set), returns full damage.
    /// Otherwise returns and clears the slot's accumulated damage.
    pub fn take_buffer_damage_report(
        &mut self,
        canvas_ptr: usize,
        width: i32,
        height: i32,
        pool_generation: u64,
        pool_size: usize,
    ) -> BufferDamageReport {
        // Check pool identity first - resets tracking if pool changed
        self.check_pool_identity(pool_generation, pool_size);
        self.frame_counter += 1;
        let frame = self.frame_counter;

        // Check if this slot is already tracked BEFORE eviction
        let is_existing = self.buffers.contains_key(&canvas_ptr);

        // Only evict if we need space for a new slot
        if !is_existing {
            self.evict_if_needed();
        }

        let global_full = self.global_full_damage;
        let global_reason = self.global_full_reason;
        let (needs_full, full_reason, mut regions) = {
            let damage = self
                .buffers
                .entry(canvas_ptr)
                .or_insert_with(|| BufferDamage {
                    regions: Vec::new(),
                    force_full: true, // New slot always needs full damage
                    force_full_reason: Some(FullDamageReason::NewBufferSlot),
                    last_used_frame: frame,
                });

            damage.last_used_frame = frame;

            // Check if we need full damage
            let needs_full = damage.force_full || global_full;
            let full_reason = if global_full {
                global_reason
            } else if damage.force_full {
                damage.force_full_reason
            } else {
                None
            };

            if needs_full {
                damage.force_full = false;
                damage.force_full_reason = None;
                damage.regions.clear();
                (true, full_reason, Vec::new())
            } else {
                // Take and deduplicate regions
                let mut regions: Vec<Rect> = damage.regions.drain(..).collect();
                regions.retain(Rect::is_valid);
                (false, None, regions)
            }
        };

        if needs_full {
            // Clear global flag only after a buffer has received it
            if global_full {
                self.global_full_damage = false;
                self.global_full_reason = None;
            }

            if width > 0
                && height > 0
                && let Some(full) = Rect::new(0, 0, width, height)
            {
                return BufferDamageReport {
                    regions: vec![full],
                    full_reason: Some(full_reason.unwrap_or(FullDamageReason::Unknown)),
                    regions_before_merge: 0,
                    regions_after_merge: 0,
                };
            }
            return BufferDamageReport {
                regions: Vec::new(),
                full_reason,
                regions_before_merge: 0,
                regions_after_merge: 0,
            };
        }

        let regions_before_merge = regions.len();
        if regions_before_merge > MAX_DAMAGE_REGIONS_BEFORE_FULL
            && width > 0
            && height > 0
            && let Some(full) = Rect::new(0, 0, width, height)
        {
            return BufferDamageReport {
                regions: vec![full],
                full_reason: Some(FullDamageReason::DamageRegionLimit),
                regions_before_merge,
                regions_after_merge: 1,
            };
        }
        // Merge overlapping regions to reduce compositor work
        merge_damage_regions(&mut regions);
        let regions_after_merge = regions.len();

        BufferDamageReport {
            regions,
            full_reason: None,
            regions_before_merge,
            regions_after_merge,
        }
    }

    #[cfg(test)]
    fn take_buffer_damage(
        &mut self,
        canvas_ptr: usize,
        width: i32,
        height: i32,
        pool_generation: u64,
        pool_size: usize,
    ) -> Vec<Rect> {
        self.take_buffer_damage_report(canvas_ptr, width, height, pool_generation, pool_size)
            .regions
    }

    /// Evicts oldest buffer if we're at capacity.
    fn evict_if_needed(&mut self) {
        if self.buffers.len() >= MAX_TRACKED_BUFFERS {
            // Find the buffer with the oldest last_used_frame
            let oldest = self
                .buffers
                .iter()
                .min_by_key(|(_, d)| d.last_used_frame)
                .map(|(id, _)| *id);

            if let Some(id) = oldest {
                self.buffers.remove(&id);
            }
        }
    }

    /// Returns true if any buffer has pending damage.
    #[allow(dead_code)]
    pub fn has_pending_damage(&self) -> bool {
        self.global_full_damage
            || self
                .buffers
                .values()
                .any(|d| d.force_full || !d.regions.is_empty())
    }
}

/// Merges aligned overlapping or adjacent damage regions to reduce compositor work.
fn merge_damage_regions(regions: &mut Vec<Rect>) {
    if regions.len() <= 1 {
        return;
    }

    // Keep sparse diagonal/path damage split. Merging those into bounding boxes can turn an
    // ordinary stroke into a full-surface damage rect even when most pixels are untouched.
    let mut merged = true;
    while merged && regions.len() > 1 {
        merged = false;
        'outer: for i in 0..regions.len() {
            for j in (i + 1)..regions.len() {
                if let Some(combined) = try_merge_rects(&regions[i], &regions[j]) {
                    regions[i] = combined;
                    regions.remove(j);
                    merged = true;
                    break 'outer;
                }
            }
        }
    }
}

/// Attempts to merge two rectangles if they overlap or are adjacent on the same row/column.
fn try_merge_rects(a: &Rect, b: &Rect) -> Option<Rect> {
    let a_right = a.x.saturating_add(a.width);
    let a_bottom = a.y.saturating_add(a.height);
    let b_right = b.x.saturating_add(b.width);
    let b_bottom = b.y.saturating_add(b.height);

    // Check if rectangles overlap or touch (with 1px margin for adjacency)
    let overlaps_x = ranges_touch_or_overlap(a.x, a_right, b.x, b_right);
    let overlaps_y = ranges_touch_or_overlap(a.y, a_bottom, b.y, b_bottom);

    if !overlaps_x || !overlaps_y {
        return None;
    }

    let aligned_horizontally = ranges_aligned(a.y, a_bottom, b.y, b_bottom);
    let aligned_vertically = ranges_aligned(a.x, a_right, b.x, b_right);
    if !aligned_horizontally && !aligned_vertically {
        return None;
    }

    let min_x = a.x.min(b.x);
    let min_y = a.y.min(b.y);
    let max_x = a_right.max(b_right);
    let max_y = a_bottom.max(b_bottom);
    Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn ranges_touch_or_overlap(a_start: i32, a_end: i32, b_start: i32, b_end: i32) -> bool {
    a_start <= b_end.saturating_add(DAMAGE_MERGE_MARGIN)
        && b_start <= a_end.saturating_add(DAMAGE_MERGE_MARGIN)
}

fn ranges_aligned(a_start: i32, a_end: i32, b_start: i32, b_end: i32) -> bool {
    a_start.abs_diff(b_start) <= DAMAGE_MERGE_ALIGNMENT_TOLERANCE as u32
        && a_end.abs_diff(b_end) <= DAMAGE_MERGE_ALIGNMENT_TOLERANCE as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test pool identity constants
    const TEST_GEN: u64 = 1;
    const TEST_SIZE: usize = 1_000_000;

    #[test]
    fn new_slot_gets_full_damage() {
        let mut tracker = BufferDamageTracker::new(3);

        // First slot should get full damage
        let d1 = tracker.take_buffer_damage(0x1000, 800, 600, TEST_GEN, TEST_SIZE);
        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0], Rect::new(0, 0, 800, 600).unwrap());

        // Second new slot should also get full damage
        let d2 = tracker.take_buffer_damage(0x2000, 800, 600, TEST_GEN, TEST_SIZE);
        assert_eq!(d2.len(), 1);
        assert_eq!(d2[0], Rect::new(0, 0, 800, 600).unwrap());
    }

    #[test]
    fn reused_slot_gets_accumulated_damage() {
        let mut tracker = BufferDamageTracker::new(3);

        // Initialize slots with full damage
        let _ = tracker.take_buffer_damage(0x1000, 800, 600, TEST_GEN, TEST_SIZE);
        let _ = tracker.take_buffer_damage(0x2000, 800, 600, TEST_GEN, TEST_SIZE);
        let _ = tracker.take_buffer_damage(0x3000, 800, 600, TEST_GEN, TEST_SIZE);

        // Add damage - should go to all 3 slots
        tracker.mark_rect(Rect::new(10, 10, 50, 50).unwrap());

        // Render slot 1 - takes its damage
        let d1 = tracker.take_buffer_damage(0x1000, 800, 600, TEST_GEN, TEST_SIZE);
        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0].x, 10);

        // Add more damage - goes to all slots including 1
        tracker.mark_rect(Rect::new(200, 200, 30, 30).unwrap());

        // Slot 2 should have both damage rects
        let d2 = tracker.take_buffer_damage(0x2000, 800, 600, TEST_GEN, TEST_SIZE);
        assert_eq!(d2.len(), 2);

        // Slot 1 reused - should only have the second rect
        let d1_again = tracker.take_buffer_damage(0x1000, 800, 600, TEST_GEN, TEST_SIZE);
        assert_eq!(d1_again.len(), 1);
        assert_eq!(d1_again[0].x, 200);
    }

    #[test]
    fn mark_all_full_affects_all_slots() {
        let mut tracker = BufferDamageTracker::new(2);

        // Initialize slots
        let _ = tracker.take_buffer_damage(0x1000, 800, 600, TEST_GEN, TEST_SIZE);
        let _ = tracker.take_buffer_damage(0x2000, 800, 600, TEST_GEN, TEST_SIZE);

        // Add some damage
        tracker.mark_rect(Rect::new(5, 5, 10, 10).unwrap());

        // Mark all full
        tracker.mark_all_full(FullDamageReason::SurfaceResized);

        // Both slots should get full damage
        let d1 = tracker.take_buffer_damage(0x1000, 800, 600, TEST_GEN, TEST_SIZE);
        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0], Rect::new(0, 0, 800, 600).unwrap());

        let d2 = tracker.take_buffer_damage(0x2000, 800, 600, TEST_GEN, TEST_SIZE);
        assert_eq!(d2.len(), 1);
        assert_eq!(d2[0], Rect::new(0, 0, 800, 600).unwrap());
    }

    #[test]
    fn report_exposes_full_damage_reasons() {
        let mut tracker = BufferDamageTracker::new(2);

        let initial = tracker.take_buffer_damage_report(0x1000, 800, 600, TEST_GEN, TEST_SIZE);
        assert_eq!(initial.full_reason, Some(FullDamageReason::InitialFrame));
        assert_eq!(initial.regions_before_merge, 0);
        assert_eq!(initial.regions_after_merge, 0);

        let new_slot = tracker.take_buffer_damage_report(0x2000, 800, 600, TEST_GEN, TEST_SIZE);
        assert_eq!(new_slot.full_reason, Some(FullDamageReason::NewBufferSlot));

        tracker.mark_all_full(FullDamageReason::CanvasClear);
        let explicit = tracker.take_buffer_damage_report(0x1000, 800, 600, TEST_GEN, TEST_SIZE);
        assert_eq!(explicit.full_reason, Some(FullDamageReason::CanvasClear));
    }

    #[test]
    fn report_exposes_merge_counts() {
        let mut tracker = BufferDamageTracker::new(2);
        let _ = tracker.take_buffer_damage_report(0x1000, 800, 600, TEST_GEN, TEST_SIZE);
        tracker.mark_rect(Rect::new(0, 0, 10, 10).unwrap());
        tracker.mark_rect(Rect::new(5, 0, 10, 10).unwrap());
        tracker.mark_rect(Rect::new(100, 100, 10, 10).unwrap());

        let report = tracker.take_buffer_damage_report(0x1000, 800, 600, TEST_GEN, TEST_SIZE);

        assert_eq!(report.full_reason, None);
        assert_eq!(report.regions_before_merge, 3);
        assert_eq!(report.regions_after_merge, 2);
        assert_eq!(report.regions.len(), 2);
    }

    #[test]
    fn pool_generation_change_resets_tracking() {
        let mut tracker = BufferDamageTracker::new(2);

        // Initialize with gen 1
        let _ = tracker.take_buffer_damage(0x1000, 800, 600, 1, TEST_SIZE);
        tracker.mark_rect(Rect::new(10, 10, 50, 50).unwrap());

        // Changing generation should reset - get full damage
        let d = tracker.take_buffer_damage(0x1000, 800, 600, 2, TEST_SIZE);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0], Rect::new(0, 0, 800, 600).unwrap());
    }

    #[test]
    fn pool_growth_resets_tracking() {
        let mut tracker = BufferDamageTracker::new(2);

        // Initialize with size 1000
        let _ = tracker.take_buffer_damage(0x1000, 800, 600, TEST_GEN, 1000);
        tracker.mark_rect(Rect::new(10, 10, 50, 50).unwrap());

        // Growing pool should reset - get full damage
        let d = tracker.take_buffer_damage(0x1000, 800, 600, TEST_GEN, 2000);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0], Rect::new(0, 0, 800, 600).unwrap());
    }

    #[test]
    fn eviction_only_happens_for_new_slots() {
        let mut tracker = BufferDamageTracker::new(2);

        // Fill up to MAX_TRACKED_BUFFERS
        for i in 0..MAX_TRACKED_BUFFERS {
            let _ = tracker.take_buffer_damage(i * 0x1000, 100, 100, TEST_GEN, TEST_SIZE);
        }
        assert_eq!(tracker.buffers.len(), MAX_TRACKED_BUFFERS);

        // Reusing an existing slot should NOT evict
        let _ = tracker.take_buffer_damage(0x1000, 100, 100, TEST_GEN, TEST_SIZE);
        assert_eq!(tracker.buffers.len(), MAX_TRACKED_BUFFERS);

        // Adding a new slot should evict the oldest
        let _ = tracker.take_buffer_damage(0xF000, 100, 100, TEST_GEN, TEST_SIZE);
        assert_eq!(tracker.buffers.len(), MAX_TRACKED_BUFFERS);
        assert!(tracker.buffers.contains_key(&0xF000));
    }

    #[test]
    fn merge_overlapping_regions() {
        let mut regions = vec![
            Rect::new(0, 0, 10, 10).unwrap(),
            Rect::new(5, 0, 10, 10).unwrap(), // Overlaps first on the same row
        ];
        merge_damage_regions(&mut regions);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], Rect::new(0, 0, 15, 10).unwrap());
    }

    #[test]
    fn merge_adjacent_regions() {
        let mut regions = vec![
            Rect::new(0, 0, 10, 10).unwrap(),
            Rect::new(11, 0, 10, 10).unwrap(), // Adjacent (1px gap)
        ];
        merge_damage_regions(&mut regions);
        assert_eq!(regions.len(), 1);
    }

    #[test]
    fn no_merge_distant_regions() {
        let mut regions = vec![
            Rect::new(0, 0, 10, 10).unwrap(),
            Rect::new(100, 100, 10, 10).unwrap(), // Far away
        ];
        merge_damage_regions(&mut regions);
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn no_merge_diagonal_regions_into_surface_sized_bounds() {
        let mut regions = (0..10)
            .map(|index| {
                let position = index * 10;
                Rect::new(position, position, 20, 20).unwrap()
            })
            .collect::<Vec<_>>();

        merge_damage_regions(&mut regions);

        assert_eq!(regions.len(), 10);
        assert!(
            regions.iter().all(|rect| !(rect.x <= 0
                && rect.y <= 0
                && rect.width >= 100
                && rect.height >= 100)),
            "diagonal damage should stay split instead of becoming full-surface: {regions:?}"
        );
    }

    #[test]
    fn buffer_damage_keeps_diagonal_path_damage_split() {
        let mut tracker = BufferDamageTracker::new(3);
        let _ = tracker.take_buffer_damage_report(0x1000, 100, 100, TEST_GEN, TEST_SIZE);

        for index in 0..10 {
            let position = index * 10;
            tracker.mark_rect(Rect::new(position, position, 20, 20).unwrap());
        }

        let report = tracker.take_buffer_damage_report(0x1000, 100, 100, TEST_GEN, TEST_SIZE);

        assert_eq!(report.full_reason, None);
        assert_eq!(report.regions.len(), 10);
        assert!(
            report.regions.iter().all(|rect| !(rect.x <= 0
                && rect.y <= 0
                && rect.width >= 100
                && rect.height >= 100)),
            "diagonal path damage should not be reported as full damage: {:?}",
            report.regions
        );
    }

    #[test]
    fn multi_buffer_path_damage_stays_split_across_reused_slots() {
        let mut tracker = BufferDamageTracker::new(3);
        for slot in [0x1000, 0x2000, 0x3000] {
            let _ = tracker.take_buffer_damage_report(slot, 1000, 1000, TEST_GEN, TEST_SIZE);
        }

        for index in 0..10 {
            let position = index * 100;
            tracker.mark_rect(Rect::new(position, position, 120, 120).unwrap());
        }

        let first_reused =
            tracker.take_buffer_damage_report(0x2000, 1000, 1000, TEST_GEN, TEST_SIZE);
        assert_eq!(first_reused.full_reason, None);
        assert_path_damage_stays_split(&first_reused.regions);

        tracker.mark_rect(Rect::new(930, 930, 60, 60).unwrap());

        let second_reused =
            tracker.take_buffer_damage_report(0x3000, 1000, 1000, TEST_GEN, TEST_SIZE);
        assert_eq!(second_reused.full_reason, None);
        assert_path_damage_stays_split(&second_reused.regions);
    }

    #[test]
    fn excessive_sparse_damage_falls_back_to_full_surface() {
        let mut tracker = BufferDamageTracker::new(3);
        let _ = tracker.take_buffer_damage_report(0x1000, 4096, 4096, TEST_GEN, TEST_SIZE);

        for index in 0..=MAX_DAMAGE_REGIONS_BEFORE_FULL {
            let position = (index * 8) as i32;
            tracker.mark_rect(Rect::new(position, position, 2, 2).unwrap());
        }

        let report = tracker.take_buffer_damage_report(0x1000, 4096, 4096, TEST_GEN, TEST_SIZE);

        assert_eq!(
            report.full_reason,
            Some(FullDamageReason::DamageRegionLimit)
        );
        assert_eq!(report.regions, vec![Rect::new(0, 0, 4096, 4096).unwrap()]);
        assert_eq!(
            report.regions_before_merge,
            MAX_DAMAGE_REGIONS_BEFORE_FULL + 1
        );
        assert_eq!(report.regions_after_merge, 1);
    }

    fn assert_path_damage_stays_split(regions: &[Rect]) {
        assert!(
            regions.len() > 1,
            "path damage should stay split across reused buffers: {regions:?}"
        );
        assert!(
            regions.iter().all(|rect| !(rect.x <= 0
                && rect.y <= 0
                && rect.width >= 1000
                && rect.height >= 1000)),
            "path damage should not become a full-surface region: {regions:?}"
        );
    }
}
