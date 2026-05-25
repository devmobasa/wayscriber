//! Runtime support for final-render color profile remapping.

use std::collections::HashMap;

use crate::config::{RenderProfileConfig, RenderProfilesConfig};
use crate::util::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb8 {
    fn key(self) -> u32 {
        (u32::from(self.r) << 16) | (u32::from(self.g) << 8) | u32::from(self.b)
    }
}

#[derive(Clone, Debug)]
pub struct RenderColorProfile {
    id: String,
    name: String,
    mappings: HashMap<u32, Rgb8>,
}

impl RenderColorProfile {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    fn from_config(config: &RenderProfileConfig) -> Option<Self> {
        let mut mappings = HashMap::with_capacity(config.mappings.len());
        for mapping in &config.mappings {
            let Some(from) = parse_hex_rgb(&mapping.from) else {
                continue;
            };
            let Some(to) = parse_hex_rgb(&mapping.to) else {
                continue;
            };
            mappings.insert(from.key(), to);
        }

        (!config.id.trim().is_empty()).then(|| Self {
            id: normalize_profile_id(&config.id),
            name: if config.name.trim().is_empty() {
                normalize_profile_id(&config.id)
            } else {
                config.name.trim().to_string()
            },
            mappings,
        })
    }

    fn remap_pixel(&self, pixel: u32) -> u32 {
        let alpha = ((pixel >> 24) & 0xff) as u8;
        if alpha == 0 {
            return pixel;
        }

        let red = unpremultiply_component(((pixel >> 16) & 0xff) as u8, alpha);
        let green = unpremultiply_component(((pixel >> 8) & 0xff) as u8, alpha);
        let blue = unpremultiply_component((pixel & 0xff) as u8, alpha);
        let Some(target) = self.mappings.get(
            &Rgb8 {
                r: red,
                g: green,
                b: blue,
            }
            .key(),
        ) else {
            return pixel;
        };

        let premul_red = premultiply_component(target.r, alpha);
        let premul_green = premultiply_component(target.g, alpha);
        let premul_blue = premultiply_component(target.b, alpha);
        (u32::from(alpha) << 24)
            | (u32::from(premul_red) << 16)
            | (u32::from(premul_green) << 8)
            | u32::from(premul_blue)
    }

    pub fn remap_argb8888_regions(
        &self,
        data: &mut [u8],
        width: i32,
        height: i32,
        stride: i32,
        regions: &[Rect],
    ) {
        if self.is_empty() || width <= 0 || height <= 0 || stride < width.saturating_mul(4) {
            return;
        }

        let stride = stride as usize;
        for region in regions {
            let x0 = region.x.max(0).min(width);
            let y0 = region.y.max(0).min(height);
            let x1 = region.x.saturating_add(region.width).max(0).min(width);
            let y1 = region.y.saturating_add(region.height).max(0).min(height);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }

            for y in y0..y1 {
                let row_start = y as usize * stride;
                for x in x0..x1 {
                    let offset = row_start + x as usize * 4;
                    if offset + 4 > data.len() {
                        return;
                    }
                    let pixel = u32::from_ne_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]);
                    let mapped = self.remap_pixel(pixel);
                    if mapped != pixel {
                        data[offset..offset + 4].copy_from_slice(&mapped.to_ne_bytes());
                    }
                }
            }
        }
    }

    pub fn remap_argb8888_regions_changed_from(
        &self,
        data: &mut [u8],
        baseline: &[u8],
        width: i32,
        height: i32,
        stride: i32,
        regions: &[Rect],
    ) {
        if self.is_empty()
            || width <= 0
            || height <= 0
            || stride < width.saturating_mul(4)
            || baseline.len() < data.len()
        {
            return;
        }

        let stride = stride as usize;
        for region in regions {
            let x0 = region.x.max(0).min(width);
            let y0 = region.y.max(0).min(height);
            let x1 = region.x.saturating_add(region.width).max(0).min(width);
            let y1 = region.y.saturating_add(region.height).max(0).min(height);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }

            for y in y0..y1 {
                let row_start = y as usize * stride;
                for x in x0..x1 {
                    let offset = row_start + x as usize * 4;
                    if offset + 4 > data.len() {
                        return;
                    }
                    if data[offset..offset + 4] == baseline[offset..offset + 4] {
                        continue;
                    }
                    let pixel = u32::from_ne_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]);
                    let mapped = self.remap_pixel(pixel);
                    if mapped != pixel {
                        data[offset..offset + 4].copy_from_slice(&mapped.to_ne_bytes());
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderProfileSet {
    profiles: Vec<RenderColorProfile>,
    active_index: Option<usize>,
    apply_to_canvas: bool,
    apply_to_ui: bool,
    generation: u64,
}

impl Default for RenderProfileSet {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            active_index: None,
            apply_to_canvas: true,
            apply_to_ui: true,
            generation: 0,
        }
    }
}

