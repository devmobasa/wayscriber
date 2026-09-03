//! Presenter, focus, and light-mode lifecycle state.
//!
//! This owner captures and restores transient chrome state. `InputState`
//! remains the coordinator for mutations that belong to the toolbar, UI
//! visibility, drawing style, feedback, and backend effect owners.

use super::{DesktopEnvironment, InputState, ShellMode, Toast, ToastPriority, ToolbarVisibility};
use crate::config::{PresenterModeConfig, PresenterToolBehavior, TopDisplayMode};
use crate::domain::Action;
use crate::input::tool::Tool;

const FOCUS_MODE_TOAST_KEY: &str = "focus.mode";

#[derive(Debug)]
pub(in crate::input::state) struct ChromeModes {
    presenter: bool,
    presenter_config: PresenterModeConfig,
    presenter_restore: Option<PresenterRestore>,
    focus_restore: Option<FocusModeRestore>,
    light: bool,
    light_drawing: bool,
    light_restore: Option<LightModeRestore>,
}

impl ChromeModes {
    pub(in crate::input::state) fn new(presenter_config: PresenterModeConfig) -> Self {
        Self {
            presenter: false,
            presenter_config,
            presenter_restore: None,
            focus_restore: None,
            light: false,
            light_drawing: false,
            light_restore: None,
        }
    }

    pub(in crate::input::state) const fn presenter_active(&self) -> bool {
        self.presenter
    }

    pub(in crate::input::state) const fn presenter_config(&self) -> &PresenterModeConfig {
        &self.presenter_config
    }

    pub(in crate::input::state) fn presenter_hides_toolbars(&self) -> bool {
        self.presenter && self.presenter_config.hide_toolbars
    }

    fn begin_presenter(&mut self, restore: PresenterRestore) {
        self.presenter_restore = Some(restore);
        self.presenter = true;
    }

    fn end_presenter(&mut self) -> Option<PresenterRestore> {
        if !self.presenter {
            return None;
        }
        self.presenter = false;
        self.presenter_restore.take()
    }

    #[cfg(test)]
    pub(in crate::input::state) fn presenter_restore_pending(&self) -> bool {
        self.presenter_restore.is_some()
    }

    pub(in crate::input::state) fn presenter_restored_status_bar(&self) -> Option<bool> {
        self.presenter_restore
            .as_ref()
            .and_then(PresenterRestore::show_status_bar)
    }

    pub(in crate::input::state) fn presenter_restored_tool_preview(&self) -> Option<bool> {
        self.presenter_restore
            .as_ref()
            .and_then(PresenterRestore::show_tool_preview)
    }

    pub(in crate::input::state) fn presenter_restored_click_highlight(&self) -> Option<bool> {
        self.presenter_restore
            .as_ref()
            .and_then(PresenterRestore::click_highlight_enabled)
    }

    pub(in crate::input::state) fn presenter_restored_top_display_mode(
        &self,
    ) -> Option<TopDisplayMode> {
        self.presenter_restore
            .as_ref()
            .and_then(PresenterRestore::toolbar_visibility)
            .map(ToolbarVisibility::top_display_mode)
    }

    pub(in crate::input::state) fn presenter_restores_visible_toolbar(&self) -> bool {
        self.presenter_restore
            .as_ref()
            .and_then(PresenterRestore::toolbar_visibility)
            .is_some_and(ToolbarVisibility::effectively_visible)
    }

    pub(in crate::input::state) fn retarget_presenter_toolbar_display_mode(
        &mut self,
        mode: TopDisplayMode,
    ) -> bool {
        let Some(toolbar) = self
            .presenter_restore
            .as_mut()
            .and_then(PresenterRestore::toolbar_visibility_mut)
        else {
            return false;
        };
        toolbar.set_top_display_mode(mode);
        true
    }

    pub(in crate::input::state) const fn focus_active(&self) -> bool {
        self.focus_restore.is_some()
    }

    fn begin_focus(&mut self, restore: FocusModeRestore) {
        self.focus_restore = Some(restore);
    }

    fn end_focus(&mut self) -> Option<FocusModeRestore> {
        self.focus_restore.take()
    }

