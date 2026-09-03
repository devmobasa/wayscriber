use crate::config::{ToolbarItemId, ToolbarItemOrderGroup, ToolbarLayoutMode};
use crate::input::InputState;
use crate::input::state::TopMenuState;
use crate::ui::toolbar::ToolbarItemCustomizeGroup;

impl InputState {
    /// Close every open top-strip menu/popover (shapes picker, overflow, and
    /// the Canvas/Session/Settings popovers); returns whether anything was
    /// open. This is the single source of truth for the top-menu dismissal
    /// set — the builtin backend's click-away path and the keyboard Escape
    /// route both dismiss the same surfaces, so they defer to (or mirror) this
    /// list rather than re-enumerating it and drifting.
    pub(crate) fn close_top_toolbar_menus(&mut self) -> bool {
        let changed = self.toolbar.close_top_menu();
        if changed {
            self.needs_redraw = true;
        }
        changed
    }

    pub(super) fn apply_toolbar_toggle_custom_section(&mut self, enable: bool) -> bool {
        if self.history_limits.custom_section_enabled() != enable {
            self.history_limits.set_custom_section_enabled(enable);
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_delay_sliders(&mut self, show: bool) -> bool {
        if self.ui_visibility.show_delay_sliders != show {
            self.ui_visibility.show_delay_sliders = show;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_open_configurator(&mut self) -> bool {
        self.launch_configurator(None);
        true
    }

    pub(super) fn apply_toolbar_open_about(&mut self) -> bool {
        self.launch_about();
        true
    }

    pub(super) fn apply_toolbar_open_config_file(&mut self) -> bool {
        self.open_config_file_default();
        true
    }

    /// Minimize keeps the surface mapped as a small restore tab instead of
    /// hiding it, so a presenter who "closes" a bar is never stranded.
    pub(crate) fn apply_toolbar_set_top_minimized(&mut self, minimized: bool) -> bool {
        if !self.toolbar.set_top_minimized(minimized) {
            return false;
        }
        self.needs_redraw = true;
        true
    }

    /// Set the top strip's display form (micro chip click → `Full`).
    pub(super) fn apply_toolbar_set_top_display_mode(
        &mut self,
        mode: crate::config::TopDisplayMode,
    ) -> bool {
        // Same presenter gate as Action::CycleToolbarDisplay: while presenter
        // mode owns toolbar visibility (e.g. the micro chip mapping), a chip
        // click must neither override the mapping nor persist a display mode
        // the user never chose. Returning false also skips the event-policy
        // persistence for this event.
        if self.presenter_mode && self.presenter_mode_config.hide_toolbars {
            return false;
        }
        if self.top_display_state() == mode {
            return false;
        }
        self.set_top_display_mode(mode);
        true
    }

    pub(super) fn apply_toolbar_pin_top_toolbar(&mut self, pin: bool) -> bool {
        if self.toolbar.top_pinned() == pin {
            return false;
        }
        self.toolbar.set_top_pinned(pin);
        true
    }

    pub(super) fn apply_toolbar_toggle_icon_mode(&mut self, use_icons: bool) -> bool {
        if self.toolbar.use_icons() == use_icons {
            return false;
        }
        self.toolbar.set_use_icons(use_icons);
        true
    }

    pub(super) fn apply_toolbar_toggle_more_colors(&mut self, show: bool) -> bool {
        if self.ui_visibility.show_more_colors != show {
            self.ui_visibility.show_more_colors = show;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_actions_section(&mut self, show: bool) -> bool {
        self.apply_section_flag(crate::config::ToolbarSectionFlag::Actions, show)
    }

    pub(super) fn apply_toolbar_toggle_actions_advanced(&mut self, show: bool) -> bool {
        self.apply_section_flag(crate::config::ToolbarSectionFlag::ActionsAdvanced, show)
    }

    pub(super) fn apply_toolbar_toggle_zoom_actions(&mut self, show: bool) -> bool {
        self.apply_section_flag(crate::config::ToolbarSectionFlag::ZoomActions, show)
    }

    pub(super) fn apply_toolbar_toggle_pages_section(&mut self, show: bool) -> bool {
        self.apply_section_flag(crate::config::ToolbarSectionFlag::Pages, show)
    }

    pub(super) fn apply_toolbar_toggle_boards_section(&mut self, show: bool) -> bool {
        self.apply_section_flag(crate::config::ToolbarSectionFlag::Boards, show)
    }

    pub(super) fn apply_toolbar_toggle_presets(&mut self, show: bool) -> bool {
        self.apply_section_flag(crate::config::ToolbarSectionFlag::Presets, show)
    }

    pub(super) fn apply_toolbar_toggle_step_section(&mut self, show: bool) -> bool {
        self.apply_section_flag(crate::config::ToolbarSectionFlag::StepSection, show)
    }

    pub(super) fn apply_toolbar_toggle_text_controls(&mut self, show: bool) -> bool {
        self.apply_section_flag(crate::config::ToolbarSectionFlag::TextControls, show)
    }

    /// Applies a section's visibility restored from runtime-UI state.
    ///
    /// What persists is the explicit setting, not the visibility it resolves
    /// to, so `Default` restores a section to following the layout mode rather
    /// than pinning it to whatever that mode happened to show.
    pub(crate) fn apply_section_visibility_runtime(
        &mut self,
        flag: crate::config::ToolbarSectionFlag,
        setting: crate::config::ToolbarItemVisibilitySetting,
    ) {
        self.set_toolbar_item_visibility_setting(flag.item_id(), setting);
        self.refresh_section_visibility();
    }

    /// Section toggles record an explicit override in the item store (the
    /// single source of truth) and re-derive the mirror booleans, so the
    /// choice survives layout-mode switches.
    fn apply_section_flag(&mut self, flag: crate::config::ToolbarSectionFlag, show: bool) -> bool {
        if !self.toolbar.set_section_visibility(flag, show) {
            return false;
        }
        self.refresh_section_visibility();
        self.needs_redraw = true;
        true
    }

    pub(super) fn apply_toolbar_toggle_context_aware_ui(&mut self, enabled: bool) -> bool {
        if self.ui_visibility.context_aware_ui != enabled {
            self.ui_visibility.context_aware_ui = enabled;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_preset_toasts(&mut self, show: bool) -> bool {
        if self.ui_visibility.show_preset_toasts != show {
            self.ui_visibility.show_preset_toasts = show;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_idle_fade(&mut self, enable: bool) -> bool {
        if self.ui_visibility.idle_fade != enable {
            self.ui_visibility.idle_fade = enable;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_tool_preview(&mut self, show: bool) -> bool {
        if self.presenter_mode && self.presenter_mode_config.hide_tool_preview {
            return false;
        }
        if self.ui_visibility.show_tool_preview != show {
            self.ui_visibility.show_tool_preview = show;
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_status_bar(&mut self, show: bool) -> bool {
        if self.presenter_mode && self.presenter_mode_config.hide_status_bar {
            return false;
        }
        // This is an explicit chrome control, so it takes ownership from
        // Focus Mode just like the keyboard/palette toggle does.
        self.break_focus_mode();
        if self.ui_visibility.show_status_bar != show {
            self.ui_visibility.show_status_bar = show;
            self.refresh_status_hud_layout();
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_set_status_bar_interactive(&mut self, interactive: bool) -> bool {
        if self.ui_visibility.status_bar_interactive == interactive {
            return false;
        }
        self.ui_visibility.status_bar_interactive = interactive;
        self.status_hud.clear_hover();
        self.needs_redraw = true;
        true
    }

    pub(super) fn apply_toolbar_set_status_bar_item_visible(
        &mut self,
        item: crate::config::StatusBarItem,
        visible: bool,
    ) -> bool {
        self.set_status_bar_item_visible(item, visible)
    }

    pub(super) fn apply_toolbar_toggle_status_board_badge(&mut self, show: bool) -> bool {
        self.set_status_bar_item_visible(crate::config::StatusBarItem::Board, show)
    }

    pub(super) fn apply_toolbar_toggle_status_page_badge(&mut self, show: bool) -> bool {
        self.set_status_bar_item_visible(crate::config::StatusBarItem::Page, show)
    }

    pub(super) fn apply_toolbar_toggle_floating_badge_always(&mut self, show: bool) -> bool {
        if self.ui_visibility.show_floating_badge_always != show {
            self.ui_visibility.show_floating_badge_always = show;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_top_overflow(&mut self, open: bool) -> bool {
        let changed = self
            .toolbar
            .set_top_menu_open(TopMenuState::TopOverflow, open);
        if changed {
            self.needs_redraw = true;
        }
        changed
    }

    /// Open/close the Canvas popover. Opening it closes the Session/Settings
    /// popovers, the overflow menu, and the shapes picker, and resets the
    /// popovers' shared internal scroll.
    pub(super) fn apply_toolbar_toggle_canvas_popover(&mut self, open: bool) -> bool {
        let mut changed = self
            .toolbar
            .set_top_menu_open(TopMenuState::CanvasPopover, open);
        if open {
            self.pending_onboarding_usage.used_canvas_popover = true;
            changed |= self.reset_top_popover_scroll();
        }
        if changed {
            self.needs_redraw = true;
        }
        changed
    }

    /// Open/close the Session popover. Opening it closes the Settings/Canvas
    /// popovers, the overflow menu, and the shapes picker, and resets the
    /// popovers' shared internal scroll.
    pub(super) fn apply_toolbar_toggle_session_popover(&mut self, open: bool) -> bool {
        let mut changed = self
            .toolbar
            .set_top_menu_open(TopMenuState::SessionPopover, open);
        if open {
            changed |= self.reset_top_popover_scroll();
        }
        if changed {
            self.needs_redraw = true;
        }
        changed
    }

    /// Open/close the Settings popover; symmetric with the Session popover.
    pub(super) fn apply_toolbar_toggle_settings_popover(&mut self, open: bool) -> bool {
        let mut changed = self
            .toolbar
            .set_top_menu_open(TopMenuState::SettingsPopover, open);
        if open {
            changed |= self.reset_top_popover_scroll();
        }
        if changed {
            self.needs_redraw = true;
        }
        changed
    }

    fn reset_top_popover_scroll(&mut self) -> bool {
        self.toolbar.reset_top_popover_scroll()
    }

    /// Set the internal scroll offset of the open Canvas/Session/Settings
    /// popover.
    pub(super) fn apply_toolbar_scroll_top_popover(&mut self, offset: f64) -> bool {
        if !self.toolbar.set_top_popover_scroll(offset) {
            return false;
        }
        self.needs_redraw = true;
        true
    }

    /// Applies the layout preset restored from runtime-UI state.
    ///
    /// Restoring the mode must not re-run the preset's section defaults: the
    /// section settings restore on their own right after this, and applying
    /// defaults here would overwrite an explicit choice with the preset's
    /// baseline before it got the chance.
    pub(crate) fn apply_toolbar_layout_mode_runtime(&mut self, mode: ToolbarLayoutMode) {
        self.toolbar.apply_layout_mode(mode);
        self.refresh_section_visibility();
        self.needs_redraw = true;
    }

    pub(super) fn apply_toolbar_set_layout_mode(&mut self, mode: ToolbarLayoutMode) -> bool {
        if self.toolbar.layout_mode() == mode {
            return false;
        }
        self.toolbar.apply_layout_mode(mode);
        self.refresh_section_visibility();
        self.toolbar
            .set_top_menu_open(TopMenuState::ShapePicker, false);
        true
    }

    pub(super) fn apply_toolbar_set_item_hidden(
        &mut self,
        id: ToolbarItemId,
        hidden: bool,
    ) -> bool {
        self.set_toolbar_item_hidden(id, hidden)
    }

    pub(super) fn apply_toolbar_move_item(
        &mut self,
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
        delta: isize,
    ) -> bool {
        self.move_toolbar_item(group, id, delta)
    }

    pub(super) fn apply_toolbar_start_item_drag(
        &mut self,
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
    ) -> bool {
        self.start_toolbar_item_drag(group, id)
    }

    pub(super) fn apply_toolbar_drag_item_over(
        &mut self,
        group: ToolbarItemOrderGroup,
        target_index: usize,
    ) -> bool {
        self.drag_toolbar_item_over(group, target_index)
    }

    pub(super) fn apply_toolbar_reset_item_order(&mut self, group: ToolbarItemOrderGroup) -> bool {
        self.reset_toolbar_item_order(group)
    }

    pub(super) fn apply_toolbar_reset_item_hidden_overrides(&mut self) -> bool {
        self.reset_toolbar_item_hidden_overrides()
    }

    pub(super) fn apply_toolbar_set_item_customization_open(&mut self, open: bool) -> bool {
        if !self.toolbar.set_customize_items_open(open) {
            return false;
        }
        self.needs_redraw = true;
        true
    }

    pub(super) fn apply_toolbar_set_item_customization_group(
        &mut self,
        group: Option<ToolbarItemCustomizeGroup>,
    ) -> bool {
        if !self.toolbar.set_customize_items_group(group) {
            return false;
        }
        self.needs_redraw = true;
        true
    }

    pub(super) fn apply_toolbar_set_status_bar_contents_open(&mut self, open: bool) -> bool {
        if !self.toolbar.set_status_bar_contents_open(open) {
            return false;
        }
        self.needs_redraw = true;
        true
    }

    pub(super) fn apply_toolbar_toggle_shape_picker(&mut self, open: bool) -> bool {
        let changed = self
            .toolbar
            .set_top_menu_open(TopMenuState::ShapePicker, open);
        if changed {
            self.needs_redraw = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{ToolbarLayoutMode, ToolbarSectionFlag};
    use crate::input::state::{TopMenuState, test_support::make_test_input_state};
    use crate::ui::toolbar::ToolbarEvent;

    #[test]
    fn section_toggles_survive_layout_mode_switches() {
        let mut state = make_test_input_state();
        assert!(state.ui_visibility.show_zoom_actions);

        state.apply_toolbar_event(ToolbarEvent::ToggleZoomActions(false));
        assert!(!state.ui_visibility.show_zoom_actions);

        // The old behavior recomputed all section booleans from mode
        // defaults on every switch, erasing the choice.
        state.apply_toolbar_event(ToolbarEvent::SetToolbarLayoutMode(
            ToolbarLayoutMode::Simple,
        ));
        assert!(!state.ui_visibility.show_zoom_actions);
        state.apply_toolbar_event(ToolbarEvent::SetToolbarLayoutMode(
            ToolbarLayoutMode::Regular,
        ));
        assert!(!state.ui_visibility.show_zoom_actions);

        // Simple hides presets by baseline; an explicit show survives the
        // round trip through other modes.
        state.apply_toolbar_event(ToolbarEvent::SetToolbarLayoutMode(
            ToolbarLayoutMode::Simple,
        ));
        assert!(!state.ui_visibility.show_presets);
        state.apply_toolbar_event(ToolbarEvent::TogglePresets(true));
        assert!(state.ui_visibility.show_presets);
        state.apply_toolbar_event(ToolbarEvent::SetToolbarLayoutMode(
            ToolbarLayoutMode::Advanced,
        ));
        state.apply_toolbar_event(ToolbarEvent::SetToolbarLayoutMode(
            ToolbarLayoutMode::Simple,
        ));
        assert!(state.ui_visibility.show_presets);
        assert!(
            state
                .resolved_toolbar_items()
                .shown
                .contains(&ToolbarSectionFlag::Presets.item_id())
        );
    }

    #[test]
    fn minimize_keeps_the_strip_visible_and_close_is_an_alias() {
        let mut state = make_test_input_state();

        // The deprecated Close event now minimizes: the surface stays
        // visible as a restore tab instead of vanishing.
        state.apply_toolbar_event(ToolbarEvent::CloseTopToolbar);
        assert!(state.toolbar_top_minimized());
        assert!(state.toolbar_top_visible());

        state.apply_toolbar_event(ToolbarEvent::SetTopMinimized(false));
        assert!(!state.toolbar_top_minimized());
    }

    #[test]
    fn pin_application_defers_persistence_confirmation_to_backend() {
        let mut state = make_test_input_state();
        let top_pinned = !state.toolbar_top_pinned();

        assert!(state.apply_toolbar_event(ToolbarEvent::PinTopToolbar(top_pinned)));
        assert!(
            state.active_toast().is_none(),
            "input application does not yet know whether the runtime mutation is durable"
        );
    }

    #[test]
    fn minimizing_the_top_strip_closes_its_popups() {
        for menu in [
            TopMenuState::ShapePicker,
            TopMenuState::TopOverflow,
            TopMenuState::CanvasPopover,
            TopMenuState::SessionPopover,
            TopMenuState::SettingsPopover,
        ] {
            let mut state = make_test_input_state();
            state.test_set_toolbar_menu_state(menu, state.toolbar_top_popover_scroll());

            state.apply_toolbar_event(ToolbarEvent::SetTopMinimized(true));

            assert_eq!(state.toolbar_top_menu(), TopMenuState::Closed);
        }
    }

    #[test]
    fn session_and_settings_popovers_are_mutually_exclusive_with_the_top_menus() {
        let mut state = make_test_input_state();

        // Opening the Session popover closes the other top menus and
        // resets the shared internal scroll.
        state.apply_toolbar_event(ToolbarEvent::ToggleTopOverflow(true));
        state.test_set_toolbar_menu_state(state.toolbar_top_menu(), 40.0);
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleSessionPopover(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::SessionPopover);
        assert_eq!(state.toolbar_top_popover_scroll(), 0.0);

        // Opening the Settings popover closes the Session popover.
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleSettingsPopover(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::SettingsPopover);

        // Re-opening the overflow (or the shapes picker) closes an open
        // popover.
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleTopOverflow(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::TopOverflow);
        state.apply_toolbar_event(ToolbarEvent::ToggleSessionPopover(true));
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleShapePicker(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::ShapePicker);

        // Explicit close is a plain toggle.
        state.apply_toolbar_event(ToolbarEvent::ToggleSettingsPopover(true));
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleSettingsPopover(false)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::Closed);
    }

    #[test]
    fn canvas_popover_is_mutually_exclusive_with_the_other_top_menus() {
        let mut state = make_test_input_state();

        assert_canvas_open_closes_top_menus(&mut state);
        assert_canvas_is_exclusive_with_session_and_settings(&mut state);
        assert_overflow_and_shapes_close_canvas(&mut state);
        assert_minimize_and_close_handle_canvas(&mut state);
    }

    fn assert_canvas_open_closes_top_menus(state: &mut crate::input::InputState) {
        // Opening Canvas closes the overflow, the shapes picker, and resets
        // the shared internal scroll.
        state.apply_toolbar_event(ToolbarEvent::ToggleTopOverflow(true));
        state.test_set_toolbar_menu_state(state.toolbar_top_menu(), 40.0);
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleCanvasPopover(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::CanvasPopover);
        assert!(state.pending_onboarding_usage.used_canvas_popover);
        assert_eq!(state.toolbar_top_popover_scroll(), 0.0);
    }

    fn assert_canvas_is_exclusive_with_session_and_settings(state: &mut crate::input::InputState) {
        // Opening Session or Settings closes Canvas, and Canvas closes them.
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleSessionPopover(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::SessionPopover);
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleCanvasPopover(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::CanvasPopover);
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleSettingsPopover(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::SettingsPopover);
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleCanvasPopover(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::CanvasPopover);
    }

    fn assert_overflow_and_shapes_close_canvas(state: &mut crate::input::InputState) {
        // Re-opening the overflow (or the shapes picker) closes Canvas.
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleTopOverflow(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::TopOverflow);
        state.apply_toolbar_event(ToolbarEvent::ToggleCanvasPopover(true));
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleShapePicker(true)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::ShapePicker);
    }

    fn assert_minimize_and_close_handle_canvas(state: &mut crate::input::InputState) {
        // Minimizing the strip closes an open Canvas popover.
        state.apply_toolbar_event(ToolbarEvent::ToggleCanvasPopover(true));
        state.apply_toolbar_event(ToolbarEvent::SetTopMinimized(true));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::Closed);

        // Explicit close is a plain toggle, and the scroll event applies
        // while Canvas is open.
        state.apply_toolbar_event(ToolbarEvent::SetTopMinimized(false));
        state.apply_toolbar_event(ToolbarEvent::ToggleCanvasPopover(true));
        assert!(state.apply_toolbar_event(ToolbarEvent::ScrollTopPopover(24.0)));
        assert_eq!(state.toolbar_top_popover_scroll(), 24.0);
        assert!(state.apply_toolbar_event(ToolbarEvent::ToggleCanvasPopover(false)));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::Closed);
    }

    /// Canvas-click-away dismissal (the backend's `dismiss_top_toolbar_menus`
    /// defers to this canonical set): a lone Canvas popover dismisses and
    /// reports `changed`, exactly like the Session and Settings popovers, so
    /// the pointer handler early-returns instead of leaking the click through
    /// to the canvas as a stray stroke.
    #[test]
    fn close_top_toolbar_menus_dismisses_each_top_popover_including_canvas() {
        for open in [
            ToolbarEvent::ToggleCanvasPopover(true),
            ToolbarEvent::ToggleSessionPopover(true),
            ToolbarEvent::ToggleSettingsPopover(true),
        ] {
            let mut state = make_test_input_state();
            state.apply_toolbar_event(open);
            assert!(state.toolbar_top_menu().is_popover());

            // The click-away reports it dismissed something (so the backend
            // early-returns) and leaves every top menu closed.
            assert!(state.close_top_toolbar_menus());
            assert_eq!(state.toolbar_top_menu(), TopMenuState::Closed);

            // Nothing left open: a second click-away is a no-op (no phantom
            // early-return that would swallow a genuine canvas stroke).
            assert!(!state.close_top_toolbar_menus());
        }
    }

    /// The touch-down and tablet pen-down paths now run the same canvas
    /// click-away guard the mouse path uses: with a top popover open, a canvas
    /// down dismisses it and is swallowed before any stroke starts. The
    /// backend handlers gate on `dismiss_top_toolbar_menus` (which defers to
    /// this canonical `close_top_toolbar_menus`) with an early-return, so a
    /// reported dismissal means no `on_mouse_press` runs and no stray stroke
    /// begins — modelled here at the state level that both modalities share.
    #[test]
    fn canvas_down_click_away_dismisses_the_popover_without_a_stray_stroke() {
        use crate::input::{DrawingState, MouseButton};

        for open in [
            ToolbarEvent::ToggleCanvasPopover(true),
            ToolbarEvent::ToggleSessionPopover(true),
            ToolbarEvent::ToggleSettingsPopover(true),
        ] {
            let mut state = make_test_input_state();
            state.apply_toolbar_event(open);

            // The down path's guard fires first: it reports a dismissal, so the
            // handler early-returns instead of pressing into the canvas.
            let swallowed = state.close_top_toolbar_menus();
            assert!(swallowed, "the canvas down dismisses the open popover");
            assert_eq!(state.toolbar_top_menu(), TopMenuState::Closed);
            // Nothing was pressed into the canvas: still Idle, no shapes.
            assert!(matches!(state.state, DrawingState::Idle));
            assert_eq!(state.boards.active_frame().shapes.len(), 0);
        }

        // Control: with nothing open the guard reports no dismissal, so the
        // same down path proceeds to start a stroke — proving the guard, not an
        // unrelated block, is what swallows the down above.
        let mut state = make_test_input_state();
        assert!(!state.close_top_toolbar_menus());
        state.on_mouse_press_with_canvas(MouseButton::Left, 10, 10, 10, 10);
        assert!(matches!(state.state, DrawingState::Drawing { .. }));
    }

    #[test]
    fn top_popover_scroll_applies_only_while_a_popover_is_open() {
        let mut state = make_test_input_state();

        // No popover open: the scroll event is a no-op.
        assert!(!state.apply_toolbar_event(ToolbarEvent::ScrollTopPopover(24.0)));
        assert_eq!(state.toolbar_top_popover_scroll(), 0.0);

        state.apply_toolbar_event(ToolbarEvent::ToggleSettingsPopover(true));
        assert!(state.apply_toolbar_event(ToolbarEvent::ScrollTopPopover(24.0)));
        assert_eq!(state.toolbar_top_popover_scroll(), 24.0);
        // Negative offsets clamp to the top.
        assert!(state.apply_toolbar_event(ToolbarEvent::ScrollTopPopover(-5.0)));
        assert_eq!(state.toolbar_top_popover_scroll(), 0.0);

        // Switching popovers resets the scroll.
        state.apply_toolbar_event(ToolbarEvent::ScrollTopPopover(31.0));
        state.apply_toolbar_event(ToolbarEvent::ToggleSessionPopover(true));
        assert_eq!(state.toolbar_top_popover_scroll(), 0.0);
    }

    #[test]
    fn top_menus_are_mutually_exclusive() {
        let mut state = make_test_input_state();

        state.apply_toolbar_event(ToolbarEvent::ToggleShapePicker(true));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::ShapePicker);
        state.apply_toolbar_event(ToolbarEvent::ToggleTopOverflow(true));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::TopOverflow);

        state.apply_toolbar_event(ToolbarEvent::ToggleShapePicker(true));
        assert_eq!(state.toolbar_top_menu(), TopMenuState::ShapePicker);
    }

    #[test]
    fn selecting_from_top_menu_closes_it() {
        let mut state = make_test_input_state();
        state.test_set_toolbar_menu_state(
            TopMenuState::TopOverflow,
            state.toolbar_top_popover_scroll(),
        );

        state.apply_toolbar_event(ToolbarEvent::SelectTool(crate::input::Tool::Line));

        assert_eq!(state.toolbar_top_menu(), TopMenuState::Closed);
    }

    #[test]
    fn customize_checkbox_and_section_toggle_share_one_store() {
        let mut state = make_test_input_state();

        // Hide the presets section through the item store (the customize
        // checkbox path) — the section boolean mirrors it.
        state.apply_toolbar_event(ToolbarEvent::SetToolbarItemHidden(
            ToolbarSectionFlag::Presets.item_id(),
            true,
        ));
        assert!(!state.ui_visibility.show_presets);

        // Re-enable through the Settings toggle — the hidden entry is
        // replaced, not fought, so the section comes back.
        state.apply_toolbar_event(ToolbarEvent::TogglePresets(true));
        assert!(state.ui_visibility.show_presets);
        assert!(
            !state
                .resolved_toolbar_items()
                .hidden
                .contains(&ToolbarSectionFlag::Presets.item_id())
        );
    }
}
