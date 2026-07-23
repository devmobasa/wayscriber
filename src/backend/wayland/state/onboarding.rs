use crate::config::keybindings::Action;
use crate::input::state::{Toast, ToastPriority};
use crate::onboarding::DEFERRED_HINT_REPEAT_MAX;
use std::time::{Duration, Instant};

use super::*;

mod first_run;

/// Slow-path threshold: this many shortcut-bound command-palette runs of the
/// same action before the coach offers the keyboard shortcut.
const SHORTCUT_COACH_THRESHOLD: u32 = 3;
/// Minimum wall-clock gap between two coach hints in one session.
const SHORTCUT_COACH_COOLDOWN: Duration = Duration::from_secs(90);
/// Maximum coach hints shown per session.
const SHORTCUT_COACH_SESSION_CAP: u32 = 2;

const PANEL_DEPRECATION_MESSAGE: &str = "Side panel deprecated \u{2014} use the new homes:\n\
     Drawing \u{2192} style pill \u{b7} Canvas \u{2192} \u{201c}Canvas\u{2026}\u{201d} / zoom chip / status bar\n\
     Presets \u{2192} top strip \u{b7} Session/Settings \u{2192} overflow";

/// The visible status-HUD entry that can open the board picker. The hint must
/// not advertise the pill when both configurable Board/Page segments are
/// absent: the remaining color/tool/help segments perform different actions.
fn status_bar_board_picker_entry(input: &crate::input::InputState) -> Option<&'static str> {
    if !input.show_status_bar || !input.status_bar_interactive || input.boards.board_count() <= 1 {
        return None;
    }

    let board = input.show_status_board_badge && input.boards.show_badge();
    let page = input.show_status_page_badge;
    match (board, page) {
        (true, true) => Some("Board or Page"),
        (true, false) => Some("Board"),
        (false, true) => Some("Page"),
        (false, false) => None,
    }
}

/// Whether the top-strip overflow that owns the Canvas popover is reachable.
fn canvas_popover_hint_relevant(input: &crate::input::InputState) -> bool {
    input.toolbar_top_visible()
        && !input.toolbar_top_minimized
        && input.toolbar_top_display_mode == crate::config::TopDisplayMode::Full
}

/// Per-session shortcut-coach accumulator. Session-only (never persisted): the
/// across-session cap and learned-suppression live in `OnboardingState`.
#[derive(Debug, Default)]
pub(super) struct ShortcutCoachSession {
    /// Action currently accumulating consecutive slow-path uses.
    tracked_action: Option<Action>,
    /// Consecutive slow-path uses of `tracked_action`.
    streak: u32,
    /// When the last coach hint fired (drives the cooldown gate).
    last_hint_at: Option<Instant>,
    /// Coach hints shown this session (per-session cap).
    hints_this_session: u32,
}

impl ShortcutCoachSession {
    /// Fold this tick's slow-path signal into the streak. A different action
    /// resets the streak: the coach nudges a sustained habit, not one-offs.
    fn record(&mut self, action: Action, repeats: u32) {
        if repeats == 0 {
            return;
        }
        if self.tracked_action != Some(action) {
            self.tracked_action = Some(action);
            self.streak = 0;
        }
        self.streak = self.streak.saturating_add(repeats);
    }

    /// Reset the streak — after a hint fires, or when the tracked action's
    /// shortcut can no longer be resolved.
    fn clear_streak(&mut self) {
        self.tracked_action = None;
        self.streak = 0;
    }
}

