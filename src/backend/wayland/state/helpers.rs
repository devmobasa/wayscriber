use std::time::Duration;

use wayland_client::{Proxy, protocol::wl_surface};

use crate::env_vars::{
    DEBUG_DAMAGE_ENV, DEBUG_TOOLBAR_COLOR_ENV, DEBUG_TOOLBAR_DRAG_ENV, FORCE_INLINE_TOOLBARS_ENV,
    TOOLBAR_DRAG_HANDOFF_MS_ENV, TOOLBAR_DRAG_PREVIEW_ENV, TOOLBAR_DRAG_THROTTLE_MS_ENV,
    TOOLBAR_POINTER_LOCK_ENV,
};
use crate::{
    config::{Config, ToolbarBackendKind},
    util::Rect,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) struct WaylandRuntimeOptions {
    debug_damage_logging: bool,
    debug_toolbar_drag_logging: bool,
    debug_toolbar_color_logging: bool,
    toolbar_pointer_lock: bool,
    toolbar_drag_preview: bool,
    toolbar_drag_throttle_interval: Option<Duration>,
    toolbar_drag_handoff_delay: Duration,
    force_inline_toolbars: bool,
    toolbar_backend_override: Option<ToolbarBackendKind>,
}

impl WaylandRuntimeOptions {
    /// Captures process environment inputs once for this Wayland root.
    /// Later handler and render decisions read this owned value instead of
    /// consulting process-global caches.
    pub(in crate::backend::wayland) fn from_env() -> Self {
        Self {
            debug_damage_logging: parse_debug_damage_env(
                &std::env::var(DEBUG_DAMAGE_ENV).unwrap_or_default(),
            ),
            debug_toolbar_drag_logging: parse_boolish_env(
                &std::env::var(DEBUG_TOOLBAR_DRAG_ENV).unwrap_or_default(),
            ),
            debug_toolbar_color_logging: parse_boolish_env(
                &std::env::var(DEBUG_TOOLBAR_COLOR_ENV).unwrap_or_default(),
            ),
            // Default ON: without pointer lock, layer-shell toolbar drags
            // jitter as surfaces move beneath the pointer.
            toolbar_pointer_lock: parse_boolish_env(
                &std::env::var(TOOLBAR_POINTER_LOCK_ENV).unwrap_or_else(|_| "1".into()),
            ),
            toolbar_drag_preview: parse_boolish_env(
                &std::env::var(TOOLBAR_DRAG_PREVIEW_ENV).unwrap_or_else(|_| "1".into()),
            ),
            toolbar_drag_throttle_interval: parse_toolbar_drag_throttle_interval(
                &std::env::var(TOOLBAR_DRAG_THROTTLE_MS_ENV).unwrap_or_else(|_| "12".into()),
            ),
            toolbar_drag_handoff_delay: parse_toolbar_drag_handoff_delay(
                &std::env::var(TOOLBAR_DRAG_HANDOFF_MS_ENV).unwrap_or_else(|_| "250".into()),
            ),
            force_inline_toolbars: parse_boolish_env(
                &std::env::var(FORCE_INLINE_TOOLBARS_ENV).unwrap_or_default(),
            ),
            toolbar_backend_override: crate::toolbar_gtk::select::backend_env_override_from_env(),
        }
    }

    pub(in crate::backend::wayland) fn debug_damage_logging(self) -> bool {
        self.debug_damage_logging
    }

    pub(in crate::backend::wayland) fn debug_toolbar_drag_logging(self) -> bool {
        self.debug_toolbar_drag_logging
    }

    pub(in crate::backend::wayland) fn debug_toolbar_color_logging(self) -> bool {
        self.debug_toolbar_color_logging
    }

    pub(in crate::backend::wayland) fn toolbar_pointer_lock(self) -> bool {
        self.toolbar_pointer_lock
    }

    pub(in crate::backend::wayland) fn toolbar_drag_preview(self) -> bool {
        self.toolbar_drag_preview
    }

    pub(in crate::backend::wayland) fn toolbar_drag_throttle_interval(self) -> Option<Duration> {
        self.toolbar_drag_throttle_interval
    }

    pub(in crate::backend::wayland) fn toolbar_drag_handoff_delay(self) -> Duration {
        self.toolbar_drag_handoff_delay
    }

    pub(in crate::backend::wayland) fn force_inline_toolbars(self) -> bool {
        self.force_inline_toolbars
    }

    pub(in crate::backend::wayland) fn requested_toolbar_backend(
        self,
        config: &Config,
    ) -> ToolbarBackendKind {
        crate::toolbar_gtk::select::requested_backend_with_override(
            config,
            self.toolbar_backend_override,
        )
    }

    pub(in crate::backend::wayland) fn drag_log(self, message: impl AsRef<str>) {
        if self.debug_toolbar_drag_logging {
            log::info!("{}", message.as_ref());
        }
    }
}

pub(in crate::backend::wayland) fn surface_id(surface: &wl_surface::WlSurface) -> u32 {
    surface.id().protocol_id()
}

fn parse_toolbar_drag_throttle_interval(raw: &str) -> Option<Duration> {
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
}

fn parse_toolbar_drag_handoff_delay(raw: &str) -> Duration {
    let Ok(ms) = raw.trim().parse::<u64>() else {
        return Duration::from_millis(250);
    };
    Duration::from_millis(ms.min(500))
}

pub(in crate::backend::wayland) fn force_inline_toolbars_requested_with_env(
    config: &Config,
    env_force_inline: bool,
) -> bool {
    config.ui.toolbar.force_inline || env_force_inline
}

#[cfg(test)]
mod tests {
    use super::{parse_toolbar_drag_handoff_delay, parse_toolbar_drag_throttle_interval};
    use std::time::Duration;

    #[test]
    fn drag_throttle_parser_preserves_defaults_and_disable_value() {
        assert_eq!(
            parse_toolbar_drag_throttle_interval(""),
            Some(Duration::from_millis(12))
        );
        assert_eq!(
            parse_toolbar_drag_throttle_interval("invalid"),
            Some(Duration::from_millis(12))
        );
        assert_eq!(parse_toolbar_drag_throttle_interval("0"), None);
        assert_eq!(
            parse_toolbar_drag_throttle_interval(" 27 "),
            Some(Duration::from_millis(27))
        );
    }

    #[test]
    fn drag_handoff_parser_preserves_default_and_upper_bound() {
        assert_eq!(
            parse_toolbar_drag_handoff_delay("invalid"),
            Duration::from_millis(250)
        );
        assert_eq!(
            parse_toolbar_drag_handoff_delay(" 175 "),
            Duration::from_millis(175)
        );
        assert_eq!(
            parse_toolbar_drag_handoff_delay("900"),
            Duration::from_millis(500)
        );
    }
}
