//! Runtime support for final-render color profile remapping.

use std::collections::HashMap;

use crate::config::{RenderProfileConfig, RenderProfileExportMode, RenderProfilesConfig};
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
    #[allow(dead_code)] // Used by tests and consumers of the library API.
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    pub(crate) fn from_config(config: &RenderProfileConfig) -> Option<Self> {
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
    export_mode: RenderProfileExportMode,
    export_profile_index: Option<usize>,
    apply_to_canvas: bool,
    apply_to_ui: bool,
    generation: u64,
}

impl Default for RenderProfileSet {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            active_index: None,
            export_mode: RenderProfileExportMode::Off,
            export_profile_index: None,
            apply_to_canvas: true,
            apply_to_ui: true,
            generation: 0,
        }
    }
}

impl RenderProfileSet {
    pub fn from_config(config: &RenderProfilesConfig) -> Self {
        let profiles: Vec<_> = config
            .profiles
            .iter()
            .filter_map(RenderColorProfile::from_config)
            .collect();
        let active_index = config.active.as_ref().and_then(|active| {
            let active = normalize_profile_id(active);
            profiles.iter().position(|profile| profile.id == active)
        });
        let export_profile_index = config.export_profile.as_ref().and_then(|profile_id| {
            let profile_id = normalize_profile_id(profile_id);
            profiles.iter().position(|profile| profile.id == profile_id)
        });
        Self {
            profiles,
            active_index,
            export_mode: config.export,
            export_profile_index,
            apply_to_canvas: config.apply_to_canvas,
            apply_to_ui: config.apply_to_ui,
            generation: 0,
        }
    }

    pub fn active(&self) -> Option<&RenderColorProfile> {
        self.active_index.and_then(|index| self.profiles.get(index))
    }

    pub fn export_profile(&self) -> Option<RenderColorProfile> {
        match self.export_mode {
            RenderProfileExportMode::Off => None,
            RenderProfileExportMode::Active => self.active().cloned(),
            RenderProfileExportMode::Profile => self
                .export_profile_index
                .and_then(|index| self.profiles.get(index))
                .cloned(),
        }
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
mod tests;
