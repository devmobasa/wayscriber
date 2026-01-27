use std::{sync::OnceLock, time::Duration};

use wayland_client::{Proxy, protocol::wl_surface};

use crate::{config::Config, util::Rect};

#[allow(dead_code)]
pub(in crate::backend::wayland) fn resolve_damage_regions(
    width: i32,
    height: i32,
    mut regions: Vec<Rect>,
) -> Vec<Rect> {
    regions.retain(Rect::is_valid);

    if regions.is_empty()
        && width > 0
        && height > 0
        && let Some(full) = Rect::new(0, 0, width, height)
    {
        regions.push(full);
    }

    regions
}

#[allow(dead_code)]
pub(in crate::backend::wayland) fn scale_damage_regions(
    regions: Vec<Rect>,
    scale: i32,
) -> Vec<Rect> {
    if scale <= 1 {
        return regions;
    }

    regions
        .into_iter()
        .filter_map(|r| {
            let x = r.x.saturating_mul(scale);
            let y = r.y.saturating_mul(scale);
            let w = r.width.saturating_mul(scale);
            let h = r.height.saturating_mul(scale);

            Rect::new(x, y, w, h)
        })
        .collect()
}

pub(in crate::backend::wayland) fn damage_summary(regions: &[Rect]) -> String {
    if regions.is_empty() {
        return "[]".to_string();
    }

    let mut parts = Vec::with_capacity(regions.len());
    for r in regions.iter().take(5) {
        parts.push(format!("({},{}) {}x{}", r.x, r.y, r.width, r.height));
    }
    if regions.len() > 5 {
        parts.push(format!("... +{} more", regions.len() - 5));
    }
    parts.join(", ")
}

pub(super) fn parse_boolish_env(raw: &str) -> bool {
    let v = raw.to_ascii_lowercase();
    !(v.is_empty() || v == "0" || v == "false" || v == "off")
}

pub(super) fn parse_debug_damage_env(raw: &str) -> bool {
    parse_boolish_env(raw)
}

pub(in crate::backend::wayland) fn debug_damage_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        parse_debug_damage_env(&std::env::var("WAYSCRIBER_DEBUG_DAMAGE").unwrap_or_default())
    })
}

pub(in crate::backend::wayland) fn surface_id(surface: &wl_surface::WlSurface) -> u32 {
    surface.id().protocol_id()
}

pub(in crate::backend::wayland) fn debug_toolbar_drag_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        parse_boolish_env(&std::env::var("WAYSCRIBER_DEBUG_TOOLBAR_DRAG").unwrap_or_default())
    })
}

pub(in crate::backend::wayland) fn toolbar_pointer_lock_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        // Default ON: without pointer lock, layer-shell toolbar drags jitter/flicker as surfaces move.
        parse_boolish_env(
            &std::env::var("WAYSCRIBER_TOOLBAR_POINTER_LOCK").unwrap_or_else(|_| "1".into()),
        )
    })
}

pub(in crate::backend::wayland) fn toolbar_drag_preview_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        parse_boolish_env(
            &std::env::var("WAYSCRIBER_TOOLBAR_DRAG_PREVIEW").unwrap_or_else(|_| "1".into()),
        )
    })
}

pub(in crate::backend::wayland) fn toolbar_drag_throttle_interval() -> Option<Duration> {
    static VALUE: OnceLock<Option<Duration>> = OnceLock::new();
    *VALUE.get_or_init(|| {
        let raw =
            std::env::var("WAYSCRIBER_TOOLBAR_DRAG_THROTTLE_MS").unwrap_or_else(|_| "12".into());
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Some(Duration::from_millis(12));
        }
        let Ok(ms) = trimmed.parse::<u64>() else {
            return Some(Duration::from_millis(12));
        };
        if ms == 0 {
            None
        } else {
            Some(Duration::from_millis(ms))
        }
    })
}

pub(in crate::backend::wayland) fn drag_log(message: impl AsRef<str>) {
    if debug_toolbar_drag_logging_enabled() {
        log::info!("{}", message.as_ref());
    }
}

fn force_inline_env_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        parse_boolish_env(&std::env::var("WAYSCRIBER_FORCE_INLINE_TOOLBARS").unwrap_or_default())
    })
}

pub(super) fn force_inline_toolbars_requested_with_env(
    config: &Config,
    env_force_inline: bool,
) -> bool {
    config.ui.toolbar.force_inline || env_force_inline
}

pub(in crate::backend::wayland) fn force_inline_toolbars_requested(config: &Config) -> bool {
    force_inline_toolbars_requested_with_env(config, force_inline_env_enabled())
}
