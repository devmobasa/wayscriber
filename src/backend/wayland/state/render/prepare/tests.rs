use super::*;
use crate::util::Rect;

const GENERATION: u64 = 1;
const POOL_SIZE: usize = 4096;

#[test]
fn empty_damage_fallback_reaches_every_reused_slot_and_retains_merge_counts() {
    let geometry = FrameGeometry::new(800, 600, 1);
    let full = Rect::new(0, 0, 800, 600).unwrap();
    let mut tracker = BufferDamageTracker::new(3);
    for slot in [1, 2, 3] {
        let report = take_frame_damage(&mut tracker, &geometry, slot, GENERATION, POOL_SIZE);
        assert_eq!(report.regions, [full]);
    }

    let report = take_frame_damage(&mut tracker, &geometry, 1, GENERATION, POOL_SIZE);
    assert_eq!(report.regions, [full]);
    assert_eq!(
        report.full_reason,
        Some(FullDamageReason::EmptyDamageFallback)
    );
    assert_eq!(
        (report.regions_before_merge, report.regions_after_merge),
        (0, 0)
    );

    // Both lagging slots and the triggering slot retain the global fallback.
    for slot in [2, 3, 1] {
        let report = take_frame_damage(&mut tracker, &geometry, slot, GENERATION, POOL_SIZE);
        assert_eq!(report.regions, [full], "slot {slot}");
        assert_eq!(
            report.full_reason,
            Some(FullDamageReason::EmptyDamageFallback)
        );
        assert_eq!(
            (report.regions_before_merge, report.regions_after_merge),
            (0, 0)
        );
    }

    // Once full damage has drained, real merging and its diagnostics resume.
    let dirty = Rect::new(10, 10, 20, 20).unwrap();
    tracker.add_regions(vec![dirty, dirty]);
    for slot in [1, 2, 3] {
        let report = take_frame_damage(&mut tracker, &geometry, slot, GENERATION, POOL_SIZE);
        assert_eq!(report.regions, [dirty]);
        assert_eq!(report.full_reason, None);
        assert_eq!(
            (report.regions_before_merge, report.regions_after_merge),
            (2, 1)
        );
    }
}

#[test]
fn empty_surface_does_not_manufacture_full_damage_fallback() {
    let geometry = FrameGeometry::new(0, 600, 1);
    let mut tracker = BufferDamageTracker::new(1);
    let first = take_frame_damage(&mut tracker, &geometry, 1, GENERATION, POOL_SIZE);
    assert!(first.regions.is_empty());
    assert_eq!(first.full_reason, Some(FullDamageReason::InitialFrame));
    let reused = take_frame_damage(&mut tracker, &geometry, 1, GENERATION, POOL_SIZE);
    assert!(reused.regions.is_empty());
    assert_eq!(reused.full_reason, None);
    assert_eq!(
        (reused.regions_before_merge, reused.regions_after_merge),
        (0, 0)
    );
}