    pub(in crate::input::state) fn cancel_focus(&mut self) {
        self.focus_restore = None;
    }

    pub(in crate::input::state) fn retarget_focus_status_bar(
        &mut self,
        show: bool,
    ) -> Option<bool> {
        let restore = self.focus_restore.as_mut()?;
        let changed = restore.show_status_bar != show;
        restore.show_status_bar = show;
        Some(changed)
    }

    pub(in crate::input::state) const fn light_active(&self) -> bool {
        self.light
    }

    pub(in crate::input::state) const fn light_drawing(&self) -> bool {
        self.light_drawing
    }

    pub(in crate::input::state) fn set_light_drawing(&mut self, drawing: bool) {
        self.light_drawing = drawing;
    }

    fn begin_light(&mut self, drawing: bool, restore: LightModeRestore) {
        self.light_restore = Some(restore);
        self.light = true;
        self.light_drawing = drawing;
    }

    fn end_light(&mut self) -> Option<LightModeRestore> {
        if !self.light {
            return None;
        }
        self.light = false;
        self.light_drawing = false;
        self.light_restore.take()
    }

    pub(in crate::input::state) fn light_restored_tool_override(&self) -> Option<Option<Tool>> {
        self.light_restore
            .as_ref()
            .map(|restore| restore.tool_override())
    }

    pub(in crate::input::state) fn retarget_visibility_from_pin(&mut self, visible: bool) -> bool {
        if let Some(restore) = self.focus_restore.as_mut() {
            restore.toolbar_visibility.set_visible(visible);
            return true;
        }
        if let Some(toolbar) = self
            .presenter_restore
            .as_mut()
            .and_then(PresenterRestore::toolbar_visibility_mut)
        {
            toolbar.set_visible(visible);
            return true;
        }
        if let Some(restore) = self.light_restore.as_mut() {
            restore.toolbar_visibility.set_visible(visible);
            return true;
        }
        false
    }

    #[cfg(test)]
    pub(in crate::input::state) fn presenter_config_mut_for_test(
        &mut self,
    ) -> &mut PresenterModeConfig {
        &mut self.presenter_config
    }

    #[cfg(test)]
    pub(in crate::input::state) fn override_presenter_for_test(&mut self, active: bool) {
        self.presenter = active;
        if !active {
            self.presenter_restore = None;
        }
    }
}

impl InputState {
    pub(crate) fn presenter_mode_active(&self) -> bool {
        self.modes.presenter_active()
    }

    pub(crate) fn presenter_mode_config(&self) -> &PresenterModeConfig {
        self.modes.presenter_config()
    }

    pub(crate) fn presenter_hides_toolbars(&self) -> bool {
        self.modes.presenter_hides_toolbars()
    }

    pub(crate) fn light_mode_active(&self) -> bool {
        self.modes.light_active()
    }

    pub(crate) fn light_mode_drawing_active(&self) -> bool {
        self.modes.light_drawing()
    }

    pub(crate) fn presenter_restored_tool_preview(&self) -> Option<bool> {
        self.modes.presenter_restored_tool_preview()
    }

    pub(crate) fn presenter_restored_click_highlight(&self) -> Option<bool> {
        self.modes.presenter_restored_click_highlight()
    }

    pub(crate) fn presenter_restored_top_display_mode(&self) -> Option<TopDisplayMode> {
        self.modes.presenter_restored_top_display_mode()
    }

    pub(crate) fn retarget_presenter_toolbar_display_mode(&mut self, mode: TopDisplayMode) -> bool {
        self.modes.retarget_presenter_toolbar_display_mode(mode)
    }

    #[cfg(test)]
    pub(crate) fn presenter_restore_pending(&self) -> bool {
        self.modes.presenter_restore_pending()
    }

    #[cfg(test)]
    pub(crate) fn presenter_mode_config_mut_for_test(&mut self) -> &mut PresenterModeConfig {
        self.modes.presenter_config_mut_for_test()
    }

    #[cfg(test)]
    pub(crate) fn override_presenter_mode_for_test(&mut self, active: bool) {
        self.modes.override_presenter_for_test(active);
    }
}

