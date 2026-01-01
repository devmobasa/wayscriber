use super::helpers;
use super::*;
use crate::util::Rect;

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
    assert!(!helpers::parse_debug_damage_env(""));
    assert!(!helpers::parse_debug_damage_env("0"));
    assert!(!helpers::parse_debug_damage_env("false"));
    assert!(!helpers::parse_debug_damage_env("off"));
    assert!(helpers::parse_debug_damage_env("1"));
    assert!(helpers::parse_debug_damage_env("true"));
}

#[test]
fn parse_boolish_env_handles_case_and_on() {
    assert!(helpers::parse_boolish_env("ON"));
    assert!(helpers::parse_boolish_env("yes"));
    assert!(!helpers::parse_boolish_env("Off"));
    assert!(!helpers::parse_boolish_env("0"));
}

#[test]
fn damage_summary_truncates_after_five_regions() {
    let mut regions = Vec::new();
    for i in 0..6 {
        regions.push(Rect {
            x: i,
            y: i,
            width: 1,
            height: 2,
        });
    }
    let summary = damage_summary(&regions);
    assert_eq!(
        summary,
        "(0,0) 1x2, (1,1) 1x2, (2,2) 1x2, (3,3) 1x2, (4,4) 1x2, ... +1 more"
    );
}

#[test]
fn resolve_then_scale_damage_regions_keeps_full_region() {
    let regions = resolve_damage_regions(100, 50, Vec::new());
    let scaled = scale_damage_regions(regions, 2);
    assert_eq!(scaled.len(), 1);
    assert_eq!(scaled[0], Rect::new(0, 0, 200, 100).unwrap());
}

#[test]
fn force_inline_toolbars_requested_uses_config_or_env() {
    let mut config = Config::default();
    assert!(!helpers::force_inline_toolbars_requested_with_env(
        &config, false
    ));
    assert!(helpers::force_inline_toolbars_requested_with_env(
        &config, true
    ));
    config.ui.toolbar.force_inline = true;
    assert!(helpers::force_inline_toolbars_requested_with_env(
        &config, false
    ));
}
