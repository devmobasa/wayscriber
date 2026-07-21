//! Status HUD (interactive status bar) layout cache and click handling.
//!
//! The layout is computed headlessly once per frame (see
//! `collect_ui_effect_damage`) and cached here, mirroring the board picker:
//! rendering, damage geometry, and pointer hit-testing all read the same
//! cache so they can never disagree.

use crate::config::{Action, StatusBarStyle, StatusPosition};
use crate::ui::{StatusHudLayout, StatusHudSegmentKind, compute_status_hud_layout};

use super::base::InputState;
use super::board_picker::BoardPickerFocus;

impl InputState {
    pub fn status_hud_layout(&self) -> Option<&StatusHudLayout> {
        self.status_hud_layout.as_ref()
    }

    /// Recompute and cache the status HUD layout for this frame. Clears the
    /// cache when the status bar is hidden.
    pub fn update_status_hud_layout(
        &mut self,
        position: StatusPosition,
        style: &StatusBarStyle,
        screen_width: u32,
        screen_height: u32,
    ) {
        self.status_hud_layout = if self.show_status_bar {
            compute_status_hud_layout(self, position, style, screen_width, screen_height)
        } else {
            None
        };
    }

    pub fn clear_status_hud_layout(&mut self) {
        self.status_hud_layout = None;
    }

    /// True while an overlay that renders above the status HUD is open. The
    /// HUD must not consume presses then: the radial menu, color picker
    /// popup, board picker, properties panel, and context menu all draw over
    /// the pill and handle their own presses later in the routing chain, so
    /// a HUD hit here would eclipse them (e.g. radial-ring clicks over the
    /// pill re-firing as chip activations). The command palette and tour are
    /// already intercepted earlier in the backend chain; they are included
    /// as belt-and-braces for paths that route presses directly.
    ///
    /// Shared with the bottom-right zoom chip: the same overlays render above
    /// both bottom-anchored interactive chrome surfaces.
    pub(in crate::input::state) fn status_hud_eclipsed_by_overlay(&self) -> bool {
        self.is_radial_menu_open()
            || self.is_color_picker_popup_open()
            || self.is_board_picker_open()
            || self.is_properties_panel_open()
            || self.is_context_menu_open()
            || self.command_palette_open
            || self.tour_active
    }

    /// True when the interactive status HUD pill is under (x, y): the press
    /// side of the press→release contract. Reports the hit without side
    /// effects; activation happens on release via [`check_status_hud_click`].
    /// Always false when `[ui] status_bar_interactive = false`, so the HUD
    /// stays a pure display and clicks pass through to the canvas, and while
    /// an overlay renders above the pill (see
    /// [`status_hud_eclipsed_by_overlay`]).
    ///
    /// [`check_status_hud_click`]: InputState::check_status_hud_click
    /// [`status_hud_eclipsed_by_overlay`]: InputState::status_hud_eclipsed_by_overlay
    pub(crate) fn status_hud_contains(&self, x: i32, y: i32) -> bool {
        self.status_bar_interactive
            && self.show_status_bar
            && !self.status_hud_eclipsed_by_overlay()
            && self
                .status_hud_layout
                .as_ref()
                .is_some_and(|layout| layout.pill_contains(x as f64, y as f64))
    }

    /// Marks a HUD press consumed by the internal routing chain (tablet and
    /// other paths that bypass the backend's own pending flag); the matching
    /// release activates via [`take_status_hud_press_pending`].
    ///
    /// [`take_status_hud_press_pending`]: InputState::take_status_hud_press_pending
    pub(in crate::input::state) fn set_status_hud_press_pending(&mut self) {
        self.status_hud_press_pending = true;
    }

    /// Clears the internal HUD press flag (called at the start of press
    /// routing so a stale flag can never swallow an unrelated release).
    pub(in crate::input::state) fn clear_status_hud_press_pending(&mut self) {
        self.status_hud_press_pending = false;
    }

    /// Takes the internal HUD press flag set by
    /// [`set_status_hud_press_pending`].
    ///
    /// [`set_status_hud_press_pending`]: InputState::set_status_hud_press_pending
    pub(in crate::input::state) fn take_status_hud_press_pending(&mut self) -> bool {
        std::mem::take(&mut self.status_hud_press_pending)
    }

    /// Check a release at (x, y) against the status HUD segments. On a
    /// segment hit, opens the matching surface directly (board picker, color
    /// picker popup, radial menu at the pointer) and/or returns the action
    /// for the backend to dispatch (help). Returns `(hit, action)` mirroring
    /// toast release resolver.
    pub(crate) fn check_status_hud_click(&mut self, x: i32, y: i32) -> (bool, Option<Action>) {
        // `status_hud_contains` also applies the open-overlay guard, so a
        // release cannot activate a chip when an overlay opened between the
        // press and the release.
        if !self.status_hud_contains(x, y) {
            return (false, None);
        }
        let Some(kind) = self
            .status_hud_layout
            .as_ref()
            .and_then(|layout| layout.segment_at(x as f64, y as f64))
        else {
            // Inside the pill but between segments: consume without action.
            return (true, None);
        };
        match kind {
            StatusHudSegmentKind::Board => {
                self.toggle_board_picker();
                (true, None)
            }
            StatusHudSegmentKind::Page => {
                // The page panel lives inside the board picker; focus it so
                // the Page chip is distinguishable from the Board chip (the
                // setter also scrolls the panel to the active page).
                if !self.is_board_picker_open() {
                    self.open_board_picker();
                }
                self.board_picker_set_focus(BoardPickerFocus::PagePanel);
                (true, None)
            }
            StatusHudSegmentKind::Color => {
                self.open_color_picker_popup();
                (true, None)
            }
            StatusHudSegmentKind::Tool => {
                self.toggle_radial_menu(x as f64, y as f64);
                (true, None)
            }
            StatusHudSegmentKind::Help => (true, Some(Action::ToggleHelp)),
        }
    }
}