#[derive(Debug, Clone, Copy)]
struct PresenterRestore {
    show_status_bar: Option<bool>,
    show_tool_preview: Option<bool>,
    toolbar_visibility: Option<ToolbarVisibility>,
    click_highlight_enabled: Option<bool>,
    input_hud_enabled: Option<bool>,
    tool_override: Option<Option<Tool>>,
}

impl PresenterRestore {
    pub(in crate::input::state) fn capture(
        config: &PresenterModeConfig,
        show_status_bar: bool,
        show_tool_preview: bool,
        toolbar_visibility: ToolbarVisibility,
        click_highlight_enabled: bool,
        input_hud_enabled: bool,
        tool_override: Option<Tool>,
    ) -> Self {
        Self {
            show_status_bar: config.hide_status_bar.then_some(show_status_bar),
            show_tool_preview: config.hide_tool_preview.then_some(show_tool_preview),
            toolbar_visibility: config.hide_toolbars.then_some(toolbar_visibility),
            click_highlight_enabled: config
                .enable_click_highlight
                .then_some(click_highlight_enabled),
            input_hud_enabled: config.enable_input_hud.then_some(input_hud_enabled),
            tool_override: (!matches!(config.tool_behavior, PresenterToolBehavior::Keep))
                .then_some(tool_override),
        }
    }

    pub(in crate::input::state) const fn show_status_bar(&self) -> Option<bool> {
        self.show_status_bar
    }

    pub(in crate::input::state) const fn show_tool_preview(&self) -> Option<bool> {
        self.show_tool_preview
    }

    pub(in crate::input::state) const fn toolbar_visibility(&self) -> Option<ToolbarVisibility> {
        self.toolbar_visibility
    }

    fn toolbar_visibility_mut(&mut self) -> Option<&mut ToolbarVisibility> {
        self.toolbar_visibility.as_mut()
    }

    pub(in crate::input::state) const fn click_highlight_enabled(&self) -> Option<bool> {
        self.click_highlight_enabled
    }

    pub(in crate::input::state) const fn input_hud_enabled(&self) -> Option<bool> {
        self.input_hud_enabled
    }

    pub(in crate::input::state) const fn tool_override(&self) -> Option<Option<Tool>> {
        self.tool_override
    }
}

#[derive(Debug, Clone, Copy)]
struct FocusModeRestore {
    show_status_bar: bool,
    toolbar_visibility: ToolbarVisibility,
    show_floating_badge: bool,
    show_zoom_chip: bool,
}

impl FocusModeRestore {
    pub(in crate::input::state) const fn capture(
        show_status_bar: bool,
        toolbar_visibility: ToolbarVisibility,
        show_floating_badge: bool,
        show_zoom_chip: bool,
    ) -> Self {
        Self {
            show_status_bar,
            toolbar_visibility,
            show_floating_badge,
            show_zoom_chip,
        }
    }

    pub(in crate::input::state) const fn show_status_bar(self) -> bool {
        self.show_status_bar
    }

    pub(in crate::input::state) const fn toolbar_visibility(self) -> ToolbarVisibility {
        self.toolbar_visibility
    }

    pub(in crate::input::state) const fn show_floating_badge(self) -> bool {
        self.show_floating_badge
    }

    pub(in crate::input::state) const fn show_zoom_chip(self) -> bool {
        self.show_zoom_chip
    }
}

#[derive(Debug, Clone, Copy)]
struct LightModeRestore {
    show_status_bar: bool,
    show_tool_preview: bool,
    toolbar_visibility: ToolbarVisibility,
    click_highlight_enabled: bool,
    tool_override: Option<Tool>,
}

impl LightModeRestore {
    pub(in crate::input::state) const fn capture(
        show_status_bar: bool,
        show_tool_preview: bool,
        toolbar_visibility: ToolbarVisibility,
        click_highlight_enabled: bool,
        tool_override: Option<Tool>,
    ) -> Self {
        Self {
            show_status_bar,
            show_tool_preview,
            toolbar_visibility,
            click_highlight_enabled,
            tool_override,
        }
    }

    pub(in crate::input::state) const fn show_status_bar(self) -> bool {
        self.show_status_bar
    }

