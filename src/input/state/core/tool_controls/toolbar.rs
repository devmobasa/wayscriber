use super::super::base::{InputState, UiToastKind};
use crate::config::{ToolbarItemId, ToolbarItemOrderGroup, TopDisplayMode};
use crate::domain::Action;

/// How long the "Cleared — Undo?" toast stays up after a mouse-path clear.
pub(crate) const CLEAR_UNDO_TOAST_MS: u64 = 2000;

impl InputState {
    /// Sets toolbar visibility flag (controls both top and side). Returns true if toggled.
    pub fn set_toolbar_visible(&mut self, visible: bool) -> bool {
        let any_change = self.toolbar_visible != visible
            || self.toolbar_top_visible != visible
            || self.toolbar_side_visible != visible;

        if !any_change {
            return false;
        }

        self.toolbar_visible = visible;
        self.toolbar_top_visible = visible;
        self.toolbar_side_visible = visible;
        // Showing toolbars always brings the top strip back: a cycle-hidden
        // strip (F2) reverts to its full form when F9 shows the bars again.
        if visible && self.toolbar_top_display_mode == TopDisplayMode::Hidden {
            self.toolbar_top_display_mode = TopDisplayMode::Full;
        }
        self.needs_redraw = true;
        true
    }

    /// Returns whether any toolbar surface is effectively visible: the top
    /// strip (not cycle-hidden) or the side palette (not retired by
    /// `side_layout = "pill"`). Raw visibility flags that cannot produce a
    /// surface do not count, so the F9 toggle always has a visible effect
    /// on its first press.
    pub fn toolbar_visible(&self) -> bool {
        self.toolbar_top_visible() || self.toolbar_side_visible()
    }

    /// Returns whether the top toolbar surface is visible. The cycle
    /// action's Hidden display mode hides the top strip without touching
    /// the side toolbar.
    pub fn toolbar_top_visible(&self) -> bool {
        self.toolbar_top_visible && self.toolbar_top_display_mode != TopDisplayMode::Hidden
    }

    /// Returns whether the side toolbar is visible. Under the opt-in
    /// `side_layout = "pill"` the side palette is retired: its surface never
    /// appears (layer-shell, inline fallback, or GTK), regardless of the
    /// visibility toggles. The default `"panel"` keeps the classic
    /// behavior.
    pub fn toolbar_side_visible(&self) -> bool {
        self.toolbar_side_visible
            && self.toolbar_side_layout == crate::config::ToolbarSideLayout::Panel
    }

    /// Restore the persisted side layout (called at startup).
    pub fn init_toolbar_side_layout_from_config(
        &mut self,
        layout: crate::config::ToolbarSideLayout,
    ) {
        self.toolbar_side_layout = layout;
    }

    /// Initialize toolbar visibility from config (called at startup).
    #[allow(clippy::too_many_arguments)]
    pub fn init_toolbar_from_config(
        &mut self,
        layout_mode: crate::config::ToolbarLayoutMode,
        mode_overrides: crate::config::ToolbarModeOverrides,
        items: crate::config::ToolbarItemsConfig,
        top_pinned: bool,
        side_pinned: bool,
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
        show_settings_section: bool,
        show_delay_sliders: bool,
        show_marker_opacity_section: bool,
        show_preset_toasts: bool,
        show_tool_preview: bool,
    ) {
        self.toolbar_top_pinned = top_pinned;
        self.toolbar_side_pinned = side_pinned;
        self.toolbar_top_visible = top_pinned;
        self.toolbar_side_visible = side_pinned;
        self.toolbar_visible = top_pinned || side_pinned;
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
        self.show_settings_section = show_settings_section;
        self.show_delay_sliders = show_delay_sliders;
        self.show_marker_opacity_section = show_marker_opacity_section;
        self.show_preset_toasts = show_preset_toasts;
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
            show_settings_section: self.show_settings_section,
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
        self.show_settings_section = visibility.show_settings_section;
    }

