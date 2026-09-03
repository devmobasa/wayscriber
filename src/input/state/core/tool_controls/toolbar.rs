use super::super::base::InputState;
use crate::config::{
    RadialMenuMouseBinding, ToolbarItemId, ToolbarItemOrderGroup, ToolbarItemVisibilitySetting,
    TopDisplayMode,
};
use crate::domain::Action;
use crate::input::state::{Toast, ToastPriority, TopMenuState};

/// How long the "Cleared — Undo?" toast stays up after a mouse-path clear.
pub(crate) const CLEAR_UNDO_TOAST_MS: u64 = 2000;

impl InputState {
    /// Sets toolbar visibility without changing its persisted pin.
    pub fn set_toolbar_visible(&mut self, visible: bool) -> bool {
        if !self.toolbar.set_visible(visible) {
            return false;
        }
        self.refresh_status_hud_layout();
        self.needs_redraw = true;
        true
    }

    /// Re-derive live visibility from the persisted pin without surfacing a
    /// toolbar hidden by a transient chrome owner.
    pub(crate) fn derive_toolbar_visibility_from_pins(&mut self) {
        let visible = self.toolbar.top_pinned();
        if let Some(restore) = self.focus_mode_restore.as_mut() {
            restore.toolbar_visibility.set_visible(visible);
            return;
        }
        if let Some(restore) = self.presenter_restore.as_mut()
            && let Some(snapshot) = restore.toolbar_visibility.as_mut()
        {
            snapshot.set_visible(visible);
            return;
        }
        if let Some(restore) = self.light_mode_restore.as_mut() {
            restore.toolbar_visibility.set_visible(visible);
            return;
        }
        self.toolbar.derive_visibility_from_pins();
        self.refresh_status_hud_layout();
    }

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
        self.push_toast(
            ToastPriority::Info,
            "ui",
            Toast::info(message).action(action_label, recovery_action),
        );
    }

    fn right_click_chrome_recovery_available(&self) -> bool {
        self.context_menu_enabled()
            && !self.zoom_active()
            && self.radial_menu.mouse_binding != RadialMenuMouseBinding::Right
    }

    fn presenter_will_restore_visible_chrome(&self) -> bool {
        if !self.presenter_mode {
            return false;
        }
        let Some(restore) = self.presenter_restore.as_ref() else {
            return false;
        };
        let restores_status_bar = restore.show_status_bar == Some(true);
        let restores_top_toolbar = restore
            .toolbar_visibility
            .is_some_and(|snapshot| snapshot.effectively_visible());
        restores_status_bar || restores_top_toolbar
    }

    pub fn toolbar_visible(&self) -> bool {
        self.toolbar.effectively_visible()
    }

    pub fn toolbar_top_visible(&self) -> bool {
        self.toolbar.effectively_visible()
    }

    pub(crate) fn toolbar_top_pinned(&self) -> bool {
        self.toolbar.top_pinned()
    }

    pub(crate) fn toolbar_use_icons(&self) -> bool {
        self.toolbar.use_icons()
    }

    pub(crate) fn toolbar_scale(&self) -> f64 {
        self.toolbar.scale()
    }

    pub(crate) fn toolbar_layout_mode(&self) -> crate::config::ToolbarLayoutMode {
        self.toolbar.layout_mode()
    }

    #[cfg(test)]
    pub(crate) fn toolbar_mode_overrides(&self) -> &crate::config::ToolbarModeOverrides {
        self.toolbar.mode_overrides()
    }

    #[cfg(test)]
    pub(crate) fn toolbar_items(&self) -> &crate::config::ToolbarItemsConfig {
        self.toolbar.items()
    }

    pub(crate) fn resolved_toolbar_items(&self) -> &crate::config::ResolvedToolbarItems {
        self.toolbar.resolved_items()
    }

    pub(crate) fn toolbar_customize_items_open(&self) -> bool {
        self.toolbar.customize_items_open()
    }

    pub(crate) fn toolbar_customize_items_group(
        &self,
    ) -> Option<crate::ui::toolbar::ToolbarItemCustomizeGroup> {
        self.toolbar.customize_items_group()
    }

    pub(crate) fn toolbar_status_bar_contents_open(&self) -> bool {
        self.toolbar.status_bar_contents_open()
    }

    pub(crate) fn toolbar_top_popover_scroll(&self) -> f64 {
        self.toolbar.top_popover_scroll()
    }

    pub(crate) fn toolbar_top_minimized(&self) -> bool {
        self.toolbar.top_minimized()
    }

    pub(crate) fn toolbar_top_display_mode(&self) -> TopDisplayMode {
        self.toolbar.top_display_mode()
    }

    pub(crate) fn toolbar_top_menu(&self) -> TopMenuState {
        self.toolbar.top_menu()
    }

    pub(crate) fn toolbar_rebind_click_label(&self) -> Option<&'static str> {
        self.toolbar.rebind_modifier().click_label()
    }

    pub(crate) fn toolbar_visibility_snapshot(&self) -> super::super::toolbar::ToolbarVisibility {
        self.toolbar.visibility_snapshot()
    }

    pub(crate) fn restore_toolbar_visibility(
        &mut self,
        snapshot: super::super::toolbar::ToolbarVisibility,
    ) {
        self.toolbar.restore_visibility(snapshot);
    }

    pub(crate) fn hide_toolbar_visibility(&mut self) {
        self.toolbar.hide();
    }

    pub(crate) fn show_toolbar_visibility(&mut self) {
        self.toolbar.show();
    }

    pub(crate) fn set_toolbar_top_pinned(&mut self, pinned: bool) {
        self.toolbar.set_top_pinned(pinned);
    }

    pub(crate) fn set_toolbar_use_icons(&mut self, use_icons: bool) {
        self.toolbar.set_use_icons(use_icons);
    }

    pub fn init_toolbar_from_config(&mut self, config: &crate::config::ToolbarConfig) {
        let legacy = crate::config::ToolbarSectionVisibility {
            show_actions_section: self.ui_visibility.show_actions_section,
            show_actions_advanced: self.ui_visibility.show_actions_advanced,
            show_zoom_actions: self.ui_visibility.show_zoom_actions,
            show_pages_section: self.ui_visibility.show_pages_section,
            show_boards_section: self.ui_visibility.show_boards_section,
            show_presets: self.ui_visibility.show_presets,
            show_step_section: self.ui_visibility.show_step_section,
            show_text_controls: self.ui_visibility.show_text_controls,
        };
        self.toolbar = super::super::toolbar::ToolbarInteraction::from_config(config, &legacy);
        self.refresh_section_visibility();
    }

    pub(crate) fn refresh_section_visibility(&mut self) {
        let visibility = self.toolbar.section_visibility();
        self.ui_visibility.show_actions_section = visibility.show_actions_section;
        self.ui_visibility.show_actions_advanced = visibility.show_actions_advanced;
        self.ui_visibility.show_zoom_actions = visibility.show_zoom_actions;
        self.ui_visibility.show_pages_section = visibility.show_pages_section;
        self.ui_visibility.show_boards_section = visibility.show_boards_section;
        self.ui_visibility.show_presets = visibility.show_presets;
        self.ui_visibility.show_step_section = visibility.show_step_section;
        self.ui_visibility.show_text_controls = visibility.show_text_controls;
    }

    pub fn top_display_state(&self) -> TopDisplayMode {
        if !self.toolbar_top_visible() {
            TopDisplayMode::Hidden
        } else if self.toolbar.top_display_mode() == TopDisplayMode::Micro
            && !self.toolbar.top_minimized()
        {
            TopDisplayMode::Micro
        } else {
            TopDisplayMode::Full
        }
    }

    pub(crate) fn set_top_display_mode(&mut self, mode: TopDisplayMode) {
        self.toolbar.set_top_display_mode(mode);
        self.refresh_status_hud_layout();
        self.needs_redraw = true;
    }

    pub fn cycle_top_toolbar_display(&mut self) -> TopDisplayMode {
        let current = self.top_display_state();
        let next = self.toolbar.cycle_top_display_mode(current);
        self.refresh_status_hud_layout();
        self.needs_redraw = true;
        next
    }

    pub fn set_toolbar_item_hidden(&mut self, id: ToolbarItemId, hidden: bool) -> bool {
        if !self.toolbar.set_item_hidden(id, hidden) {
            return false;
        }
        self.refresh_section_visibility();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn set_toolbar_item_visibility_setting(
        &mut self,
        id: ToolbarItemId,
        setting: ToolbarItemVisibilitySetting,
    ) -> bool {
        if !self.toolbar.set_item_visibility_setting(id, setting) {
            return false;
        }
        self.refresh_section_visibility();
        self.needs_redraw = true;
        true
    }

    pub fn reset_toolbar_item_hidden_overrides(&mut self) -> bool {
        if !self.toolbar.reset_individual_item_visibility() {
            return false;
        }
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
        if !self.toolbar.move_item_by(group, id, delta) {
            return false;
        }
        self.needs_redraw = true;
        true
    }

    pub fn start_toolbar_item_drag(
        &mut self,
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
    ) -> bool {
        let drag = (group, id);
        if self.toolbar.customize_drag() == Some(&drag) {
            return false;
        }
        self.toolbar.begin_customize_drag(drag);
        true
    }

    pub fn drag_toolbar_item_over(
        &mut self,
        group: ToolbarItemOrderGroup,
        target_index: usize,
    ) -> bool {
        if !self.toolbar.move_dragged_item_to_index(group, target_index) {
            return false;
        }
        self.needs_redraw = true;
        true
    }

    pub fn clear_toolbar_item_drag(&mut self) {
        self.toolbar.clear_customize_drag();
    }

    pub(crate) fn set_toolbar_item_order(
        &mut self,
        group: ToolbarItemOrderGroup,
        order: &[ToolbarItemId],
    ) -> bool {
        if !self.toolbar.set_item_order(group, order) {
            return false;
        }
        self.needs_redraw = true;
        true
    }

    pub fn reset_toolbar_item_order(&mut self, group: ToolbarItemOrderGroup) -> bool {
        if !self.toolbar.reset_item_order(group) {
            return false;
        }
        self.needs_redraw = true;
        true
    }

    #[cfg(test)]
    pub(crate) fn toolbar_visible_flag(&self) -> bool {
        self.toolbar.visible()
    }

    #[cfg(test)]
    pub(crate) fn toolbar_top_visible_flag(&self) -> bool {
        self.toolbar.top_visible()
    }

    #[cfg(test)]
    pub(crate) fn test_set_toolbar_visibility_state(
        &mut self,
        visible: bool,
        top_visible: bool,
        top_pinned: bool,
    ) {
        self.toolbar
            .override_visibility_for_test(visible, top_visible, top_pinned);
    }

    #[cfg(test)]
    pub(crate) fn test_set_toolbar_appearance(&mut self, use_icons: bool, scale: f64) {
        self.toolbar.override_appearance_for_test(use_icons, scale);
    }

    #[cfg(test)]
    pub(crate) fn test_set_toolbar_display_state(&mut self, mode: TopDisplayMode, minimized: bool) {
        self.toolbar.override_display_for_test(mode, minimized);
    }

    #[cfg(test)]
    pub(crate) fn test_set_toolbar_menu_state(&mut self, menu: TopMenuState, scroll: f64) {
        self.toolbar.override_menu_for_test(menu, scroll);
    }

    #[cfg(test)]
    pub(crate) fn test_set_toolbar_items(&mut self, items: crate::config::ToolbarItemsConfig) {
        self.toolbar.override_items_for_test(items);
        self.refresh_section_visibility();
    }

    #[cfg(all(test, feature = "toolbar-gtk"))]
    pub(crate) fn test_set_toolbar_layout(
        &mut self,
        mode: crate::config::ToolbarLayoutMode,
        overrides: crate::config::ToolbarModeOverrides,
    ) {
        self.toolbar.override_layout_for_test(mode, overrides);
        self.refresh_section_visibility();
    }

    #[cfg(test)]
    pub(crate) fn test_set_toolbar_customization(
        &mut self,
        items_open: bool,
        group: Option<crate::ui::toolbar::ToolbarItemCustomizeGroup>,
        status_bar_contents_open: bool,
    ) {
        self.toolbar
            .override_customization_for_test(items_open, group, status_bar_contents_open);
    }

    #[cfg(test)]
    pub(crate) fn test_set_toolbar_rebind_modifier(
        &mut self,
        modifier: crate::config::ToolbarRebindModifier,
    ) {
        self.toolbar.override_rebind_modifier_for_test(modifier);
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
        let config = crate::config::ToolbarConfig {
            show_zoom_actions: false,
            ..Default::default()
        };
        state.ui_visibility = crate::input::state::UiVisibility::from(&crate::config::UiConfig {
            toolbar: config.clone(),
            ..Default::default()
        });
        state.init_toolbar_from_config(&config);

        // Effective visibility is bit-identical to the legacy booleans...
        assert!(!state.ui_visibility.show_zoom_actions);
        assert!(state.ui_visibility.show_presets);
        // ...and the disagreement is now an explicit override that
        // survives mode switches.
        let zoom_id = crate::config::ToolbarSectionFlag::ZoomActions.item_id();
        assert!(state.resolved_toolbar_items().hidden.contains(&zoom_id));
        state.apply_toolbar_event(crate::ui::toolbar::ToolbarEvent::SetToolbarLayoutMode(
            crate::config::ToolbarLayoutMode::Advanced,
        ));
        assert!(!state.ui_visibility.show_zoom_actions);
    }

    #[test]
    fn factory_visibility_reset_changes_only_the_centralized_eligible_set() {
        let mut state = make_test_input_state();
        let section = crate::config::ToolbarSectionFlag::Actions.item_id();
        let mut items = ToolbarItemsConfig::default();
        items.set_hidden(ids::TOP_UTILITY_SCREENSHOT, false);
        items.set_hidden(ids::TOP_UTILITY_OCR, false);
        items.set_hidden(ids::TOP_TOOL_PEN, true);
        items.set_hidden(section, true);
        items.set_hidden(ids::TOP_CHROME_OVERFLOW, true);
        items.hidden.push("future.toolbar.item".to_string());
        state.test_set_toolbar_items(items);

        assert!(state.reset_toolbar_item_hidden_overrides());

        let resolved = state.toolbar_items().resolved();
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