    pub(in crate::input::state) const fn show_tool_preview(self) -> bool {
        self.show_tool_preview
    }

    pub(in crate::input::state) const fn toolbar_visibility(self) -> ToolbarVisibility {
        self.toolbar_visibility
    }

    pub(in crate::input::state) const fn click_highlight_enabled(self) -> bool {
        self.click_highlight_enabled
    }

    pub(in crate::input::state) const fn tool_override(self) -> Option<Tool> {
        self.tool_override
    }
}

impl InputState {
    /// True while a focus-mode snapshot is waiting to be restored.
    pub fn focus_mode_active(&self) -> bool {
        self.modes.focus_active()
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
        !self.status_hud_effectively_visible()
            && (self.zoom_active()
                || self.frozen_active()
                || (self.boards.pan_enabled()
                    && self.boards.show_pan_badge()
                    && !self.board_is_transparent())
                || (matches!(self.state, crate::input::DrawingState::TextInput { .. })
                    && self.text_editing.edit_target().is_some()))
    }

    /// Apply a status-bar value authored by a preset or session restore.
    /// While Focus Mode owns chrome, update the value waiting behind its
    /// suppression instead of making the bar visible or leaving a stale
    /// snapshot that would overwrite the authored value on exit.
    pub(crate) fn set_status_bar_visibility_preserving_focus(&mut self, show: bool) -> bool {
        if let Some(changed) = self.modes.retarget_focus_status_bar(show) {
            return changed;
        }
        let changed = self.ui_visibility.show_status_bar != show;
        self.ui_visibility.show_status_bar = show;
        changed
    }

    /// A manual chrome toggle takes ownership of visibility: drop the
    /// snapshot (without restoring it) so a later focus-mode exit cannot
    /// override what the user just chose by hand. The next
    /// `ToggleFocusMode` starts from a fresh snapshot.
    pub(crate) fn break_focus_mode(&mut self) {
        self.modes.cancel_focus();
        self.clear_focus_mode_toast();
    }

    fn clear_focus_mode_toast(&mut self) {
        // The Restore action can be queued behind a higher-priority warning,
        // so retract it from both slots whenever Focus no longer owns chrome.
        self.remove_matching_toasts(|key, action| {
            key == FOCUS_MODE_TOAST_KEY || action == Some(Action::ToggleFocusMode)
        });
    }

    fn clear_all_chrome_recovery_toast(&mut self) {
        // The generic all-chrome warning deliberately shares the routine
        // `ui` key so it can replace "Toolbar: hidden" in place. Match the
        // recovery action as well: after the rescue arm shows the toolbar,
        // either toast's Show action would immediately hide it again.
        self.remove_matching_toasts(|key, action| {
            key == "ui" && action == Some(Action::ToggleToolbar)
        });
    }

    /// Toggle focus mode:
    /// - snapshot present → restore it (chrome returns exactly as it was,
    ///   including a micro top strip);
    /// - chrome visible → snapshot and hide everything;
    /// - nothing visible and no snapshot → show everything (rescue arm, so
    ///   the action always has a visible effect).
    pub(crate) fn toggle_focus_mode(&mut self) {
        if self.light_mode_active() {
            self.exit_light_mode();
        }
        if let Some(restore) = self.modes.end_focus() {
            self.clear_focus_mode_toast();
            self.ui_visibility.show_status_bar = restore.show_status_bar();
            self.restore_toolbar_visibility(restore.toolbar_visibility());
            self.ui_visibility.show_floating_badge = restore.show_floating_badge();
            self.ui_visibility.show_zoom_chip = restore.show_zoom_chip();
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return;
        }

        // Only surfaces that can actually be on screen count: with all of
        // them already gone, "enter focus mode" would be a confusing no-op
        // snapshot of nothing — restore the full UI instead.
        let anything_to_hide = self.toolbar_visible()
            || self.status_hud_effectively_visible()
            || self.floating_badge_visible()
            || self.zoom_chip_enabled()
            || self.fallback_mode_badge_may_be_active();
        if !anything_to_hide {
            self.clear_all_chrome_recovery_toast();
            self.set_toolbar_visible(true);
            self.ui_visibility.show_status_bar = true;
            self.ui_visibility.show_floating_badge = true;
            self.ui_visibility.show_zoom_chip = true;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return;
        }

        let restore = FocusModeRestore::capture(
            self.ui_visibility.show_status_bar,
            self.toolbar_visibility_snapshot(),
            self.ui_visibility.show_floating_badge,
            self.ui_visibility.show_zoom_chip,
        );
        // Leave display mode untouched so a micro strip comes back as micro.
        self.hide_toolbar_visibility();
        self.ui_visibility.show_status_bar = false;
        self.ui_visibility.show_floating_badge = false;
        self.ui_visibility.show_zoom_chip = false;
        self.modes.begin_focus(restore);

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
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }
}

