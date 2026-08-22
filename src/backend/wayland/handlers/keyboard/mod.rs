// Bridges Wayland key events into our `InputState`, including capture-action plumbing.
mod translate;

use log::{debug, warn};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, KeyboardHandler, Modifiers, RawModifiers};
use std::time::{Duration, Instant};
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_keyboard, wl_surface},
};

use crate::{config::Action, input::Key, notification};

use super::super::state::WaylandState;
pub(in crate::backend::wayland) use translate::keysym_to_key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum XdgFocusLeaveAction {
    Ignore,
    AwaitDesktopOpen,
    RestoreClipboardFocus,
    StayOpen,
    Exit,
}

pub(in crate::backend::wayland) fn xdg_focus_leave_action(
    is_xdg_window: bool,
    desktop_open_in_progress: bool,
    focus_exit_suppressed: bool,
    focus_loss_exits_overlay: bool,
) -> XdgFocusLeaveAction {
    if !is_xdg_window {
        XdgFocusLeaveAction::Ignore
    } else if desktop_open_in_progress {
        XdgFocusLeaveAction::AwaitDesktopOpen
    } else if focus_exit_suppressed {
        XdgFocusLeaveAction::RestoreClipboardFocus
    } else if focus_loss_exits_overlay {
        XdgFocusLeaveAction::Exit
    } else {
        XdgFocusLeaveAction::StayOpen
    }
}

impl KeyboardHandler for WaylandState {
    fn enter(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        serial: u32,
        _raw: &[u32],
        _keysyms: &[smithay_client_toolkit::seat::keyboard::Keysym],
    ) {
        debug!("Keyboard focus entered");
        self.set_keyboard_focus(true);
        self.clear_focus_exit_suppression();
        self.clear_xdg_close_guard();
        self.set_last_activation_serial(Some(serial));
        self.maybe_retry_activation(qh);
        if self.toolbar.is_focusable_surface(surface) {
            self.set_toolbar_focus_active(true);
        } else {
            self.clear_toolbar_focus();
        }
        // Mark overlay as ready once we have focus and surface is configured
        if self.surface.is_configured() {
            self.set_overlay_ready(true);
            debug!("Overlay ready for keybinds");
        }
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        debug!("Keyboard focus left");
        self.set_keyboard_focus(false);
        self.set_overlay_ready(false);
        self.clear_toolbar_focus();

        // When the compositor moves focus away from our surface (e.g. to a portal
        // dialog, another layer surface, or a different window), it's possible for
        // us to miss some key release events. To avoid leaving modifiers "stuck"
        // and breaking shortcuts/tools, aggressively reset our modifier state on
        // focus loss.
        self.input_state.reset_modifiers();
        self.sync_region_square_modifier(false);
        self.input_state.clear_command_palette_repeat();
        self.clear_key_repeat();
        self.set_board_pan_key_held(false);
        self.stop_board_pan();

        match xdg_focus_leave_action(
            self.surface.is_xdg_window(),
            self.desktop_open_in_progress(),
            self.focus_exit_suppressed(),
            self.xdg_focus_loss_exits_overlay(),
        ) {
            XdgFocusLeaveAction::Ignore => {}
            XdgFocusLeaveAction::AwaitDesktopOpen => {
                // The opener deliberately transfers focus. Overlay exit waits
                // for the detached spawn handoff so teardown cannot race it.
                warn!(
                    "Keyboard focus left the xdg fallback during desktop-open; awaiting helper handoff"
                );
            }
            XdgFocusLeaveAction::RestoreClipboardFocus => {
                warn!(
                    "Keyboard focus lost in xdg fallback; suppressing exit after clipboard action"
                );
                self.set_xdg_close_guard_for(Duration::from_millis(2500));
                self.request_xdg_activation(qh);
            }
            XdgFocusLeaveAction::StayOpen => {
                warn!(
                    "Keyboard focus lost in xdg fallback; keeping overlay open without auto-reactivation (ui.xdg_focus_loss_behavior=stay)"
                );
                self.set_xdg_close_guard_for(Duration::from_millis(2500));
            }
            XdgFocusLeaveAction::Exit => {
                warn!("Keyboard focus lost in xdg fallback; exiting overlay");
                notification::send_notification_async(
                    &self.tokio_handle,
                    "Wayscriber lost focus".to_string(),
                    "The desktop could not keep the overlay focused, so Wayscriber closed it."
                        .to_string(),
                    Some("dialog-warning".to_string()),
                );
                self.input_state.should_exit = true;
            }
        }
    }

