//! Interactive bottom-right zoom chip layout cache and click handling.
//!
//! Mirrors the status HUD (`status_hud.rs`): the layout is computed headlessly
//! once per frame (see `collect_ui_effect_damage`) and cached here, so
//! rendering, damage geometry, and pointer hit-testing all read the same cache
//! and can never disagree.
//!
//! Visibility and interactivity are gated on both the existing
//! `show_zoom_actions` toggle (the same one the Canvas popover's Zoom section
//! uses), the persisted `show_zoom_chip` master preference, and its display
//! policy. Unlike a
//! cursor-follower, the chip is a persistent fixed-corner control, so the
//! backend's `zoom_chip_visible()` gate delegates to [`InputState::zoom_chip_enabled`]:
//! it is deliberately NOT gated on cursor focus or toolbar blocking (see that
//! method for why). Only an overlay rendering above the pill suppresses hit
//! testing, via `zoom_chip_contains`'s eclipse guard.

mod state;

pub use state::ZoomChipState;

use crate::config::{Action, StatusBarStyle};
use crate::ui::{ZoomChipButtonKind, ZoomChipLayout, ZoomChipPress};

use super::base::InputState;

impl InputState {
    pub fn zoom_chip_layout(&self) -> Option<&ZoomChipLayout> {
        self.zoom_chip.layout()
    }

    /// Effective zoom-chip visibility: the `show_zoom_actions` toolbar
    /// toggle AND the persisted `Action::ToggleZoomChip` master preference AND the
    /// `zoom_chip_display` policy ("while-zoomed" keeps the corner clean at
    /// 100%). Every chip gate (layout cache, hit-testing, backend
    /// render/damage/press guards) goes through this so they can never
    /// disagree. Outside Focus Mode, fallback zoom badges show exactly when
    /// the chip does not; Focus Mode deliberately suppresses both.
    pub fn zoom_chip_enabled(&self) -> bool {
        self.zoom_chip.is_enabled(
            self.ui_visibility.show_zoom_actions,
            self.ui_visibility.show_zoom_chip,
            self.zoom_active(),
        )
    }

    /// Recompute and cache the zoom chip layout for this frame. Clears the
    /// cache when the chip is hidden.
    pub fn update_zoom_chip_layout(
        &mut self,
        style: &StatusBarStyle,
        screen_width: u32,
        screen_height: u32,
    ) {
        self.update_zoom_chip_layout_for_pointer(style, screen_width, screen_height, true);
    }

    pub(crate) fn update_zoom_chip_layout_for_pointer(
        &mut self,
        style: &StatusBarStyle,
        screen_width: u32,
        screen_height: u32,
        chrome_cursor_focused: bool,
    ) {
        crate::ui_text::with_scoped_engine(|engine| {
            self.update_zoom_chip_layout_for_pointer_with_engine(
                engine,
                style,
                screen_width,
                screen_height,
                chrome_cursor_focused,
            )
        });
    }

    pub(crate) fn update_zoom_chip_layout_for_pointer_with_engine(
        &mut self,
        engine: &crate::ui_text::UiTextEngine,
        style: &StatusBarStyle,
        screen_width: u32,
        screen_height: u32,
        chrome_cursor_focused: bool,
    ) {
        let layout = if self.zoom_chip_enabled() {
            crate::ui::compute_zoom_chip_layout_with_engine(
                engine,
                self,
                style,
                screen_width,
                screen_height,
            )
        } else {
            None
        };
        self.zoom_chip.replace_layout(layout);
        if chrome_cursor_focused {
            // The right-anchored chip changes width when optional controls such as
            // Lock appear or disappear. Reclassify from the stationary pointer so
            // the highlight and cursor do not follow an old button identity into
            // its new position after the layout shifts.
            let (pointer_x, pointer_y) = self.pointer_position();
            self.update_zoom_chip_hover_from_pointer(pointer_x, pointer_y);
        } else if self.zoom_chip.hover.take().is_some() {
            // Cached coordinates outlive pointer/stylus focus. Never resurrect
            // a highlight while the cursor is off-surface or over a toolbar.
            self.needs_redraw = true;
        }
    }

    pub fn clear_zoom_chip_layout(&mut self) {
        self.zoom_chip.clear_layout();
    }

    /// Update the hovered chip button from idle pointer motion (same
    /// contract as `update_status_hud_hover_from_pointer`: click gates +
    /// idle pointer, redraw on transitions only).
    pub(crate) fn update_zoom_chip_hover_from_pointer(&mut self, x: i32, y: i32) {
        let new_hover = if matches!(self.state, crate::input::DrawingState::Idle)
            && self.zoom_chip_contains(x, y)
        {
            self.zoom_chip_button_at(x, y)
        } else {
            None
        };
        if self.zoom_chip.update_hover(new_hover) {
            self.needs_redraw = true;
        }
    }

    /// True when the zoom chip pill is under (x, y): the press side of the
    /// press→release contract. Reports the hit without side effects;
    /// activation happens on release via [`check_zoom_chip_click`]. Gated on
    /// [`zoom_chip_enabled`] (so the chip stays absent, and clicks pass
    /// through to the canvas, when hidden) and suppressed while an overlay
    /// renders above the pill.
    ///
    /// [`check_zoom_chip_click`]: InputState::check_zoom_chip_click
    /// [`zoom_chip_enabled`]: InputState::zoom_chip_enabled
    pub(crate) fn zoom_chip_contains(&self, x: i32, y: i32) -> bool {
        self.zoom_chip.contains(
            self.zoom_chip_enabled() && !self.status_hud_eclipsed_by_overlay(),
            x,
            y,
        )
    }

    /// The zoom-chip button under (x, y), or `None` for a hit off any button
    /// (the passive `NN%` readout or the inter-piece gap) or outside the chip.
    /// Read on press to record the pressed button for the same-button release
    /// contract.
    pub(crate) fn zoom_chip_button_at(&self, x: i32, y: i32) -> Option<ZoomChipButtonKind> {
        self.zoom_chip.button_at(x, y)
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
        self.zoom_chip.set_press_pending(pressed);
    }

    /// Clears the internal chip press flag (called at the start of press
    /// routing so a stale flag can never swallow an unrelated release).
    pub(in crate::input::state) fn clear_zoom_chip_press_pending(&mut self) {
        self.zoom_chip.clear_press_pending();
    }

    /// Takes the internal chip press flag set by
    /// [`set_zoom_chip_press_pending`], leaving `None` behind.
    ///
    /// [`set_zoom_chip_press_pending`]: InputState::set_zoom_chip_press_pending
    pub(in crate::input::state) fn take_zoom_chip_press_pending(&mut self) -> ZoomChipPress {
        self.zoom_chip.take_press_pending()
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
