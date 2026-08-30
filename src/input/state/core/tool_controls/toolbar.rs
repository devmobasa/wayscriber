use super::super::base::InputState;
use crate::config::{
    RadialMenuMouseBinding, ToolbarItemId, ToolbarItemOrderGroup, ToolbarItemVisibilitySetting,
    TopDisplayMode, factory_individual_toolbar_item_visibility_settings,
};
use crate::domain::Action;
use crate::input::state::{Toast, ToastPriority, TopMenuState};

/// How long the "Cleared — Undo?" toast stays up after a mouse-path clear.
pub(crate) const CLEAR_UNDO_TOAST_MS: u64 = 2000;

impl InputState {
    /// Sets the toolbar visibility flag. Returns true if toggled.
    pub fn set_toolbar_visible(&mut self, visible: bool) -> bool {
        // Showing must also count a cycle-hidden (F2) top strip as a change:
        // the raw flag can be true while no surface is visible, and the
        // raw-flag comparison alone would swallow the restore (F9, the
        // onboarding toast's Show action, and the status-bar hint chip all
        // dispatch ToggleToolbar into this setter).
        let unhide_top = visible && self.toolbar_top_display_mode == TopDisplayMode::Hidden;
        let any_change =
            unhide_top || self.toolbar_visible != visible || self.toolbar_top_visible != visible;

        if !any_change {
            return false;
        }

        self.toolbar_visible = visible;
        self.toolbar_top_visible = visible;
        // Showing toolbars always brings the top strip back: a cycle-hidden
        // strip (F2) reverts to its full form when F9 shows the bars again.
        if visible && self.toolbar_top_display_mode == TopDisplayMode::Hidden {
            self.toolbar_top_display_mode = TopDisplayMode::Full;
        }
        self.refresh_status_hud_layout();
        self.needs_redraw = true;
        true
    }

    /// Re-derive the live visibility flags from the pin flag — the
    /// same rule startup applies — and refresh the status HUD layout, which
    /// follows toolbar visibility (see `set_toolbar_visible`). Used when a
    /// rolled-back visibility toggle hands the pre-toggle pins back:
    /// visibility itself is never persisted, so it must be recomputed for
    /// the screen to match what a restart would show.
    pub(crate) fn derive_toolbar_visibility_from_pins(&mut self) {
        let top = self.toolbar_top_pinned;
        // A rollback can resolve long after the toggle (a failed write
        // barrier holds it), by which time a transient chrome owner —
        // focus mode, presenter mode with `hide_toolbars`, or light mode —
        // may have taken toolbar visibility. Writing the live flags then
        // would surface toolbars out from under the owner while its
        // restore snapshot still held the post-toggle state, so exit would
        // restore the wrong screen. Write the derived values into the
        // owner's snapshot instead (the presenter-aware pattern of
        // `apply_persisted_top_display_mode`): the owner keeps its screen
        // now and hands back pin-agreeing visibility on exit. The three
        // owners never nest — presenter entry restores focus and exits
        // light, focus entry is presenter-gated and exits light, light
        // entry restores focus and exits presenter — so at most one
        // snapshot exists and there is no restore-order ambiguity.
        if let Some(restore) = self.focus_mode_restore.as_mut() {
            restore.toolbar_top_visible = top;
            restore.toolbar_visible = top;
            return;
        }
        // Presenter tracks toolbar visibility only when `hide_toolbars`
        // took it (the three fields are `Some` together); otherwise the
        // live flags are still the user's and are written below.
        if let Some(restore) = self.presenter_restore.as_mut()
            && restore.toolbar_visible.is_some()
        {
            restore.toolbar_top_visible = Some(top);
            restore.toolbar_visible = Some(top);
            return;
        }
        if let Some(restore) = self.light_mode_restore.as_mut() {
            restore.toolbar_top_visible = top;
            restore.toolbar_visible = top;
            return;
        }
        self.toolbar_top_visible = top;
        self.toolbar_visible = top;
        self.refresh_status_hud_layout();
    }

