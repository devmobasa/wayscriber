//! Layer policy and bounded XDG fullscreen placement for frozen mode.
use std::time::{Duration, Instant};

const XDG_FROZEN_FULLSCREEN_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum XdgFrozenFullscreenState {
    #[default]
    Inactive,
    PendingConfigure,
    Active,
}

#[derive(Debug, Default)]
pub(in crate::backend::wayland) struct XdgFrozenFullscreen {
    state: XdgFrozenFullscreenState,
    requested_at: Option<Instant>,
}

impl XdgFrozenFullscreen {
    pub(in crate::backend::wayland) fn request(&mut self, now: Instant) {
        self.state = XdgFrozenFullscreenState::PendingConfigure;
        self.requested_at = Some(now);
    }

    pub(in crate::backend::wayland) fn activate(&mut self) {
        self.state = XdgFrozenFullscreenState::Active;
        self.requested_at = None;
    }

    pub(in crate::backend::wayland) fn finish(&mut self) {
        self.state = XdgFrozenFullscreenState::Inactive;
        self.requested_at = None;
    }

    pub(in crate::backend::wayland) fn timeout(&self, now: Instant) -> Option<Duration> {
        if !self.pending_configure() {
            return None;
        }
        Some(
            self.requested_at
                .and_then(|requested_at| requested_at.checked_add(XDG_FROZEN_FULLSCREEN_TIMEOUT))
                .map(|deadline| deadline.saturating_duration_since(now))
                .unwrap_or(Duration::ZERO),
        )
    }

    pub(in crate::backend::wayland) fn pending_configure(&self) -> bool {
        self.state == XdgFrozenFullscreenState::PendingConfigure
    }

    pub(in crate::backend::wayland) fn requested(&self) -> bool {
        self.state != XdgFrozenFullscreenState::Inactive
    }
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct SurfacePlacement {
    preferred_output_identity: Option<String>,
    xdg_fullscreen: bool,
    main_surface_uses_overlay_layer: bool,
    xdg_frozen: XdgFrozenFullscreen,
}

impl SurfacePlacement {
    pub(in crate::backend::wayland) fn new(
        preferred_output_identity: Option<String>,
        xdg_fullscreen: bool,
        main_surface_uses_overlay_layer: bool,
    ) -> Self {
        Self {
            preferred_output_identity,
            xdg_fullscreen,
            main_surface_uses_overlay_layer,
            xdg_frozen: XdgFrozenFullscreen::default(),
        }
    }

    pub(in crate::backend::wayland) fn preferred_output_identity(&self) -> Option<&str> {
        self.preferred_output_identity.as_deref()
    }

    pub(in crate::backend::wayland) fn xdg_fullscreen(&self) -> bool {
        self.xdg_fullscreen
    }

    pub(in crate::backend::wayland) fn layer(
        &self,
    ) -> smithay_client_toolkit::shell::wlr_layer::Layer {
        if self.main_surface_uses_overlay_layer {
            smithay_client_toolkit::shell::wlr_layer::Layer::Overlay
        } else {
            smithay_client_toolkit::shell::wlr_layer::Layer::Top
        }
    }

    pub(in crate::backend::wayland) fn xdg_frozen(&self) -> &XdgFrozenFullscreen {
        &self.xdg_frozen
    }

    pub(in crate::backend::wayland) fn xdg_frozen_mut(&mut self) -> &mut XdgFrozenFullscreen {
        &mut self.xdg_frozen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_fullscreen_deadline_uses_injected_time() {
        let start = Instant::now();
        let mut state = XdgFrozenFullscreen::default();
        state.request(start);

        assert_eq!(state.timeout(start), Some(XDG_FROZEN_FULLSCREEN_TIMEOUT));
        assert_eq!(
            state.timeout(start + XDG_FROZEN_FULLSCREEN_TIMEOUT),
            Some(Duration::ZERO)
        );
        assert!(state.pending_configure());
        assert!(state.requested());
    }

    #[test]
    fn frozen_fullscreen_activate_and_finish_clear_pending_timeout() {
        let start = Instant::now();
        let mut state = XdgFrozenFullscreen::default();
        state.request(start);
        state.activate();

        assert!(state.requested());
        assert!(!state.pending_configure());
        assert_eq!(state.timeout(start + XDG_FROZEN_FULLSCREEN_TIMEOUT), None);

        state.finish();
        assert!(!state.requested());
        assert_eq!(state.timeout(start), None);
    }

    #[test]
    fn placement_keeps_output_fullscreen_and_layer_policy_together() {
        let placement = SurfacePlacement::new(Some("DP-1".to_owned()), true, true);

        assert_eq!(placement.preferred_output_identity(), Some("DP-1"));
        assert!(placement.xdg_fullscreen());
        assert_eq!(
            placement.layer(),
            smithay_client_toolkit::shell::wlr_layer::Layer::Overlay
        );
    }
}
