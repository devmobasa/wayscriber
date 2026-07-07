use crate::util::Rect;

use super::super::FullDamageReason;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::backend::wayland) struct PerfDamageDiagnostics {
    pub(in crate::backend::wayland) input_regions: usize,
    pub(in crate::backend::wayland) input_full_reason: Option<FullDamageReason>,
    pub(in crate::backend::wayland) input_covers_surface: bool,
    pub(in crate::backend::wayland) buffer_regions_before_merge: usize,
    pub(in crate::backend::wayland) buffer_regions_after_merge: usize,
    pub(in crate::backend::wayland) buffer_covers_surface: bool,
    pub(in crate::backend::wayland) final_single_surface_rect: bool,
    pub(in crate::backend::wayland) largest_region_area_pct_hundredths: u32,
}

pub(in crate::backend::wayland) struct PerfFrameDamageContext<'a> {
    pub(in crate::backend::wayland) damage_screen: &'a [Rect],
    pub(in crate::backend::wayland) logical_width: u32,
    pub(in crate::backend::wayland) logical_height: u32,
    pub(in crate::backend::wayland) damage_rects: usize,
    pub(in crate::backend::wayland) force_full_reason: Option<FullDamageReason>,
    pub(in crate::backend::wayland) diagnostics: PerfDamageDiagnostics,
}

pub(super) fn format_effective_full_damage_reason(
    full_damage: bool,
    force_full_reason: Option<FullDamageReason>,
) -> &'static str {
    format_damage_reason(effective_full_damage_reason(full_damage, force_full_reason))
}

pub(super) fn effective_full_damage_reason(
    full_damage: bool,
    force_full_reason: Option<FullDamageReason>,
) -> Option<FullDamageReason> {
    if !full_damage {
        None
    } else {
        Some(force_full_reason.unwrap_or(FullDamageReason::DamageRegionsCoverSurface))
    }
}

pub(super) fn full_damage_source(
    full_damage: bool,
    force_full_reason: Option<FullDamageReason>,
    diagnostics: &PerfDamageDiagnostics,
) -> &'static str {
    if !full_damage {
        "none"
    } else if diagnostics.input_full_reason.is_some() {
        "input_full"
    } else if force_full_reason.is_some() {
        "explicit_force"
    } else if diagnostics.input_covers_surface {
        "input_regions_cover_surface"
    } else if diagnostics.buffer_covers_surface {
        "buffer_regions_cover_surface"
    } else if diagnostics.final_single_surface_rect {
        "final_single_surface_rect"
    } else {
        "unknown"
    }
}

pub(super) fn format_pct_hundredths(value: u32) -> String {
    format!("{}.{:02}", value / 100, value % 100)
}

pub(super) fn damage_area_pct(damage: &[Rect], logical_width: u32, logical_height: u32) -> f64 {
    let surface_area = u64::from(logical_width).saturating_mul(u64::from(logical_height));
    if surface_area == 0 {
        return 0.0;
    }

    let damage_area = damage
        .iter()
        .map(|rect| clamped_rect_area(*rect, logical_width, logical_height))
        .sum::<u64>();
    ((damage_area as f64 / surface_area as f64) * 100.0).min(100.0)
}

pub(super) fn largest_region_area_pct_hundredths(
    damage: &[Rect],
    logical_width: u32,
    logical_height: u32,
) -> u32 {
    let surface_area = u64::from(logical_width).saturating_mul(u64::from(logical_height));
    if surface_area == 0 {
        return 0;
    }

    let largest = damage
        .iter()
        .map(|rect| clamped_rect_area(*rect, logical_width, logical_height))
        .max()
        .unwrap_or(0);
    let hundredths = (u128::from(largest) * 10_000) / u128::from(surface_area);
    hundredths.min(10_000) as u32
}

pub(super) fn damage_covers_surface(
    damage: &[Rect],
    logical_width: u32,
    logical_height: u32,
) -> bool {
    let width = logical_width.min(i32::MAX as u32) as i32;
    let height = logical_height.min(i32::MAX as u32) as i32;
    damage_covers_logical_surface(damage, width, height)
}

pub(in crate::backend::wayland) fn damage_covers_logical_surface(
    damage: &[Rect],
    logical_width: i32,
    logical_height: i32,
) -> bool {
    damage.iter().any(|rect| {
        rect.x <= 0 && rect.y <= 0 && rect.width >= logical_width && rect.height >= logical_height
    })
}

fn clamped_rect_area(rect: Rect, logical_width: u32, logical_height: u32) -> u64 {
    let max_x = logical_width.min(i32::MAX as u32) as i32;
    let max_y = logical_height.min(i32::MAX as u32) as i32;
    let left = rect.x.clamp(0, max_x);
    let top = rect.y.clamp(0, max_y);
    let right = rect.x.saturating_add(rect.width).clamp(0, max_x);
    let bottom = rect.y.saturating_add(rect.height).clamp(0, max_y);
    if right <= left || bottom <= top {
        return 0;
    }
    (right - left) as u64 * (bottom - top) as u64
}

fn format_damage_reason(reason: Option<FullDamageReason>) -> &'static str {
    reason.map_or("none", FullDamageReason::as_str)
}
