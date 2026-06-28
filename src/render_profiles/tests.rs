use super::*;
use crate::config::RenderColorMappingConfig;

fn profile(from: &str, to: &str) -> RenderColorProfile {
    RenderColorProfile::from_config(&RenderProfileConfig {
        id: "print".to_string(),
        name: "Print".to_string(),
        mappings: vec![RenderColorMappingConfig {
            from: from.to_string(),
            to: to.to_string(),
        }],
    })
    .expect("profile")
}

fn argb(alpha: u8, red: u8, green: u8, blue: u8) -> u32 {
    let red = premultiply_component(red, alpha);
    let green = premultiply_component(green, alpha);
    let blue = premultiply_component(blue, alpha);
    (u32::from(alpha) << 24) | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

#[test]
fn parse_hex_rgb_accepts_supported_forms() {
    assert_eq!(
        parse_hex_rgb("#8B4513"),
        Some(Rgb8 {
            r: 0x8b,
            g: 0x45,
            b: 0x13,
        })
    );
    assert_eq!(
        parse_hex_rgb("0xFFFFFF"),
        Some(Rgb8 {
            r: 255,
            g: 255,
            b: 255
        })
    );
    assert_eq!(parse_hex_rgb("000000"), Some(Rgb8 { r: 0, g: 0, b: 0 }));
    assert_eq!(
        format_hex_rgb(Rgb8 {
            r: 0x8b,
            g: 0x45,
            b: 0x13,
        }),
        "#8B4513"
    );
}

#[test]
fn parse_hex_rgb_rejects_invalid_values() {
    assert_eq!(parse_hex_rgb("#FFF"), None);
    assert_eq!(parse_hex_rgb("#GG0000"), None);
    assert_eq!(parse_hex_rgb(""), None);
}

#[test]
fn remap_preserves_alpha_for_semitransparent_pixels() {
    let profile = profile("#808000", "#0000FF");
    let mapped = profile.remap_pixel(argb(128, 128, 128, 0));
    assert_eq!(mapped, argb(128, 0, 0, 255));
}

#[test]
fn remap_leaves_unmapped_and_transparent_pixels_unchanged() {
    let profile = profile("#000000", "#FFFFFF");
    assert_eq!(
        profile.remap_pixel(argb(255, 10, 20, 30)),
        argb(255, 10, 20, 30)
    );
    assert_eq!(profile.remap_pixel(0), 0);
}

#[test]
fn remap_argb8888_regions_only_changes_damaged_pixels() {
    let profile = profile("#000000", "#FFFFFF");
    let mut data = Vec::new();
    data.extend_from_slice(&argb(255, 0, 0, 0).to_ne_bytes());
    data.extend_from_slice(&argb(255, 0, 0, 0).to_ne_bytes());

    profile.remap_argb8888_regions(
        &mut data,
        2,
        1,
        8,
        &[Rect::new(1, 0, 1, 1).expect("valid rect")],
    );

    assert_eq!(
        u32::from_ne_bytes(data[0..4].try_into().unwrap()),
        argb(255, 0, 0, 0)
    );
    assert_eq!(
        u32::from_ne_bytes(data[4..8].try_into().unwrap()),
        argb(255, 255, 255, 255)
    );
}

#[test]
fn remap_argb8888_changed_regions_skips_unchanged_canvas_pixels() {
    let profile = profile("#000000", "#FFFFFF");
    let mut baseline = Vec::new();
    baseline.extend_from_slice(&argb(255, 0, 0, 0).to_ne_bytes());
    baseline.extend_from_slice(&argb(255, 255, 0, 0).to_ne_bytes());
    let mut data = Vec::new();
    data.extend_from_slice(&argb(255, 0, 0, 0).to_ne_bytes());
    data.extend_from_slice(&argb(255, 0, 0, 0).to_ne_bytes());

    profile.remap_argb8888_regions_changed_from(
        &mut data,
        &baseline,
        2,
        1,
        8,
        &[Rect::new(0, 0, 2, 1).expect("valid rect")],
    );

    assert_eq!(
        u32::from_ne_bytes(data[0..4].try_into().unwrap()),
        argb(255, 0, 0, 0)
    );
    assert_eq!(
        u32::from_ne_bytes(data[4..8].try_into().unwrap()),
        argb(255, 255, 255, 255)
    );
}

#[test]
fn render_profile_set_cycles_through_profiles_and_off_state() {
    fn active_id(set: &RenderProfileSet) -> Option<&str> {
        set.active().map(|profile| profile.id.as_str())
    }

    let config = RenderProfilesConfig {
        active: Some("first".to_string()),
        apply_to_canvas: true,
        apply_to_ui: true,
        export: RenderProfileExportMode::Off,
        export_profile: None,
        profiles: vec![
            RenderProfileConfig {
                id: "first".to_string(),
                name: "First".to_string(),
                mappings: Vec::new(),
            },
            RenderProfileConfig {
                id: "second".to_string(),
                name: "Second".to_string(),
                mappings: Vec::new(),
            },
        ],
    };
    let mut set = RenderProfileSet::from_config(&config);

    assert_eq!(active_id(&set), Some("first"));
    assert!(set.activate_next());
    assert_eq!(active_id(&set), Some("second"));
    assert!(set.activate_next());
    assert_eq!(active_id(&set), None);
    assert!(set.activate_previous());
    assert_eq!(active_id(&set), Some("second"));
}

#[test]
fn render_profile_set_preserves_target_flags() {
    let set = RenderProfileSet::from_config(&RenderProfilesConfig {
        active: None,
        apply_to_canvas: false,
        apply_to_ui: true,
        export: RenderProfileExportMode::Off,
        export_profile: None,
        profiles: Vec::new(),
    });

    assert!(!set.applies_to_canvas());
    assert!(set.applies_to_ui());
}

#[test]
fn export_profile_resolves_off_active_and_named_profiles() {
    let config = RenderProfilesConfig {
        active: Some("active".to_string()),
        apply_to_canvas: true,
        apply_to_ui: true,
        export: RenderProfileExportMode::Active,
        export_profile: Some("off".to_string()),
        profiles: vec![
            RenderProfileConfig {
                id: "active".to_string(),
                name: "Active".to_string(),
                mappings: Vec::new(),
            },
            RenderProfileConfig {
                id: "off".to_string(),
                name: "Off Named Profile".to_string(),
                mappings: Vec::new(),
            },
        ],
    };

    let mut active = RenderProfileSet::from_config(&config);
    assert_eq!(
        active.export_profile().as_ref().map(|p| p.id()),
        Some("active")
    );

    let mut named_config = config;
    named_config.export = RenderProfileExportMode::Profile;
    active = RenderProfileSet::from_config(&named_config);
    assert_eq!(
        active.export_profile().as_ref().map(|p| p.id()),
        Some("off")
    );

    named_config.export = RenderProfileExportMode::Off;
    active = RenderProfileSet::from_config(&named_config);
    assert!(active.export_profile().is_none());
}

#[test]
fn config_serializes_profile_collection_as_profiles() {
    let config = RenderProfilesConfig {
        active: None,
        apply_to_canvas: true,
        apply_to_ui: true,
        export: RenderProfileExportMode::Off,
        export_profile: None,
        profiles: vec![RenderProfileConfig {
            id: "print".to_string(),
            name: "Print".to_string(),
            mappings: Vec::new(),
        }],
    };

    let serialized = toml::to_string(&config).expect("serialize");

    assert!(serialized.contains("[[profiles]]"));
    assert!(!serialized.contains("[[items]]"));
}
