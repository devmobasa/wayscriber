//! Interactive bottom-right zoom chip layout cache and click handling.
//!
//! Mirrors the status HUD (`status_hud.rs`): the layout is computed headlessly
//! once per frame (see `collect_ui_effect_damage`) and cached here, so
//! rendering, damage geometry, and pointer hit-testing all read the same cache
//! and can never disagree.
//!
//! Visibility and interactivity are gated on the existing `show_zoom_actions`
//! toggle (the same one the Canvas popover's Zoom section uses) — there is no
//! separate config key. Unlike a cursor-follower, the chip is a persistent
//! fixed-corner control, so the backend's `zoom_chip_visible()` gate is just
//! `show_zoom_actions`: it is deliberately NOT gated on cursor focus or
//! toolbar blocking (see that method for why). Only an overlay rendering above
//! the pill suppresses it, via `zoom_chip_contains`'s eclipse guard.

use crate::config::{Action, StatusBarStyle};
use crate::ui::{ZoomChipButtonKind, ZoomChipLayout, ZoomChipPress, compute_zoom_chip_layout};

use super::base::InputState;

impl InputState {
    pub fn zoom_chip_layout(&self) -> Option<&ZoomChipLayout> {
        self.zoom_chip_layout.as_ref()
    }

    /// Recompute and cache the zoom chip layout for this frame. Clears the
    /// cache when zoom actions are hidden.
    pub fn update_zoom_chip_layout(
        &mut self,
        style: &StatusBarStyle,
        screen_width: u32,
        screen_height: u32,
    ) {
        self.zoom_chip_layout = if self.show_zoom_actions {
            compute_zoom_chip_layout(self, style, screen_width, screen_height)
        } else {
            None
        };
    }

    pub fn clear_zoom_chip_layout(&mut self) {
        self.zoom_chip_layout = None;
    }

    /// True when the zoom chip pill is under (x, y): the press side of the
    /// press→release contract. Reports the hit without side effects;
    /// activation happens on release via [`check_zoom_chip_click`]. Gated on
    /// `show_zoom_actions` (so the chip stays absent, and clicks pass through
    /// to the canvas, when the toggle is off) and suppressed while an overlay
    /// renders above the pill.
    ///
    /// [`check_zoom_chip_click`]: InputState::check_zoom_chip_click
    pub(crate) fn zoom_chip_contains(&self, x: i32, y: i32) -> bool {
        self.show_zoom_actions
            && !self.status_hud_eclipsed_by_overlay()
            && self
                .zoom_chip_layout
                .as_ref()
                .is_some_and(|layout| layout.chip_contains(x as f64, y as f64))
    }

    /// The zoom-chip button under (x, y), or `None` for a hit off any button
    /// (the passive `NN%` readout or the inter-piece gap) or outside the chip.
    /// Read on press to record the pressed button for the same-button release
    /// contract.
    pub(crate) fn zoom_chip_button_at(&self, x: i32, y: i32) -> Option<ZoomChipButtonKind> {
        self.zoom_chip_layout
            .as_ref()
            .and_then(|layout| layout.button_at(x as f64, y as f64))
    }

    /// Classify a press at (x, y) into the three-state [`ZoomChipPress`] the
    /// press→release contract records: `Button(kind)` when it lands on an
    /// actionable button, `Passive` when it lands inside the pill but off every
    /// button (the passive `NN%` readout or an inter-piece gap), and `None`
    /// when it is outside the chip. Callers generally gate on
    /// [`zoom_chip_contains`] first (to decide whether to swallow the press),
    /// but the `None` arm keeps this self-consistent either way.
    ///
    /// [`zoom_chip_contains`]: InputState::zoom_chip_contains
    pub(crate) fn zoom_chip_press_at(&self, x: i32, y: i32) -> ZoomChipPress {
        if !self.zoom_chip_contains(x, y) {
            return ZoomChipPress::None;
        }
        match self.zoom_chip_button_at(x, y) {
            Some(kind) => ZoomChipPress::Button(kind),
            None => ZoomChipPress::Passive,
        }
    }