/// Pure gate for the shortcut coach — threshold, per-session cap, across-session
/// cap, learned suppression, and cooldown — so each rule is unit-testable
/// without a `WaylandState`.
pub(super) fn shortcut_coach_should_fire(
    streak: u32,
    hints_this_session: u32,
    coach_hint_shown: bool,
    coach_hint_count: u32,
    last_hint_at: Option<Instant>,
    now: Instant,
) -> bool {
    if coach_hint_shown {
        return false; // learned -> permanently suppressed
    }
    if coach_hint_count >= DEFERRED_HINT_REPEAT_MAX {
        return false; // across-session cap
    }
    if hints_this_session >= SHORTCUT_COACH_SESSION_CAP {
        return false; // per-session cap
    }
    if streak < SHORTCUT_COACH_THRESHOLD {
        return false; // threshold not reached
    }
    match last_hint_at {
        Some(at) => now.saturating_duration_since(at) >= SHORTCUT_COACH_COOLDOWN,
        None => true,
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn apply_onboarding_hints(&mut self) {
        // Show capability warning toast first if applicable.
        self.apply_capability_toast();
        // Honest legacy notice: nudge panel-mode users toward the new homes.
        self.apply_panel_deprecation_notice();
        // Capture the coach's slow-path signal before apply_first_run_progress
        // drains pending_onboarding_usage.
        let coach_slow_path = self
            .input_state
            .pending_onboarding_usage
            .shortcut_slow_path_action
            .map(|action| {
                (
                    action,
                    self.input_state
                        .pending_onboarding_usage
                        .shortcut_slow_path_repeats,
                )
            });
        self.apply_first_run_progress();
        self.apply_shortcut_coach(coach_slow_path);
        self.apply_contextual_feature_hints();
        self.apply_toolbar_visibility_hint();
    }

    /// Shortcut coach: when the user keeps invoking a shortcut-bound action via
    /// a slow path — the command palette or the toolbar — nudge them toward the
    /// key. Gated by a repeat threshold, a per-session cap, a cooldown, an
    /// across-session cap, and a learned-suppression flag.
    fn apply_shortcut_coach(&mut self, slow_path: Option<(Action, u32)>) {
        // Only coach after first-run onboarding: during onboarding the palette
        // and toolbar are being taught, so slow-path use there is expected.
        if !self.onboarding.state().first_run_completed {
            return;
        }

        // Fold this tick's slow-path signal into the streak first — even behind
        // a modal — so a sustained habit still accumulates toward the threshold.
        if let Some((action, repeats)) = slow_path {
            self.data.shortcut_coach.record(action, repeats);
        }

        // Never compete with real feedback or interrupt a modal overlay.
        if !self.surface.is_configured() || self.overlay_suppressed() {
            return;
        }
        if self.input_state.presenter_mode
            || self.input_state.show_help
            || self.input_state.command_palette_open
            || self.input_state.tour_active
        {
            return;
        }
        if !self.input_state.toasts_idle() {
            return;
        }

        let now = Instant::now();
        let should_fire = {
            let session = &self.data.shortcut_coach;
            let state = self.onboarding.state();
            shortcut_coach_should_fire(
                session.streak,
                session.hints_this_session,
                state.coach_hint_shown,
                state.coach_hint_count,
                session.last_hint_at,
                now,
            )
        };
        if !should_fire {
            return;
        }

        let Some(action) = self.data.shortcut_coach.tracked_action else {
            return;
        };
        let Some(shortcut) = self.input_state.shortcut_for_action(action) else {
            // The shortcut was unbound since we started counting; drop the streak.
            self.data.shortcut_coach.clear_streak();
            return;
        };

        let message = format!(
            "Tip: press {shortcut} for {}.",
            crate::config::action_label(action)
        );
        let outcome = self.input_state.push_toast(
            ToastPriority::Hint,
            "onboarding.coach",
            Toast::info(message),
        );
        if outcome.accepted() {
            let session = &mut self.data.shortcut_coach;
            session.last_hint_at = Some(now);
            session.hints_this_session = session.hints_this_session.saturating_add(1);
            session.clear_streak();

            let state = self.onboarding.state_mut();
            state.coach_hint_count = state.coach_hint_count.saturating_add(1);
            if state.coach_hint_count >= DEFERRED_HINT_REPEAT_MAX {
                state.coach_hint_shown = true;
            }
            self.onboarding.save();
        }
    }

    fn apply_contextual_feature_hints(&mut self) {
        if !self.surface.is_configured() || self.overlay_suppressed() {
            return;
        }
        if self.input_state.presenter_mode
            || self.input_state.show_help
            || self.input_state.command_palette_open
            || self.input_state.tour_active
        {
            return;
        }
        // Hints never compete with real feedback: only fire when no toast is
        // visible and nothing is queued (the Hint priority would be rejected
        // otherwise and the shown/count bookkeeping below would burn a slot).
        if !self.input_state.toasts_idle() {
            return;
        }

        // Read the on-screen presence of each new surface before borrowing the
        // onboarding state mutably: a surface hint must only fire when that
        // surface is actually visible. The board-picker entry point (the status
        // bar) is only meaningful when the bar is interactive and there is more
        // than one board to switch between.
        let status_bar_entry = status_bar_board_picker_entry(&self.input_state);
        // Effective visibility: the runtime ToggleZoomChip hide must not
        // leave onboarding advertising nonexistent lower-right controls.
        let zoom_chip_present = self.input_state.zoom_chip_enabled();
        // The Canvas popover opens from the top-strip "…" overflow, which is
        // only reachable when the top strip is shown (not toggled off via
        // ToggleToolbar) and in full display (not collapsed to the micro chip).
        let canvas_hint_relevant = canvas_popover_hint_relevant(&self.input_state);

        let mut changed = false;
        let mut hint_kind: Option<&'static str> = None;
        {
            let state = self.onboarding.state_mut();
            if !state.first_run_completed {
                return;
            }
            if state.sessions_seen >= 2
                && !state.used_help_overlay
                && !state.hint_help_shown
                && state.hint_help_count < DEFERRED_HINT_REPEAT_MAX
            {
                state.hint_help_shown = true;
                state.hint_help_count = state.hint_help_count.saturating_add(1);
                changed = true;
                hint_kind = Some("help");
            } else if state.sessions_seen >= 3
                && !state.used_command_palette
                && !state.hint_palette_shown
                && state.hint_palette_count < DEFERRED_HINT_REPEAT_MAX
            {
                state.hint_palette_shown = true;
                state.hint_palette_count = state.hint_palette_count.saturating_add(1);
                changed = true;
                hint_kind = Some("palette");
            } else if state.sessions_seen >= 2
                && !state.used_radial_menu
                && !state.used_context_menu_right_click
                && !state.used_context_menu_keyboard
                && !state.hint_quick_access_shown
                && state.hint_quick_access_count < DEFERRED_HINT_REPEAT_MAX
            {
                state.hint_quick_access_shown = true;
                state.hint_quick_access_count = state.hint_quick_access_count.saturating_add(1);
                changed = true;
                hint_kind = Some("quick_access");
            // M9 surface hints, staggered across sessions so they never all
            // fire at once (the else-if chain already limits it to one per tick,
            // and the no-active-toast gate keeps them from clobbering). The
            // status bar is the most important of the three — the board picker's
            // on-screen entry point is easy to miss — so it comes first and at
            // the earliest threshold among the new surfaces.
            } else if status_bar_entry.is_some()
                && state.sessions_seen >= 3
                && !state.hint_status_bar_shown
                && state.hint_status_bar_count < DEFERRED_HINT_REPEAT_MAX
            {
                state.hint_status_bar_shown = true;
                state.hint_status_bar_count = state.hint_status_bar_count.saturating_add(1);
                changed = true;
                hint_kind = Some("status_bar");
            } else if canvas_hint_relevant
                && state.sessions_seen >= 5
                && !state.hint_canvas_popover_shown
                && state.hint_canvas_popover_count < DEFERRED_HINT_REPEAT_MAX
            {
                state.hint_canvas_popover_shown = true;
                state.hint_canvas_popover_count = state.hint_canvas_popover_count.saturating_add(1);
                changed = true;
                hint_kind = Some("canvas_popover");
            } else if zoom_chip_present
                && state.sessions_seen >= 7
                && !state.hint_zoom_chip_shown
                && state.hint_zoom_chip_count < DEFERRED_HINT_REPEAT_MAX
            {
                state.hint_zoom_chip_shown = true;
                state.hint_zoom_chip_count = state.hint_zoom_chip_count.saturating_add(1);
                changed = true;
                hint_kind = Some("zoom_chip");
            }
        }

        if changed {
            self.onboarding.save();
        }
        if let Some(kind) = hint_kind {
            let message = match kind {
                "help" => format!(
                    "Press {} for all shortcuts.",
                    self.shortcut_label(Action::ToggleHelp, "Help")
                ),
                "palette" => format!(
                    "Press {} to search actions.",
                    self.shortcut_label(Action::ToggleCommandPalette, "Command Palette")
                ),
                "status_bar" => format!(
                    "Click the {} segment in the status bar to switch boards and pages.",
                    status_bar_entry.expect("status-bar hint requires a visible picker entry")
                ),
                "canvas_popover" => {
                    "Open \u{201c}Canvas\u{2026}\u{201d} from the \u{2026} overflow for boards, \
                     pages, zoom, and advanced controls."
                        .to_string()
                }
                "zoom_chip" => match self.shortcut_label_opt(Action::ZoomIn) {
                    Some(key) => {
                        format!("Zoom from the chip in the bottom-right corner, or press {key}.")
                    }
                    None => "Zoom from the chip in the bottom-right corner.".to_string(),
                },
                _ => {
                    let context = self.shortcut_label_opt(Action::OpenContextMenu);
                    let radial = self.shortcut_label_opt(Action::ToggleRadialMenu);
                    match (context, radial) {
                        (Some(c), Some(r)) => format!("Try quick access: {c} or {r}."),
                        (Some(c), None) => format!("Try quick access: {c}."),
                        (None, Some(r)) => format!("Try quick access: {r}."),
                        (None, None) => {
                            "Quick-access menus are available from toolbar actions.".to_string()
                        }
                    }
                }
            };
            self.input_state.push_toast(
                ToastPriority::Hint,
                "onboarding.hint",
                Toast::info(message),
            );
        }
    }

    fn apply_toolbar_visibility_hint(&mut self) {
        if self.onboarding.state().toolbar_hint_shown {
            return;
        }
        if !self.surface.is_configured() || self.overlay_suppressed() {
            return;
        }
        if self.input_state.presenter_mode || self.input_state.show_help {
            return;
        }
        if self.onboarding.state().first_run_active() {
            return;
        }
        if self.input_state.toolbar_visible() || !self.input_state.toasts_idle() {
            return;
        }

        let toolbar_binding = self.shortcut_label(Action::ToggleToolbar, "Toggle toolbar");
        let outcome = self.input_state.push_toast(
            ToastPriority::Hint,
            "onboarding.toolbar",
            Toast::info("Toolbars hidden")
                .action(format!("Show ({toolbar_binding})"), Action::ToggleToolbar),
        );
        if outcome.accepted() {
            self.onboarding.state_mut().toolbar_hint_shown = true;
            self.onboarding.save();
        }
    }

    fn shortcut_label(&self, action: Action, fallback: &str) -> String {
        self.shortcut_label_opt(action)
            .unwrap_or_else(|| fallback.to_string())
    }

    fn shortcut_label_opt(&self, action: Action) -> Option<String> {
        self.input_state.shortcut_for_action(action)
    }

    /// Warn about limited compositor features: once per session unless the
    /// detected capabilities change (#156 — the queue's once-per-content rate
    /// limit keeps a re-push of the same summary from re-showing it).
    fn apply_capability_toast(&mut self) {
        if !self.config.ui.show_capabilities_warning {
            return;
        }
        if !self.surface.is_configured() {
            return;
        }

        let caps = self.input_state.compositor_capabilities;
        let message = capability_toast_message(self.input_state.capability_toast_caps, caps);
        self.input_state.capability_toast_caps = Some(caps);

        if let Some(message) = message {
            self.input_state.push_toast(
                ToastPriority::Critical,
                "capability.limitations",
                Toast::warning(message).once_per_content(),
            );
        }
    }

    /// Left-panel deprecation, made honest: when the user is running the
    /// deprecated legacy side palette (`side_layout = "panel"`), show a
    /// once-per-session notice pointing at the concrete new homes for each
    /// retired pane. Reuses the capability-toast once-per-session pattern: the
    /// session-only `*_shown` flag prevents repeats within one launch. No config
    /// key gates it: the notice only appears at all under the deprecated layout
    /// the user opted into.
    fn apply_panel_deprecation_notice(&mut self) {
        // Only the deprecated legacy side palette; the pill default already
        // re-homed everything, so there is nothing to explain there.
        if self.input_state.toolbar_side_layout != crate::config::ToolbarSideLayout::Panel {
            return;
        }
        if self.data.panel_deprecation_notice_shown {
            return; // once per session
        }
        if !self.surface.is_configured() || self.overlay_suppressed() {
            return;
        }
        if self.input_state.presenter_mode
            || self.input_state.show_help
            || self.input_state.command_palette_open
            || self.input_state.tour_active
        {
            return;
        }
        // Don't nag during first-run onboarding; wait until it is finished or
        // skipped.
        if self.onboarding.state().first_run_active() {
            return;
        }
        // Never clobber real feedback: only when the toast queue is idle.
        if !self.input_state.toasts_idle() {
            return;
        }

        let outcome = self.input_state.push_toast(
            ToastPriority::Info,
            "onboarding.panel_deprecation",
            Toast::info(PANEL_DEPRECATION_MESSAGE).once_per_content(),
        );
        if outcome.accepted() {
            self.data.panel_deprecation_notice_shown = true;
        }
    }
}

/// The capability warning to raise, if any: only when this exact capability
/// set has not been evaluated yet this session (once per session unless the
/// detected capabilities change) and something is actually limited. The
/// queue's once-per-content rate limit additionally keeps an identical
/// summary from re-showing.
pub(super) fn capability_toast_message(
    previous: Option<crate::input::state::CompositorCapabilities>,
    current: crate::input::state::CompositorCapabilities,
) -> Option<String> {
    if previous == Some(current) {
        return None;
    }
    // `limitations_summary()` is the authority on whether anything is limited:
    // it returns `None` only when nothing is degraded. (`all_available()` omits
    // `freeze_capture`, so a freeze-only limitation must not gate on it.)
    current.limitations_summary()
}

#[cfg(test)]
mod tests;