    fn press_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        // Block keybinds until overlay is fully ready (prevents Ctrl+W leaking to apps)
        if !self.is_overlay_ready() {
            debug!("Ignoring key press before overlay ready");
            return;
        }
        let key = keysym_to_key(event.keysym);
        // Report the physical press to the input HUD before any subsystem
        // routing consumes it, so the HUD always shows what was pressed rather
        // than what happened to reach the canvas. Compositor-synced modifier
        // state is already current here, so the chord label is exact.
        self.input_state
            .note_input_hud_key(key, self.input_state.modifiers);
        // Any fresh key press ends the previous auto-repeat; a repeatable one
        // re-arms it at the end of this handler.
        self.clear_key_repeat();
        if self.input_state.region_is_engaged() {
            let action = self.input_state.action_for_key(key);
            if let Some(action) = action {
                if self
                    .input_state
                    .refuse_region_capture_while_screen_modal_engaged(action)
                {
                    return;
                }
                if self
                    .input_state
                    .capture_region_action_reaches_backend(action)
                {
                    // Route the opening action directly while its picker owns
                    // the modal. Going through InputState would clear held
                    // modifiers before the backend can distinguish same-action
                    // cancellation from a different-action refusal.
                    self.handle_capture_action(action);
                    return;
                }
                if action == Action::CopyTextFromScreen
                    && self.input_state.region_state().purpose()
                        == Some(crate::input::state::RegionPurposeTag::Ocr)
                {
                    self.cancel_ocr();
                    return;
                }
            }
            if self.input_state.region_state().is_review()
                && let Some(review_action) = region_review_key_action(
                    key,
                    self.input_state.modifiers.ctrl,
                    self.input_state.modifiers.shift,
                )
            {
                match review_action {
                    RegionReviewKeyAction::Nudge(dx, dy) => {
                        self.nudge_region_review(dx, dy);
                    }
                    RegionReviewKeyAction::Submit(action) => {
                        self.submit_region_review_action(action);
                    }
                }
                return;
            }
            if region_capture_select_all_pressed(
                self.input_state.region_is_active(),
                self.input_state.region_state().purpose(),
                self.input_state.modifiers.ctrl,
                key,
            ) {
                self.submit_whole_region_capture();
                return;
            }
            // Every other shortcut is swallowed while the selector is up so a
            // key cannot change the active tool mid-drag.
            if matches!(key, Key::Escape) {
                if self
                    .input_state
                    .region_state()
                    .purpose()
                    .is_some_and(crate::input::state::RegionPurposeTag::is_capture)
                {
                    self.cancel_region_capture();
                } else {
                    self.cancel_ocr();
                }
            }
            return;
        }
        if self.input_state.eyedropper_is_engaged() {
            let action = self.input_state.action_for_key(key);
            if action.is_some_and(|action| {
                self.input_state
                    .refuse_region_capture_while_screen_modal_engaged(action)
            }) {
                return;
            }
            if matches!(key, Key::Escape) || action == Some(Action::PickScreenColor) {
                self.cancel_eyedropper();
            }
            return;
        }
        if matches!(key, Key::Escape)
            && self.input_state.modifiers.shift
            && self.try_skip_first_run_onboarding()
        {
            return;
        }
        if self.try_handle_first_run_background_mode_choice(key) {
            return;
        }
        if matches!(key, Key::Space) && self.should_capture_space_for_board_pan() {
            self.set_board_pan_key_held(true);
            self.input_state.needs_redraw = true;
            return;
        }
        if self.zoom.is_engaged() {
            match key {
                Key::Escape => {
                    self.exit_zoom();
                    return;
                }
                Key::Up | Key::Down | Key::Left | Key::Right => {
                    if !self.zoom.active {
                        return;
                    }
                    if self.zoom.locked {
                        return;
                    }
                    let step = if self.input_state.modifiers.shift {
                        WaylandState::ZOOM_PAN_STEP_LARGE
                    } else {
                        WaylandState::ZOOM_PAN_STEP
                    };
                    let (dx, dy) = match key {
                        Key::Up => (0.0, step),
                        Key::Down => (0.0, -step),
                        Key::Left => (step, 0.0),
                        Key::Right => (-step, 0.0),
                        _ => (0.0, 0.0),
                    };
                    self.zoom.pan_by_screen_delta(
                        dx,
                        dy,
                        self.surface.width(),
                        self.surface.height(),
                    );
                    self.sync_input_zoom_state();
                    self.input_state.dirty_tracker.mark_full();
                    self.input_state.needs_redraw = true;
                    return;
                }
                _ => {}
            }
        }
        debug!("Key pressed: {:?}", key);
        let modal_capture = self.input_state.modal_owns_text_input();
        let modal_blocks_repeat = self.input_state.modal_blocks_canvas_key_repeat();
        if should_try_toolbar_key(key, modal_capture)
            && self.handle_toolbar_key(key, Some(conn), Some(qh))
        {
            return;
        }