impl RenderProfileSet {
    pub fn from_config(config: &RenderProfilesConfig) -> Self {
        let profiles: Vec<_> = config
            .items
            .iter()
            .filter_map(RenderColorProfile::from_config)
            .collect();
        let active_index = config.active.as_ref().and_then(|active| {
            let active = normalize_profile_id(active);
            profiles.iter().position(|profile| profile.id == active)
        });
        Self {
            profiles,
            active_index,
            apply_to_canvas: config.apply_to_canvas,
            apply_to_ui: config.apply_to_ui,
            generation: 0,
        }
    }

    pub fn active(&self) -> Option<&RenderColorProfile> {
        self.active_index.and_then(|index| self.profiles.get(index))
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn applies_to_canvas(&self) -> bool {
        self.apply_to_canvas
    }

    pub fn applies_to_ui(&self) -> bool {
        self.apply_to_ui
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    pub fn activate_next(&mut self) -> bool {
        if self.profiles.is_empty() {
            return false;
        }
        let next = match self.active_index {
            None => Some(0),
            Some(index) if index + 1 < self.profiles.len() => Some(index + 1),
            Some(_) => None,
        };
        self.set_active_index(next)
    }

    pub fn activate_previous(&mut self) -> bool {
        if self.profiles.is_empty() {
            return false;
        }
        let previous = match self.active_index {
            None => Some(self.profiles.len() - 1),
            Some(0) => None,
            Some(index) => Some(index - 1),
        };
        self.set_active_index(previous)
    }

    pub fn deactivate(&mut self) -> bool {
        self.set_active_index(None)
    }

    fn set_active_index(&mut self, active_index: Option<usize>) -> bool {
        if self.active_index == active_index {
            return false;
        }
        self.active_index = active_index;
        self.generation = self.generation.wrapping_add(1);
        true
    }
}

pub fn normalize_profile_id(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub fn parse_hex_rgb(value: &str) -> Option<Rgb8> {
    let trimmed = value.trim();
    let hex = trimmed
        .strip_prefix('#')
        .or_else(|| trimmed.strip_prefix("0x"))
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if hex.len() != 6 || !hex.as_bytes().iter().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Rgb8 { r, g, b })
}

pub fn format_hex_rgb(color: Rgb8) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

fn unpremultiply_component(value: u8, alpha: u8) -> u8 {
    if alpha == 255 {
        value
    } else {
        ((u32::from(value) * 255 + u32::from(alpha) / 2) / u32::from(alpha)).min(255) as u8
    }
}

fn premultiply_component(value: u8, alpha: u8) -> u8 {
    ((u32::from(value) * u32::from(alpha) + 127) / 255).min(255) as u8
}

#[cfg(test)]
mod tests {
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
        (u32::from(alpha) << 24)
            | (u32::from(red) << 16)
            | (u32::from(green) << 8)
            | u32::from(blue)
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
            items: vec![
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
            items: Vec::new(),
        });

        assert!(!set.applies_to_canvas());
        assert!(set.applies_to_ui());
    }
}