    /// Records a chip press consumed by the internal routing chain (tablet and
    /// other paths that bypass the backend's own pending flag). `Passive` (the
    /// `NN%` readout / inter-piece gap) and `Button(kind)` both keep the
    /// matching release consumed; only a `Button` release that lands on the
    /// SAME button activates, via [`take_zoom_chip_press_pending`].
    ///
    /// [`take_zoom_chip_press_pending`]: InputState::take_zoom_chip_press_pending
    pub(in crate::input::state) fn set_zoom_chip_press_pending(&mut self, pressed: ZoomChipPress) {
        self.zoom_chip_press_pending = pressed;
    }

    /// Clears the internal chip press flag (called at the start of press
    /// routing so a stale flag can never swallow an unrelated release).
    pub(in crate::input::state) fn clear_zoom_chip_press_pending(&mut self) {
        self.zoom_chip_press_pending = ZoomChipPress::None;
    }

    /// Takes the internal chip press flag set by
    /// [`set_zoom_chip_press_pending`], leaving `None` behind.
    ///
    /// [`set_zoom_chip_press_pending`]: InputState::set_zoom_chip_press_pending
    pub(in crate::input::state) fn take_zoom_chip_press_pending(&mut self) -> ZoomChipPress {
        std::mem::replace(&mut self.zoom_chip_press_pending, ZoomChipPress::None)
    }

    /// Check a release at (x, y) against the pressed zoom chip button, enforcing
    /// the same-button contract: the release fires only when it lands on the
    /// SAME `pressed` button the press recorded (mirroring the status HUD /
    /// toast same-target contracts). On a match, returns the zoom [`Action`] for
    /// the caller to dispatch through the shared action path. Returns
    /// `(hit, action)` mirroring `check_status_hud_click`: a hit inside the pill
    /// but not on the pressed button (a drag to a different button, the `NN%`
    /// readout, or the inter-piece gap) reports `(true, None)`; a release
    /// outside the pill (or under a newly-opened overlay) reports
    /// `(false, None)`.
    ///
    /// This resolves only the `Button` arm of the three-state
    /// [`ZoomChipPress`]. Whether the release is *consumed* is decided by the
    /// caller from the pending value ([`ZoomChipPress::is_pending`]) — a
    /// `Passive` press has no button to resolve here, yet its release is still
    /// consumed by the chip.
    pub(crate) fn check_zoom_chip_click(
        &mut self,
        pressed: ZoomChipButtonKind,
        x: i32,
        y: i32,
    ) -> (bool, Option<Action>) {
        // `zoom_chip_contains` also applies the open-overlay guard, so a
        // release cannot activate a button when an overlay opened between the
        // press and the release.
        if !self.zoom_chip_contains(x, y) {
            return (false, None);
        }
        // Same-button contract: a release on any button other than the pressed
        // one (or off every button) is consumed without an action.
        if self.zoom_chip_button_at(x, y) != Some(pressed) {
            return (true, None);
        }
        let action = match pressed {
            ZoomChipButtonKind::Out => Action::ZoomOut,
            ZoomChipButtonKind::In => Action::ZoomIn,
            ZoomChipButtonKind::Fit => Action::ResetZoom,
            ZoomChipButtonKind::Lock => Action::ToggleZoomLock,
        };
        // Shortcut-coach slow-path signal: activating a zoom action from the
        // chip is the same "you could have pressed the key" case the toolbar
        // (`apply_toolbar_event`) and command palette record. The chip resolves
        // to an `Action` the backend dispatches through the shared action path
        // (`handle_action`) — the fast/keyboard path, which never feeds the
        // coach — so this InputState-level seam is where the nudge is recorded.
        // Only actions that resolve to a shortcut count, so the coach can always
        // name the key.
        if self.shortcut_for_action(action).is_some() {
            self.pending_onboarding_usage
                .note_shortcut_slow_path(action);
        }
        (true, Some(action))
    }
}