        self.apply_input_key(key);

        // Arm auto-repeat for editing/navigation keys that reached normal
        // dispatch. Some dedicated entry modals manage or intentionally block
        // repeat themselves; other routed overlays (for example Help search)
        // still use this timer even though they disable the canvas IME.
        if !modal_blocks_repeat && is_repeatable_key(key) && self.has_keyboard_focus() {
            self.key_repeat_key = Some(key);
            self.key_repeat_next_tick = Some(Instant::now() + Self::KEY_REPEAT_INITIAL_DELAY);
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        let key = keysym_to_key(event.keysym);
        debug!("Key released: {:?}", key);
        // Stop auto-repeat once the held key comes up.
        if self.key_repeat_key == Some(key) {
            self.clear_key_repeat();
        }
        if screen_modal_swallows_key_release(
            self.input_state.region_is_engaged(),
            self.input_state.eyedropper_is_engaged(),
        ) {
            return;
        }
        if matches!(key, Key::Space) && self.board_pan_key_held() {
            self.set_board_pan_key_held(false);
            self.input_state.needs_redraw = true;
            return;
        }
        self.input_state.on_key_release(key);
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _layout: RawModifiers,
        _group: u32,
    ) {
        debug!(
            "Modifiers: ctrl={} alt={} shift={} logo={}",
            modifiers.ctrl, modifiers.alt, modifiers.shift, modifiers.logo
        );
        // Trust compositor-reported modifier state to reconcile any missed key release
        // events and avoid "stuck" modifiers.
        self.input_state.sync_modifiers(
            modifiers.shift,
            modifiers.ctrl,
            modifiers.alt,
            modifiers.logo,
        );
        self.sync_region_square_modifier(modifiers.shift);
    }

    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        // sctk only calls this with a calloop-driven repeat keyboard, which
        // this manual poll loop does not use; the loop's `tick_key_repeat`
        // drives repeats through the same path instead. Kept for parity.
        self.dispatch_key_repeat(keysym_to_key(event.keysym), conn, qh);
    }
}

/// Delay before a held key begins repeating. Shared with the input HUD's
/// system monitor so held keys tick at the same cadence in both capture modes.
pub(in crate::backend::wayland) const KEY_REPEAT_INITIAL_DELAY: Duration =
    Duration::from_millis(400);
/// Interval between repeats once repeating (≈25/s).
pub(in crate::backend::wayland) const KEY_REPEAT_INTERVAL: Duration = Duration::from_millis(40);