    /// After hiding a chrome surface: if nothing interactive remains on
    /// screen (no toolbar surface, no effective status HUD), teach the way
    /// back right now — the status-bar hint chip cannot help once the HUD
    /// itself is gone. Skipped only when presenter mode will restore a
    /// surface that was visible before it took ownership of that surface.
    pub(crate) fn warn_if_all_chrome_hidden(&mut self) {
        if self.toolbar_visible()
            || self.status_hud_effectively_visible()
            || self.presenter_will_restore_visible_chrome()
        {
            return;
        }
        let mut parts = Vec::new();
        if let Some(binding) = self.action_binding_primary_label(Action::ToggleToolbar) {
            parts.push(format!("{binding} toolbar"));
        }
        if let Some(binding) = self.action_binding_primary_label(Action::ToggleStatusBar) {
            parts.push(format!("{binding} status bar"));
        }
        let message = if parts.is_empty() && self.right_click_chrome_recovery_available() {
            "All UI hidden — right-click to restore".to_string()
        } else if parts.is_empty() {
            "All UI hidden — select the recovery action".to_string()
        } else {
            format!("All UI hidden — {}", parts.join(" · "))
        };
        let (action_label, recovery_action) =
            if self.presenter_mode && self.presenter_mode_config.hide_toolbars {
                ("Show status bar", Action::ToggleStatusBar)
            } else {
                ("Show toolbar", Action::ToggleToolbar)
            };
        // Same key as the routine chrome toasts ("Toolbar: hidden"), so this
        // supersedes them in place instead of queueing behind them. The
        // action chip gives one-click recovery even when the context menu is
        // unavailable; the message covers the bindings for both.
        self.push_toast(
            ToastPriority::Info,
            "ui",
            Toast::info(message).action(action_label, recovery_action),
        );
    }

    fn right_click_chrome_recovery_available(&self) -> bool {
        self.context_menu_enabled()
            && !self.zoom_active()
            && self.radial_menu_mouse_binding != RadialMenuMouseBinding::Right
    }

    fn presenter_will_restore_visible_chrome(&self) -> bool {
        if !self.presenter_mode {
            return false;
        }
        let Some(restore) = self.presenter_restore.as_ref() else {
            return false;
        };

        let restores_status_bar = restore.show_status_bar == Some(true);
        let restored_top_mode = restore
            .toolbar_top_display_mode
            .unwrap_or(self.toolbar_top_display_mode);
        let restores_top_toolbar = restore.toolbar_top_visible == Some(true)
            && restored_top_mode != TopDisplayMode::Hidden;

        restores_status_bar || restores_top_toolbar
    }

    /// Returns whether the toolbar is effectively visible: the top strip and
    /// not cycle-hidden. A raw visibility flag that cannot produce a surface
    /// does not count, so the F9 toggle always has a visible effect on its
    /// first press.
    pub fn toolbar_visible(&self) -> bool {
        self.toolbar_top_visible()
    }

    /// Returns whether the top toolbar surface is visible. The cycle
    /// action's Hidden display mode hides the top strip.
    pub fn toolbar_top_visible(&self) -> bool {
        self.toolbar_top_visible && self.toolbar_top_display_mode != TopDisplayMode::Hidden
    }

    /// Store the configured toolbar shortcut-rebind modifier (called at
    /// startup) so onboarding copy can name the chord without hardcoding keys.
    pub fn init_toolbar_rebind_modifier_from_config(
        &mut self,
        modifier: crate::config::ToolbarRebindModifier,
    ) {
        self.toolbar_rebind_modifier = modifier;
    }

