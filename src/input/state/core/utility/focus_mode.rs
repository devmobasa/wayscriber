//! Focus mode: one action that clears every persistent chrome surface
//! (toolbars, status bar, floating board/page badge, zoom chip) at once and
//! restores the exact prior visibility on the second press.
//!
//! Mirrors the presenter-mode snapshot pattern (`presenter_mode.rs`), but
//! for chrome only: no tool override, no click highlight, no config
//! involvement — a pure runtime "clean screen" switch for recording or
//! screenshots. Manual chrome toggles while active take ownership via
//! [`InputState::break_focus_mode`], so exiting later can never stomp an
//! explicit user choice.

use super::super::base::{FocusModeRestore, InputState};
use crate::domain::Action;
use crate::input::state::{Toast, ToastPriority};

const FOCUS_MODE_TOAST_KEY: &str = "focus.mode";

impl InputState {
    /// True while a focus-mode snapshot is waiting to be restored.
    pub fn focus_mode_active(&self) -> bool {
        self.focus_mode_restore.is_some()
    }

    /// Whether passive fallback mode badges may render. Focus Mode suppresses
    /// these separately from their normal status-bar/chip visibility gates so
    /// hiding the persistent controls cannot make zoom, frozen, pan, or text
    /// editing chrome reappear through the fallback path.
    pub(crate) fn fallback_mode_badges_visible(&self) -> bool {
        !self.focus_mode_active()
    }

    /// Whether a transient mode can currently contribute visible fallback
    /// chrome while the status HUD is hidden. The frozen preference itself is
    /// backend-owned, so treat an active freeze conservatively: choosing the
    /// hide arm is safer than making a possibly visible badge trigger rescue.
    fn fallback_mode_badge_may_be_active(&self) -> bool {
        !self.show_status_bar
            && (self.zoom_active()
                || self.frozen_active()
                || (self.boards.pan_enabled()
                    && self.boards.show_pan_badge()
                    && !self.board_is_transparent())
                || (matches!(self.state, crate::input::DrawingState::TextInput { .. })
                    && self.text_edit_target.is_some()))
    }

    /// Apply a status-bar value authored by a preset or session restore.
    /// While Focus Mode owns chrome, update the value waiting behind its
    /// suppression instead of making the bar visible or leaving a stale
    /// snapshot that would overwrite the authored value on exit.
    pub(crate) fn set_status_bar_visibility_preserving_focus(&mut self, show: bool) -> bool {
        if let Some(restore) = self.focus_mode_restore.as_mut() {
            let changed = restore.show_status_bar != show;
            restore.show_status_bar = show;
            return changed;
        }
        let changed = self.show_status_bar != show;
        self.show_status_bar = show;
        changed
    }

    /// A manual chrome toggle takes ownership of visibility: drop the
    /// snapshot (without restoring it) so a later focus-mode exit cannot
    /// override what the user just chose by hand. The next
    /// `ToggleFocusMode` starts from a fresh snapshot.
    pub(crate) fn break_focus_mode(&mut self) {
        self.focus_mode_restore = None;
        self.clear_focus_mode_toast();
    }

    fn clear_focus_mode_toast(&mut self) {
        // The Restore action can be queued behind a higher-priority warning,
        // so retract it from both slots whenever Focus no longer owns chrome.
        let active_removed = self
            .toast_queue
            .remove_matching(&mut self.ui_toast, |key, action| {
                key == FOCUS_MODE_TOAST_KEY || action == Some(Action::ToggleFocusMode)
            });
        if active_removed {
            self.ui_toast_bounds = None;
            self.needs_redraw = true;
        }
    }

    fn clear_all_chrome_recovery_toast(&mut self) {
        // The generic all-chrome warning deliberately shares the routine
        // `ui` key so it can replace "Toolbar: hidden" in place. Match the
        // recovery action as well: after the rescue arm shows the toolbar,
        // either toast's Show action would immediately hide it again.
        let active_removed = self
            .toast_queue
            .remove_matching(&mut self.ui_toast, |key, action| {
                key == "ui" && action == Some(Action::ToggleToolbar)
            });
        if active_removed {
            self.ui_toast_bounds = None;
            self.needs_redraw = true;
        }
    }

    /// Toggle focus mode:
    /// - snapshot present → restore it (chrome returns exactly as it was,
    ///   including a micro top strip);
    /// - chrome visible → snapshot and hide everything;
    /// - nothing visible and no snapshot → show everything (rescue arm, so
    ///   the action always has a visible effect).
    pub(crate) fn toggle_focus_mode(&mut self) {
        if self.light_mode {
            self.exit_light_mode();
        }
        if let Some(restore) = self.focus_mode_restore.take() {
            self.clear_focus_mode_toast();
            let status_changed = self.show_status_bar != restore.show_status_bar;
            self.show_status_bar = restore.show_status_bar;
            self.toolbar_visible = restore.toolbar_visible;
            self.toolbar_top_visible = restore.toolbar_top_visible;
            self.toolbar_side_visible = restore.toolbar_side_visible;
            self.toolbar_top_display_mode = restore.toolbar_top_display_mode;
            self.show_floating_badge = restore.show_floating_badge;
            self.show_zoom_chip = restore.show_zoom_chip;
            if status_changed {
                self.mark_session_dirty();
            }
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return;
        }

        // Only surfaces that can actually be on screen count: with all of
        // them already gone, "enter focus mode" would be a confusing no-op
        // snapshot of nothing — restore the full UI instead.
        let anything_to_hide = self.toolbar_visible()
            || self.show_status_bar
            || self.floating_badge_visible()
            || self.zoom_chip_enabled()
            || self.fallback_mode_badge_may_be_active();
        if !anything_to_hide {
            self.clear_all_chrome_recovery_toast();
            self.set_toolbar_visible(true);
            self.show_status_bar = true;
            self.show_floating_badge = true;
            self.show_zoom_chip = true;
            self.mark_session_dirty();
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return;
        }

        let restore = FocusModeRestore {
            show_status_bar: self.show_status_bar,
            toolbar_visible: self.toolbar_visible,
            toolbar_top_visible: self.toolbar_top_visible,
            toolbar_side_visible: self.toolbar_side_visible,
            toolbar_top_display_mode: self.toolbar_top_display_mode,
            show_floating_badge: self.show_floating_badge,
            show_zoom_chip: self.show_zoom_chip,
        };
        let status_changed = self.show_status_bar;
        // Raw flags only: the display mode stays untouched so a micro strip
        // comes back as micro on restore.
        self.toolbar_visible = false;
        self.toolbar_top_visible = false;
        self.toolbar_side_visible = false;
        self.show_status_bar = false;
        self.show_floating_badge = false;
        self.show_zoom_chip = false;
        self.focus_mode_restore = Some(restore);

        // Focus mode teaches its own way back (instead of the generic
        // all-chrome warning): one press restores everything.
        let label = match self.action_binding_primary_label(Action::ToggleFocusMode) {
            Some(binding) => format!("Restore ({binding})"),
            None => "Restore".to_string(),
        };
        self.push_toast(
            ToastPriority::Info,
            FOCUS_MODE_TOAST_KEY,
            Toast::info("Focus mode — UI hidden").action(label, Action::ToggleFocusMode),
        );
        if status_changed {
            self.mark_session_dirty();
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }
}
