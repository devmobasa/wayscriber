use super::*;

// NOTE: The functions below are used for diagnostic logging only; the renderer currently applies
// full-surface damage for correctness. These tests document the intended behavior if we ever
// reintroduce partial damage handling.
#[test]
fn resolve_damage_returns_full_when_empty() {
    let regions = resolve_damage_regions(1920, 1080, Vec::new());
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0], Rect::new(0, 0, 1920, 1080).unwrap());
}

#[test]
fn resolve_damage_filters_invalid_rects() {
    let regions = resolve_damage_regions(
        800,
        600,
        vec![
            Rect {
                x: 10,
                y: 10,
                width: 50,
                height: 40,
            },
            Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 10,
            },
        ],
    );

    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0], Rect::new(10, 10, 50, 40).unwrap());
}

#[test]
fn resolve_damage_preserves_existing_regions() {
    let regions = resolve_damage_regions(
        800,
        600,
        vec![Rect {
            x: 5,
            y: 5,
            width: 20,
            height: 30,
        }],
    );

    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0], Rect::new(5, 5, 20, 30).unwrap());
}

#[test]
fn full_damage_policy_is_explicit() {
    // This documents that we intentionally call damage_buffer over the full surface to avoid
    // stale pixels with buffer reuse. If you switch back to partial damage, implement
    // per-buffer damage tracking instead of draining a single accumulator.
}

#[test]
fn scale_damage_regions_multiplies_by_scale() {
    let regions = vec![Rect {
        x: 2,
        y: 3,
        width: 4,
        height: 5,
    }];
    let scaled = scale_damage_regions(regions, 2);
    assert_eq!(scaled.len(), 1);
    assert_eq!(scaled[0], Rect::new(4, 6, 8, 10).unwrap());
}

#[test]
fn debug_damage_logging_env_parses_falsey() {
    assert!(!parse_debug_damage_env(""));
    assert!(!parse_debug_damage_env("0"));
    assert!(!parse_debug_damage_env("false"));
    assert!(!parse_debug_damage_env("off"));
    assert!(parse_debug_damage_env("1"));
    assert!(parse_debug_damage_env("true"));
}