    /// Initialize toolbar visibility from config (called at startup).
    #[allow(clippy::too_many_arguments)]
    pub fn init_toolbar_from_config(
        &mut self,
        layout_mode: crate::config::ToolbarLayoutMode,
        mode_overrides: crate::config::ToolbarModeOverrides,
        items: crate::config::ToolbarItemsConfig,
        top_pinned: bool,
        use_icons: bool,
        scale: f64,
        show_more_colors: bool,
        show_actions_section: bool,
        show_actions_advanced: bool,
        show_zoom_actions: bool,
        show_pages_section: bool,
        show_boards_section: bool,
        show_presets: bool,
        show_step_section: bool,
        show_text_controls: bool,
        context_aware_ui: bool,
        show_delay_sliders: bool,
        show_marker_opacity_section: bool,
        show_preset_toasts: bool,
        idle_fade: bool,
        show_tool_preview: bool,
    ) {
        self.toolbar_top_pinned = top_pinned;
        self.toolbar_top_visible = top_pinned;
        self.toolbar_visible = top_pinned;
        self.toolbar_use_icons = use_icons;
        self.toolbar_scale = scale;
        self.toolbar_layout_mode = layout_mode;
        self.toolbar_mode_overrides = mode_overrides;
        self.resolved_toolbar_items = items.resolved();
        self.toolbar_items = items;
        self.show_more_colors = show_more_colors;
        self.show_actions_section = show_actions_section;
        self.show_actions_advanced = show_actions_advanced;
        self.show_zoom_actions = show_zoom_actions;
        self.show_pages_section = show_pages_section;
        self.show_boards_section = show_boards_section;
        self.show_presets = show_presets;
        self.show_step_section = show_step_section;
        self.show_text_controls = show_text_controls;
        self.context_aware_ui = context_aware_ui;
        self.show_delay_sliders = show_delay_sliders;
        self.show_marker_opacity_section = show_marker_opacity_section;
        self.show_preset_toasts = show_preset_toasts;
        self.idle_fade = idle_fade;
        self.show_tool_preview = show_tool_preview;
        // Fold the legacy show_* booleans into explicit item overrides,
        // then re-derive them from the one resolver. Effective visibility
        // is bit-identical; the overrides now survive mode switches.
        let mut legacy = crate::config::ToolbarSectionVisibility {
            show_actions_section: self.show_actions_section,
            show_actions_advanced: self.show_actions_advanced,
            show_zoom_actions: self.show_zoom_actions,
            show_pages_section: self.show_pages_section,
            show_boards_section: self.show_boards_section,
            show_presets: self.show_presets,
            show_step_section: self.show_step_section,
            show_text_controls: self.show_text_controls,
        };
        legacy.apply_mode_override(self.toolbar_mode_overrides.for_mode(layout_mode));
        if crate::config::fold_legacy_section_flags(
            &legacy,
            layout_mode,
            &self.toolbar_mode_overrides,
            &mut self.toolbar_items,
        ) {
            self.resolved_toolbar_items = self.toolbar_items.resolved();
        }
        self.refresh_section_visibility();
    }

    /// Re-derive the live section booleans from the visibility resolver.
    /// They stay as fields (and config keys) purely as mirrors: every read
    /// site keeps working and older versions can still read the config.
    pub(crate) fn refresh_section_visibility(&mut self) {
        let visibility = crate::config::resolve_section_visibility(
            self.toolbar_layout_mode,
            &self.toolbar_mode_overrides,
            &self.resolved_toolbar_items,
        );
        self.show_actions_section = visibility.show_actions_section;
        self.show_actions_advanced = visibility.show_actions_advanced;
        self.show_zoom_actions = visibility.show_zoom_actions;
        self.show_pages_section = visibility.show_pages_section;
        self.show_boards_section = visibility.show_boards_section;
        self.show_presets = visibility.show_presets;
        self.show_step_section = visibility.show_step_section;
        self.show_text_controls = visibility.show_text_controls;
    }

    /// Restore the persisted minimize state of the top strip (called at
    /// startup). A minimized strip comes back as its edge restore tab.
    pub fn init_toolbar_minimized_from_config(&mut self, top: bool) {
        self.toolbar_top_minimized = top;
    }

    /// Restore the persisted top-strip display form (called at startup).
    /// `Hidden` sanitizes to `Full`: hidden is runtime-only, startup
    /// visibility stays governed by `top_pinned`.
    pub fn init_toolbar_display_mode_from_config(&mut self, mode: TopDisplayMode) {
        self.toolbar_top_display_mode = mode.persisted();
    }

    /// Effective display state of the top strip: `Hidden` when the strip
    /// surface is not visible (either via the cycle action or a plain
    /// visibility toggle), otherwise the current form. A minimized strip
    /// reports `Full` — minimize is a sibling feature and wins over micro.
    pub fn top_display_state(&self) -> TopDisplayMode {
        if !self.toolbar_top_visible() {
            TopDisplayMode::Hidden
        } else if self.toolbar_top_display_mode == TopDisplayMode::Micro
            && !self.toolbar_top_minimized
        {
            TopDisplayMode::Micro
        } else {
            TopDisplayMode::Full
        }
    }

    /// Put the top strip into `mode`. `Full` and `Micro` also make the top
    /// strip visible; entering `Micro` un-minimizes the strip (micro and
    /// minimized are mutually exclusive through the UI paths) and closes
    /// the strip's menus, like minimize does.
    pub(crate) fn set_top_display_mode(&mut self, mode: TopDisplayMode) {
        self.toolbar_top_display_mode = mode;
        self.refresh_status_hud_layout();
        match mode {
            TopDisplayMode::Full => {
                self.show_top_strip_surface();
            }
            TopDisplayMode::Micro => {
                self.toolbar_top_minimized = false;
                self.toolbar_top_menu = TopMenuState::Closed;
                self.show_top_strip_surface();
            }
            TopDisplayMode::Hidden => {
                self.toolbar_top_menu = TopMenuState::Closed;
            }
        }
        self.needs_redraw = true;
    }