    /// Restore the persisted minimize state of both bars (called at
    /// startup). Minimized bars come back as their edge restore tabs.
    pub fn init_toolbar_minimized_from_config(&mut self, top: bool, side: bool) {
        self.toolbar_top_minimized = top;
        self.toolbar_side_minimized = side;
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
        match mode {
            TopDisplayMode::Full => {
                self.show_top_strip_surface();
            }
            TopDisplayMode::Micro => {
                self.toolbar_top_minimized = false;
                self.toolbar_shapes_expanded = false;
                self.toolbar_top_overflow_open = false;
                self.show_top_strip_surface();
            }
            TopDisplayMode::Hidden => {
                self.toolbar_shapes_expanded = false;
                self.toolbar_top_overflow_open = false;
            }
        }
        self.needs_redraw = true;
    }

    fn show_top_strip_surface(&mut self) {
        self.toolbar_top_visible = true;
        self.toolbar_visible = self.toolbar_top_visible || self.toolbar_side_visible;
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

    /// Restore the persisted side-palette pane and collapsed sections
    /// (called at startup). Unknown ids are ignored; they are preserved in
    /// the config file itself for forward compatibility.
    pub fn init_toolbar_side_panes_from_config(
        &mut self,
        active_pane_id: &str,
        collapsed_section_ids: &[String],
    ) {
        self.toolbar_side_pane =
            crate::ui::toolbar::SidePane::from_config_id(active_pane_id).unwrap_or_default();
        self.toolbar_collapsed_side_sections = collapsed_section_ids
            .iter()
            .filter_map(|id| crate::ui::toolbar::ToolbarSideSection::from_config_id(id))
            .collect();
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

    pub fn reset_toolbar_item_hidden_overrides(&mut self) -> bool {
        if !self.toolbar_items.reset_known_hidden_to_defaults() {
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
            self.set_ui_toast_with_action_and_duration(
                UiToastKind::Info,
                "Cleared",
                "Undo?",
                Action::Undo,
                CLEAR_UNDO_TOAST_MS,
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
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::{SidePane, ToolbarSideSection};

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
            true,  // settings section
            false,
            false,
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
    fn pill_side_layout_retires_the_side_surface_and_panel_restores_it() {
        let mut state = make_test_input_state();
        // The struct (and config) default keeps the classic panel behavior
        // until the Session/Settings panes are re-hosted in the top strip.
        assert_eq!(
            state.toolbar_side_layout,
            crate::config::ToolbarSideLayout::Panel
        );
        assert!(state.toolbar_side_visible());

        // Opting into Pill retires the side surface: it never reports
        // visible, even through the plain visibility toggles.
        state.init_toolbar_side_layout_from_config(crate::config::ToolbarSideLayout::Pill);
        assert!(!state.toolbar_side_visible());
        state.set_toolbar_visible(true);
        assert!(!state.toolbar_side_visible());
        // The top strip is unaffected by the side retirement.
        assert!(state.toolbar_top_visible());

        // The deprecated escape hatch restores the classic behavior.
        state.init_toolbar_side_layout_from_config(crate::config::ToolbarSideLayout::Panel);
        assert!(state.toolbar_side_visible());
    }

    #[test]
    fn side_pane_config_restore_ignores_unknown_ids() {
        let mut state = make_test_input_state();
        state.init_toolbar_side_panes_from_config(
            "session",
            &[
                "colors".to_string(),
                "unknown-id".to_string(),
                "step-undo".to_string(),
            ],
        );
        assert_eq!(state.toolbar_side_pane, SidePane::Session);
        assert!(
            state
                .toolbar_collapsed_side_sections
                .contains(&ToolbarSideSection::Colors)
        );
        assert!(
            state
                .toolbar_collapsed_side_sections
                .contains(&ToolbarSideSection::StepUndo)
        );
        assert_eq!(state.toolbar_collapsed_side_sections.len(), 2);

        state.init_toolbar_side_panes_from_config("bogus", &[]);
        assert_eq!(state.toolbar_side_pane, SidePane::Draw);
        assert!(state.toolbar_collapsed_side_sections.is_empty());
    }
}