impl InputState {
    pub fn light_mode_supported(&self) -> bool {
        self.compositor_capabilities.layer_shell
    }

    fn light_mode_unsupported_message(&self) -> &'static str {
        let caps = self.compositor_capabilities;
        match (
            caps.desktop_environment,
            caps.shell_mode,
            caps.freeze_capture,
        ) {
            (DesktopEnvironment::Gnome, ShellMode::XdgFallback, true) => {
                "Light Mode passthrough is not supported in this GNOME Wayland session."
            }
            (DesktopEnvironment::Gnome, ShellMode::XdgFallback, false) => {
                "Light Mode passthrough is not supported in this GNOME Wayland session. Screen capture is also unavailable."
            }
            (_, ShellMode::XdgFallback, _) => {
                "Light Mode passthrough is not supported in this desktop session."
            }
            _ => "Light Mode passthrough is not supported by this compositor.",
        }
    }

    pub fn light_mode_passthrough(&self) -> bool {
        self.light_mode_supported() && self.light_mode_active() && !self.light_mode_drawing_active()
    }

    pub(crate) fn session_tool_override(&self) -> Option<Tool> {
        self.modes
            .light_restored_tool_override()
            .unwrap_or_else(|| self.tool_override())
    }

    pub(crate) fn session_active_tool(&self) -> Tool {
        self.session_tool_override()
            .unwrap_or_else(|| self.active_tool())
    }

    pub(crate) fn toggle_light_mode(&mut self) -> bool {
        if self.light_mode_active() {
            self.exit_light_mode();
        } else {
            if !self.light_mode_supported() {
                self.push_toast(
                    ToastPriority::Info,
                    "light_mode",
                    Toast::warning(self.light_mode_unsupported_message()),
                );
                self.needs_redraw = true;
                return false;
            }
            self.enter_light_mode(false);
        }
        self.light_mode_active()
    }

    pub fn toggle_light_mode_drawing(&mut self) -> bool {
        let drawing = if self.light_mode_active() {
            !self.light_mode_drawing_active()
        } else {
            true
        };
        self.set_light_mode_drawing(drawing)
    }

    pub fn set_light_mode_drawing(&mut self, drawing: bool) -> bool {
        if !self.light_mode_active() {
            if drawing {
                if !self.light_mode_supported() {
                    self.push_toast(
                        ToastPriority::Info,
                        "light_mode",
                        Toast::warning(self.light_mode_unsupported_message()),
                    );
                    self.needs_redraw = true;
                    return false;
                }
                self.enter_light_mode(true);
            }
            return self.light_mode_drawing_active();
        }

        if self.light_mode_drawing_active() == drawing {
            return self.light_mode_drawing_active();
        }

        self.cancel_active_interaction();
        self.modes.set_light_drawing(drawing);
        let message = if drawing {
            "Light Mode drawing"
        } else {
            "Light Mode passthrough"
        };
        self.push_toast(ToastPriority::Info, "light_mode", Toast::info(message));
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.light_mode_drawing_active()
    }

    pub(crate) fn exit_light_mode(&mut self) {
        if !self.light_mode_active() {
            return;
        }

        self.cancel_active_interaction();

        if let Some(restore) = self.modes.end_light() {
            self.ui_visibility.show_status_bar = restore.show_status_bar();
            self.ui_visibility.show_tool_preview = restore.show_tool_preview();
            self.restore_toolbar_visibility(restore.toolbar_visibility());
            self.set_tool_override(restore.tool_override());
            if self.click_highlight_enabled() != restore.click_highlight_enabled() {
                self.toggle_click_highlight();
            }
        }

        self.push_toast(
            ToastPriority::Info,
            "light_mode",
            Toast::info("Stopping Light Mode"),
        );
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    fn enter_light_mode(&mut self, drawing: bool) {
        if self.focus_mode_active() {
            self.toggle_focus_mode();
        }
        if self.presenter_mode_active() {
            self.toggle_presenter_mode();
        }

        self.cancel_active_interaction();
        self.close_context_menu();
        self.close_properties_panel();
        self.close_radial_menu();
        self.close_board_picker();
        self.close_color_picker_popup(false);
        if self.help_overlay.visible {
            self.toggle_help_overlay();
        }

        let restore = LightModeRestore::capture(
            self.ui_visibility.show_status_bar,
            self.ui_visibility.show_tool_preview,
            self.toolbar_visibility_snapshot(),
            self.click_highlight_enabled(),
            self.tool_override(),
        );

        self.ui_visibility.show_status_bar = false;
        self.ui_visibility.show_tool_preview = false;
        self.hide_toolbar_visibility();
        self.set_tool_override(Some(Tool::Pen));
        if self.click_highlight_forced_in_light_mode() && !self.click_highlight_enabled() {
            self.toggle_click_highlight();
        }

        self.modes.begin_light(drawing, restore);
        let message = if drawing {
            "Light Mode drawing"
        } else {
            "Light Mode passthrough"
        };
        self.push_toast(
            ToastPriority::Action,
            "light_mode",
            Toast::info(message).action("Exit", Action::ToggleLightMode),
        );
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }
}