    fn show_top_strip_surface(&mut self) {
        self.toolbar_top_visible = true;
        self.toolbar_visible = true;
    }

    /// Advance the top strip through Full → Micro → Hidden → Full and
    /// return the new state.
    pub fn cycle_top_toolbar_display(&mut self) -> TopDisplayMode {
        let next = match self.top_display_state() {
            TopDisplayMode::Full => TopDisplayMode::Micro,
            TopDisplayMode::Micro => TopDisplayMode::Hidden,
            TopDisplayMode::Hidden => TopDisplayMode::Full,
        };
        self.set_top_display_mode(next);
        next
    }

    pub fn set_toolbar_item_hidden(&mut self, id: ToolbarItemId, hidden: bool) -> bool {
        let before = self.toolbar_items.clone();
        self.toolbar_items.set_hidden(id, hidden);
        if self.toolbar_items == before {
            return false;
        }
        self.resolved_toolbar_items = self.toolbar_items.resolved();
        self.refresh_section_visibility();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn set_toolbar_item_visibility_setting(
        &mut self,
        id: ToolbarItemId,
        setting: ToolbarItemVisibilitySetting,
    ) -> bool {
        if !self.toolbar_items.set_visibility_setting(id, setting) {
            return false;
        }
        self.resolved_toolbar_items = self.toolbar_items.resolved();
        self.refresh_section_visibility();
        self.needs_redraw = true;
        true
    }

    pub fn reset_toolbar_item_hidden_overrides(&mut self) -> bool {
        let mut changed = false;
        for (&id, &setting) in factory_individual_toolbar_item_visibility_settings() {
            changed |= self.toolbar_items.set_visibility_setting(id, setting);
        }
        if !changed {
            return false;
        }
        self.resolved_toolbar_items = self.toolbar_items.resolved();
        self.refresh_section_visibility();
        self.needs_redraw = true;
        true
    }

    pub fn move_toolbar_item(
        &mut self,
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
        delta: isize,
    ) -> bool {
        if !self.toolbar_items.move_item_by(group, id, delta) {
            return false;
        }

        self.resolved_toolbar_items = self.toolbar_items.resolved();
        self.needs_redraw = true;
        true
    }

    pub fn start_toolbar_item_drag(
        &mut self,
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
    ) -> bool {
        if self.toolbar_customize_drag == Some((group, id)) {
            return false;
        }

        self.toolbar_customize_drag = Some((group, id));
        true
    }

    pub fn drag_toolbar_item_over(
        &mut self,
        group: ToolbarItemOrderGroup,
        target_index: usize,
    ) -> bool {
        let Some((source_group, id)) = self.toolbar_customize_drag else {
            return false;
        };
        if source_group != group {
            return false;
        }

        if !self
            .toolbar_items
            .move_item_to_index(group, id, target_index)
        {
            return false;
        }

        self.resolved_toolbar_items = self.toolbar_items.resolved();
        self.needs_redraw = true;
        true
    }

    pub fn clear_toolbar_item_drag(&mut self) {
        self.toolbar_customize_drag = None;
    }

    pub(crate) fn set_toolbar_item_order(
        &mut self,
        group: ToolbarItemOrderGroup,
        order: &[ToolbarItemId],
    ) -> bool {
        if !self.toolbar_items.set_known_order(group, order) {
            return false;
        }
        self.resolved_toolbar_items = self.toolbar_items.resolved();
        self.needs_redraw = true;
        true
    }

    pub fn reset_toolbar_item_order(&mut self, group: ToolbarItemOrderGroup) -> bool {
        if !self.toolbar_items.reset_known_order_to_defaults(group) {
            return false;
        }

        self.resolved_toolbar_items = self.toolbar_items.resolved();
        self.needs_redraw = true;
        true
    }

    /// Layout-mode switches re-resolve the section booleans against the new
    /// baseline; explicit user overrides in the item store survive, so a
    /// mode switch no longer erases hand-tuned section settings.
    pub(crate) fn apply_toolbar_mode_defaults(&mut self, _mode: crate::config::ToolbarLayoutMode) {
        self.refresh_section_visibility();
    }

    /// Wrapper for undo that preserves existing action plumbing.
    pub fn toolbar_undo(&mut self) {
        self.handle_action(Action::Undo);
    }

    /// Wrapper for redo that preserves existing action plumbing.
    pub fn toolbar_redo(&mut self) {
        self.handle_action(Action::Redo);
    }

    /// Wrapper for clear that preserves existing action plumbing.
    pub fn toolbar_clear(&mut self) {
        self.handle_action(Action::ClearCanvas);
    }

    /// Mouse-path clear: clears like `Action::ClearCanvas` and, when shapes
    /// were removed without a locked-shape warning, offers a short toast with
    /// an "Undo?" chip. The keyboard action and Shift+click stay instant.
    pub fn toolbar_clear_with_undo_toast(&mut self) {
        let (has_locked, has_unlocked) = {
            let frame = self.boards.active_frame();
            (
                frame.shapes.iter().any(|shape| shape.locked),
                frame.shapes.iter().any(|shape| !shape.locked),
            )
        };
        self.toolbar_clear();
        // The locked-shape paths already raise their own warning toasts in
        // `handle_action`; only the silent success path gets the undo offer.
        if has_unlocked && !has_locked {
            self.push_toast(
                ToastPriority::Action,
                "canvas.clear",
                Toast::info("Cleared")
                    .action("Undo?", Action::Undo)
                    .duration_ms(CLEAR_UNDO_TOAST_MS),
            );
        }
    }

    /// Wrapper for entering text mode.
    pub fn toolbar_enter_text_mode(&mut self) {
        self.handle_action(Action::EnterTextMode);
    }

    /// Wrapper for entering sticky note mode.
    pub fn toolbar_enter_sticky_note_mode(&mut self) {
        self.handle_action(Action::EnterStickyNoteMode);
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{ToolbarItemsConfig, toolbar_item_ids as ids};
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn init_folds_legacy_section_booleans_into_explicit_overrides() {
        let mut state = make_test_input_state();
        // A legacy Regular config where zoom actions were turned off and
        // everything else matches the baseline.
        state.init_toolbar_from_config(
            crate::config::ToolbarLayoutMode::Regular,
            crate::config::ToolbarModeOverrides::default(),
            crate::config::ToolbarItemsConfig::default(),
            true,
            true,
            1.0,
            false,
            true,  // actions
            false, // advanced
            false, // zoom — differs from the Regular baseline
            true,  // pages
            true,  // boards
            true,  // presets
            false, // step
            true,  // text controls
            true,  // context aware ui
            false,
            false,
            true,
            true,
            false,
        );

        // Effective visibility is bit-identical to the legacy booleans...
        assert!(!state.show_zoom_actions);
        assert!(state.show_presets);
        // ...and the disagreement is now an explicit override that
        // survives mode switches.
        let zoom_id = crate::config::ToolbarSectionFlag::ZoomActions.item_id();
        assert!(state.resolved_toolbar_items.hidden.contains(&zoom_id));
        state.apply_toolbar_event(crate::ui::toolbar::ToolbarEvent::SetToolbarLayoutMode(
            crate::config::ToolbarLayoutMode::Advanced,
        ));
        assert!(!state.show_zoom_actions);
    }

    #[test]
    fn factory_visibility_reset_changes_only_the_centralized_eligible_set() {
        let mut state = make_test_input_state();
        let section = crate::config::ToolbarSectionFlag::Actions.item_id();
        state.toolbar_items = ToolbarItemsConfig::default();
        state
            .toolbar_items
            .set_hidden(ids::TOP_UTILITY_SCREENSHOT, false);
        state.toolbar_items.set_hidden(ids::TOP_UTILITY_OCR, false);
        state.toolbar_items.set_hidden(ids::TOP_TOOL_PEN, true);
        state.toolbar_items.set_hidden(section, true);
        state
            .toolbar_items
            .set_hidden(ids::TOP_CHROME_OVERFLOW, true);
        state
            .toolbar_items
            .hidden
            .push("future.toolbar.item".to_string());
        state.resolved_toolbar_items = state.toolbar_items.resolved();

        assert!(state.reset_toolbar_item_hidden_overrides());

        let resolved = state.toolbar_items.resolved();
        assert!(resolved.hidden.contains(&ids::TOP_UTILITY_SCREENSHOT));
        // Restored to its baseline rather than to an explicit entry.
        assert!(resolved.is_hidden(ids::TOP_UTILITY_OCR));
        assert!(!resolved.hidden.contains(&ids::TOP_UTILITY_OCR));
        assert!(!resolved.hidden.contains(&ids::TOP_TOOL_PEN));
        assert!(resolved.hidden.contains(&section));
        assert!(resolved.hidden.contains(&ids::TOP_CHROME_OVERFLOW));
        assert_eq!(resolved.unknown_hidden, ["future.toolbar.item"]);
        assert!(!state.reset_toolbar_item_hidden_overrides());
    }
}