/// Keys that auto-repeat while held: text entry, deletion, and
/// navigation. Action/toggle keys (Return, Escape, Tab, F-keys) are left
/// out so holding them never spams their one-shot effect.
///
/// This is an *action* policy, not a keymap fact: the input HUD's system
/// monitor deliberately asks xkb which keys repeat instead, because there it
/// mirrors what the focused app receives rather than gating wayscriber's own
/// dispatch.
fn is_repeatable_key(key: Key) -> bool {
    matches!(
        key,
        Key::Char(_)
            | Key::Backspace
            | Key::Delete
            | Key::Space
            | Key::Left
            | Key::Right
            | Key::Up
            | Key::Down
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown
    )
}

impl WaylandState {
    pub(in crate::backend::wayland) const KEY_REPEAT_INITIAL_DELAY: Duration =
        KEY_REPEAT_INITIAL_DELAY;

    pub(in crate::backend::wayland) fn clear_key_repeat(&mut self) {
        self.key_repeat_key = None;
        self.key_repeat_next_tick = None;
    }

    /// Duration until the next repeat fires, for the event-loop timeout. The
    /// loop otherwise sleeps until a real event and would never wake to
    /// repeat a held key.
    pub(in crate::backend::wayland) fn key_repeat_timeout(&self, now: Instant) -> Option<Duration> {
        if !self.has_keyboard_focus() {
            return None;
        }
        self.key_repeat_next_tick
            .map(|next| next.saturating_duration_since(now))
    }

    /// Fire a repeat if one is due, then reschedule from `now` (so a long
    /// block does not burst-catch-up). Called once per event-loop iteration.
    pub(in crate::backend::wayland) fn tick_key_repeat(
        &mut self,
        now: Instant,
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if !self.has_keyboard_focus() {
            self.clear_key_repeat();
            return;
        }
        if self.input_state.modal_blocks_canvas_key_repeat() {
            // A modal can open from pointer/toolbar input while a canvas key is
            // still held. Retire that timer before it starts feeding the new
            // focus owner (or duplicates the command palette's own repeat).
            self.clear_key_repeat();
            return;
        }
        let Some(key) = self.key_repeat_key else {
            return;
        };
        let Some(next) = self.key_repeat_next_tick else {
            return;
        };
        if now < next {
            return;
        }
        self.dispatch_key_repeat(key, conn, qh);
        self.key_repeat_next_tick = Some(now + KEY_REPEAT_INTERVAL);
    }

    /// Re-dispatch a held key through the same routing a fresh press uses
    /// (overlay-ready gate, eyedropper/zoom/pan guards, toolbar routing, then
    /// `apply_input_key`). Shared by the manual repeat tick and sctk's
    /// `repeat_key`.
    fn dispatch_key_repeat(&mut self, key: Key, conn: &Connection, qh: &QueueHandle<Self>) {
        if !self.is_overlay_ready() {
            return;
        }
        // A held key ticks the HUD chip's repeat counter at the repeat rate,
        // exactly like a fresh press reports the first one.
        self.input_state
            .note_input_hud_key(key, self.input_state.modifiers);
        if self.input_state.region_is_engaged() || self.input_state.eyedropper_is_engaged() {
            return;
        }
        if matches!(key, Key::Space) && self.board_pan_key_held() {
            return;
        }
        if self.zoom.active {
            match key {
                Key::Up | Key::Down | Key::Left | Key::Right => {
                    if self.zoom.locked {
                        return;
                    }
                    let step = if self.input_state.modifiers.shift {
                        WaylandState::ZOOM_PAN_STEP_LARGE
                    } else {
                        WaylandState::ZOOM_PAN_STEP
                    };
                    let (dx, dy) = match key {
                        Key::Up => (0.0, step),
                        Key::Down => (0.0, -step),
                        Key::Left => (step, 0.0),
                        Key::Right => (-step, 0.0),
                        _ => (0.0, 0.0),
                    };
                    self.zoom.pan_by_screen_delta(
                        dx,
                        dy,
                        self.surface.width(),
                        self.surface.height(),
                    );
                    self.sync_input_zoom_state();
                    self.input_state.dirty_tracker.mark_full();
                    self.input_state.needs_redraw = true;
                    return;
                }
                _ => {}
            }
        }
        if self.input_state.command_palette_open && matches!(key, Key::Up | Key::Down) {
            return;
        }
        let modal_capture = self.input_state.modal_owns_text_input();
        if should_try_toolbar_key(key, modal_capture)
            && self.handle_toolbar_key(key, Some(conn), Some(qh))
        {
            return;
        }
        self.apply_input_key_repeat(key);
    }
}