impl InputState {
    pub(crate) fn toggle_presenter_mode(&mut self) -> bool {
        if self.presenter_mode_active() {
            self.stop_presenter_mode()
        } else {
            self.start_presenter_mode()
        }
    }

    fn stop_presenter_mode(&mut self) -> bool {
        let config = self.presenter_mode_config().clone();
        if let Some(restore) = self.modes.end_presenter() {
            if let Some(value) = restore.show_status_bar() {
                self.ui_visibility.show_status_bar = value;
            }
            if let Some(value) = restore.show_tool_preview() {
                self.ui_visibility.show_tool_preview = value;
            }
            if let Some(snapshot) = restore.toolbar_visibility() {
                self.restore_toolbar_visibility(snapshot);
            }
            if let Some(value) = restore.tool_override() {
                self.set_tool_override(value);
            }
            if let Some(value) = restore.click_highlight_enabled()
                && self.click_highlight_enabled() != value
            {
                self.toggle_click_highlight();
            }
            if let Some(value) = restore.input_hud_enabled() {
                self.set_input_hud_enabled(value);
            }
        }
        if config.show_toast {
            self.push_toast(
                ToastPriority::Info,
                "presenter",
                Toast::info("Stopping Presenter Mode"),
            );
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.presenter_mode_active()
    }

    fn start_presenter_mode(&mut self) -> bool {
        let config = self.presenter_mode_config().clone();
        if self.light_mode_active() {
            self.exit_light_mode();
        }
        if self.focus_mode_active() {
            // Restore Focus Mode's snapshot before Presenter Mode captures its
            // own chrome baseline. This keeps the two transient owners from
            // nesting and lets micro-toolbar presenter policy operate on the
            // real pre-Focus visibility.
            self.toggle_focus_mode();
        }

        if config.close_help_overlay && self.help_overlay.visible {
            self.toggle_help_overlay();
        }

        self.cancel_active_interaction();
        let restore = PresenterRestore::capture(
            &config,
            self.ui_visibility.show_status_bar,
            self.ui_visibility.show_tool_preview,
            self.toolbar_visibility_snapshot(),
            self.click_highlight_enabled(),
            self.input_hud_enabled(),
            self.tool_override(),
        );
        if config.hide_status_bar {
            self.ui_visibility.show_status_bar = false;
        }
        if config.hide_tool_preview {
            self.ui_visibility.show_tool_preview = false;
        }
        if config.hide_toolbars {
            match config.toolbar_mode {
                crate::config::PresenterToolbarMode::Hidden => {
                    self.hide_toolbar_visibility();
                }
                crate::config::PresenterToolbarMode::Micro => {
                    // The top strip stays up as the micro chip.
                    self.set_top_display_mode(crate::config::TopDisplayMode::Micro);
                }
            }
        }
        if !matches!(
            config.tool_behavior,
            crate::config::PresenterToolBehavior::Keep
        ) {
            self.set_tool_override(Some(Tool::Highlight));
        }
        if config.enable_click_highlight && !self.click_highlight_enabled() {
            self.toggle_click_highlight();
        }
        if config.enable_input_hud {
            self.set_input_hud_enabled(true);
        }

        self.modes.begin_presenter(restore);
        if config.show_toast {
            self.push_toast(
                ToastPriority::Action,
                "presenter",
                Toast::info("Presenter Mode active").action("Exit", Action::TogglePresenterMode),
            );
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.presenter_mode_active()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PresenterModeConfig, PresenterToolbarMode};
    use crate::input::state::core::ToolbarInteraction;
    use crate::input::tool::Tool;

    fn toolbar_visibility() -> crate::input::state::core::ToolbarVisibility {
        ToolbarInteraction::default().visibility_snapshot()
    }

    #[test]
    fn presenter_snapshot_round_trips_complete_toolbar_visibility_for_micro_mapping() {
        let config = PresenterModeConfig {
            hide_toolbars: true,
            toolbar_mode: PresenterToolbarMode::Micro,
            ..PresenterModeConfig::default()
        };
        let visibility = toolbar_visibility();
        let restore = PresenterRestore::capture(
            &config,
            true,
            true,
            visibility,
            false,
            false,
            Some(Tool::Arrow),
        );
        let mut modes = ChromeModes::new(config);

        modes.begin_presenter(restore);
        let restored = modes.end_presenter().expect("presenter restore");

        assert_eq!(restored.toolbar_visibility(), Some(visibility));
    }

    #[test]
    fn presenter_without_toolbar_hiding_has_no_toolbar_retarget() {
        let config = PresenterModeConfig {
            hide_toolbars: false,
            ..PresenterModeConfig::default()
        };
        let restore = PresenterRestore::capture(
            &config,
            true,
            true,
            toolbar_visibility(),
            false,
            false,
            None,
        );
        let mut modes = ChromeModes::new(config);

        modes.begin_presenter(restore);

        assert!(
            !modes.retarget_presenter_toolbar_display_mode(crate::config::TopDisplayMode::Micro)
        );
        assert_eq!(modes.end_presenter().unwrap().toolbar_visibility(), None);
    }

    #[test]
    fn ending_inactive_modes_returns_no_restore() {
        let mut modes = ChromeModes::new(PresenterModeConfig::default());

        assert!(modes.end_presenter().is_none());
        assert!(modes.end_focus().is_none());
        assert!(modes.end_light().is_none());
        assert!(!modes.presenter_active());
        assert!(!modes.focus_active());
        assert!(!modes.light_active());
    }

    #[test]
    fn focus_and_light_snapshots_round_trip_their_values() {
        let visibility = toolbar_visibility();
        let focus = FocusModeRestore::capture(false, visibility, true, false);
        let light = LightModeRestore::capture(false, true, visibility, true, Some(Tool::Pen));
        let mut modes = ChromeModes::new(PresenterModeConfig::default());

        modes.begin_focus(focus);
        let focus = modes.end_focus().expect("focus restore");
        assert!(!focus.show_status_bar());
        assert_eq!(focus.toolbar_visibility(), visibility);
        assert!(focus.show_floating_badge());
        assert!(!focus.show_zoom_chip());

        modes.begin_light(true, light);
        assert!(modes.light_active());
        assert!(modes.light_drawing());
        let light = modes.end_light().expect("light restore");
        assert!(!light.show_status_bar());
        assert!(light.show_tool_preview());
        assert_eq!(light.toolbar_visibility(), visibility);
        assert!(light.click_highlight_enabled());
        assert_eq!(light.tool_override(), Some(Tool::Pen));
        assert!(!modes.light_active());
        assert!(!modes.light_drawing());
    }
}
