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

use crate::util::Rect;

/// Maximum number of buffer slots to track (prevents unbounded growth).
const MAX_TRACKED_BUFFERS: usize = 8;

/// Damage state for a single buffer slot.
#[derive(Debug, Default)]
struct BufferDamage {
    /// Dirty regions accumulated since this buffer was last rendered.
    regions: Vec<Rect>,
    /// Force full damage on next render (e.g., new buffer or after resize).
    force_full: bool,
    /// Frame counter for LRU eviction.
    last_used_frame: u64,
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
    pub fn mark_all_full(&mut self) {
        self.global_full_damage = true;
        for damage in self.buffers.values_mut() {
            damage.force_full = true;
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
    pub fn take_buffer_damage(
        &mut self,
        canvas_ptr: usize,
        width: i32,
        height: i32,
        pool_generation: u64,
        pool_size: usize,
    ) -> Vec<Rect> {
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

        let damage = self
            .buffers
            .entry(canvas_ptr)
            .or_insert_with(|| BufferDamage {
                regions: Vec::new(),
                force_full: true, // New slot always needs full damage
                last_used_frame: frame,
            });

        damage.last_used_frame = frame;

        // Check if we need full damage
        let needs_full = damage.force_full || self.global_full_damage;

        if needs_full {
            damage.force_full = false;
            damage.regions.clear();
            // Clear global flag only after a buffer has received it
            if self.global_full_damage {
                self.global_full_damage = false;
            }

            if width > 0
                && height > 0
                && let Some(full) = Rect::new(0, 0, width, height)
            {
                return vec![full];
            }
            return Vec::new();
        }

        // Take and deduplicate regions
        let mut regions: Vec<Rect> = damage.regions.drain(..).collect();
        regions.retain(Rect::is_valid);

        // Merge overlapping regions to reduce compositor work
        merge_damage_regions(&mut regions);

        regions
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

/// Merges overlapping or adjacent damage regions to reduce compositor work.
fn merge_damage_regions(regions: &mut Vec<Rect>) {
    if regions.len() <= 1 {
        return;
    }

    // Simple greedy merge: combine regions that overlap or touch
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

/// Attempts to merge two rectangles if they overlap or are adjacent.
fn try_merge_rects(a: &Rect, b: &Rect) -> Option<Rect> {
    let a_right = a.x.saturating_add(a.width);
    let a_bottom = a.y.saturating_add(a.height);
    let b_right = b.x.saturating_add(b.width);
    let b_bottom = b.y.saturating_add(b.height);

    // Check if rectangles overlap or touch (with 1px margin for adjacency)
    let margin = 1;
    let overlaps_x = a.x <= b_right + margin && b.x <= a_right + margin;
    let overlaps_y = a.y <= b_bottom + margin && b.y <= a_bottom + margin;

    if overlaps_x && overlaps_y {
        let min_x = a.x.min(b.x);
        let min_y = a.y.min(b.y);
        let max_x = a_right.max(b_right);
        let max_y = a_bottom.max(b_bottom);
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    } else {
        None
    }
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
        tracker.mark_all_full();

        // Both slots should get full damage
        let d1 = tracker.take_buffer_damage(0x1000, 800, 600, TEST_GEN, TEST_SIZE);
        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0], Rect::new(0, 0, 800, 600).unwrap());

        let d2 = tracker.take_buffer_damage(0x2000, 800, 600, TEST_GEN, TEST_SIZE);
        assert_eq!(d2.len(), 1);
        assert_eq!(d2[0], Rect::new(0, 0, 800, 600).unwrap());
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
            Rect::new(5, 5, 10, 10).unwrap(), // Overlaps first
        ];
        merge_damage_regions(&mut regions);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], Rect::new(0, 0, 15, 15).unwrap());
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
}