fn should_try_toolbar_key(key: Key, modal_capture_active: bool) -> bool {
    if modal_capture_active {
        return false;
    }
    matches!(key, Key::Tab | Key::Return | Key::Space | Key::Escape)
}

fn screen_modal_swallows_key_release(
    region_selector_engaged: bool,
    eyedropper_engaged: bool,
) -> bool {
    region_selector_engaged || eyedropper_engaged
}

fn region_capture_select_all_pressed(
    region_active: bool,
    purpose: Option<crate::input::state::RegionPurposeTag>,
    ctrl: bool,
    key: Key,
) -> bool {
    region_active
        && purpose.is_some_and(crate::input::state::RegionPurposeTag::is_capture)
        && ctrl
        && matches!(key, Key::Char('a' | 'A'))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionReviewKeyAction {
    Nudge(i64, i64),
    Submit(crate::ui::RegionAction),
}

fn region_review_key_action(key: Key, ctrl: bool, shift: bool) -> Option<RegionReviewKeyAction> {
    let step = if shift { 10 } else { 1 };
    match key {
        Key::Left => Some(RegionReviewKeyAction::Nudge(-step, 0)),
        Key::Right => Some(RegionReviewKeyAction::Nudge(step, 0)),
        Key::Up => Some(RegionReviewKeyAction::Nudge(0, -step)),
        Key::Down => Some(RegionReviewKeyAction::Nudge(0, step)),
        Key::Char('c' | 'C') if ctrl => {
            Some(RegionReviewKeyAction::Submit(crate::ui::RegionAction::Copy))
        }
        Key::Char('s' | 'S') if ctrl => {
            Some(RegionReviewKeyAction::Submit(crate::ui::RegionAction::Save))
        }
        Key::Return => Some(RegionReviewKeyAction::Submit(crate::ui::RegionAction::Both)),
        Key::Char('b' | 'B') if !ctrl => Some(RegionReviewKeyAction::Submit(
            crate::ui::RegionAction::Board,
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_routing_is_blocked_while_a_modal_capture_is_active() {
        assert!(!should_try_toolbar_key(Key::Tab, true));
        assert!(!should_try_toolbar_key(Key::Return, true));
        assert!(!should_try_toolbar_key(Key::Space, true));
    }

    #[test]
    fn engaged_region_modal_swallows_shift_release_until_modifier_sync() {
        assert!(screen_modal_swallows_key_release(true, false));
        assert!(screen_modal_swallows_key_release(false, true));
        assert!(!screen_modal_swallows_key_release(false, false));

        let mut input = crate::input::state::test_support::make_test_input_state();
        input.sync_modifiers(true, false, false, false);
        input.set_region_pending_capture(
            crate::input::state::RegionPurposeTag::Ocr,
            1,
            crate::input::state::ScreenCaptureSource::Frozen,
        );
        if !screen_modal_swallows_key_release(input.region_is_engaged(), false) {
            input.on_key_release(Key::Shift);
        }
        assert!(
            input.modifiers.shift,
            "key release must not outrun modifier sync"
        );
        input.sync_modifiers(false, false, false, false);
        assert!(
            !input.modifiers.shift,
            "compositor modifier sync is authoritative"
        );
    }

    #[test]
    fn ctrl_a_selects_all_only_for_an_active_capture_picker() {
        use crate::input::state::RegionPurposeTag;

        assert!(region_capture_select_all_pressed(
            true,
            Some(RegionPurposeTag::CaptureDeliver),
            true,
            Key::Char('a'),
        ));
        assert!(region_capture_select_all_pressed(
            true,
            Some(RegionPurposeTag::CaptureInteractive),
            true,
            Key::Char('A'),
        ));
        assert!(!region_capture_select_all_pressed(
            false,
            Some(RegionPurposeTag::CaptureDeliver),
            true,
            Key::Char('a'),
        ));
        assert!(!region_capture_select_all_pressed(
            true,
            Some(RegionPurposeTag::Ocr),
            true,
            Key::Char('a'),
        ));
        assert!(!region_capture_select_all_pressed(
            true,
            Some(RegionPurposeTag::CaptureDeliver),
            false,
            Key::Char('a'),
        ));
    }

    #[test]
    fn review_keys_map_to_one_shot_pixel_edits_and_typed_destinations() {
        assert_eq!(
            region_review_key_action(Key::Left, false, false),
            Some(RegionReviewKeyAction::Nudge(-1, 0))
        );
        assert_eq!(
            region_review_key_action(Key::Down, false, true),
            Some(RegionReviewKeyAction::Nudge(0, 10))
        );
        assert_eq!(
            region_review_key_action(Key::Char('c'), true, false),
            Some(RegionReviewKeyAction::Submit(crate::ui::RegionAction::Copy))
        );
        assert_eq!(
            region_review_key_action(Key::Char('s'), true, false),
            Some(RegionReviewKeyAction::Submit(crate::ui::RegionAction::Save))
        );
        assert_eq!(
            region_review_key_action(Key::Return, false, false),
            Some(RegionReviewKeyAction::Submit(crate::ui::RegionAction::Both))
        );
        assert_eq!(
            region_review_key_action(Key::Char('b'), false, false),
            Some(RegionReviewKeyAction::Submit(
                crate::ui::RegionAction::Board
            ))
        );
        assert_eq!(region_review_key_action(Key::Char('b'), true, false), None);
    }

    #[test]
    fn toolbar_routing_only_allows_activate_and_tab_keys() {
        assert!(should_try_toolbar_key(Key::Tab, false));
        assert!(should_try_toolbar_key(Key::Return, false));
        assert!(should_try_toolbar_key(Key::Space, false));
        assert!(should_try_toolbar_key(Key::Escape, false));
        assert!(!should_try_toolbar_key(Key::Down, false));
    }

    #[test]
    fn text_and_navigation_keys_auto_repeat() {
        // The reported case (hold Backspace to delete) plus the rest of the
        // editing/navigation set.
        assert!(is_repeatable_key(Key::Backspace));
        assert!(is_repeatable_key(Key::Delete));
        assert!(is_repeatable_key(Key::Char('a')));
        assert!(is_repeatable_key(Key::Space));
        for key in [Key::Left, Key::Right, Key::Up, Key::Down] {
            assert!(is_repeatable_key(key));
        }
    }

    #[test]
    fn one_shot_keys_do_not_auto_repeat() {
        // Holding these must never spam their effect.
        assert!(!is_repeatable_key(Key::Return));
        assert!(!is_repeatable_key(Key::Escape));
        assert!(!is_repeatable_key(Key::Tab));
        assert!(!is_repeatable_key(Key::F10));
    }

    #[test]
    fn active_desktop_open_owns_xdg_focus_leave_for_every_focus_policy() {
        for focus_exit_suppressed in [false, true] {
            for focus_loss_exits_overlay in [false, true] {
                assert_eq!(
                    xdg_focus_leave_action(
                        true,
                        true,
                        focus_exit_suppressed,
                        focus_loss_exits_overlay,
                    ),
                    XdgFocusLeaveAction::AwaitDesktopOpen,
                );
            }
        }
    }

    #[test]
    fn focus_leave_routing_preserves_non_desktop_open_policies() {
        assert_eq!(
            xdg_focus_leave_action(false, true, true, true),
            XdgFocusLeaveAction::Ignore,
        );
        assert_eq!(
            xdg_focus_leave_action(true, false, true, true),
            XdgFocusLeaveAction::RestoreClipboardFocus,
        );
        assert_eq!(
            xdg_focus_leave_action(true, false, false, false),
            XdgFocusLeaveAction::StayOpen,
        );
        assert_eq!(
            xdg_focus_leave_action(true, false, false, true),
            XdgFocusLeaveAction::Exit,
        );
    }
}
