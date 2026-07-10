use crate::config::{ToolbarItemId, ToolbarItemOrderGroup, ToolbarLayoutMode};
use crate::input::InputState;
use crate::ui::toolbar::{ToolbarItemCustomizeGroup, ToolbarSideSection};

impl InputState {
    pub(super) fn apply_toolbar_toggle_custom_section(&mut self, enable: bool) -> bool {
        if self.custom_section_enabled != enable {
            self.custom_section_enabled = enable;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_delay_sliders(&mut self, show: bool) -> bool {
        if self.show_delay_sliders != show {
            self.show_delay_sliders = show;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_open_configurator(&mut self) -> bool {
        self.launch_configurator();
        true
    }

    pub(super) fn apply_toolbar_open_config_file(&mut self) -> bool {
        self.open_config_file_default();
        true
    }

    pub(super) fn apply_toolbar_close_top_toolbar(&mut self) -> bool {
        self.toolbar_top_visible = false;
        self.toolbar_shapes_expanded = false;
        self.toolbar_visible = self.toolbar_top_visible || self.toolbar_side_visible;
        true
    }

    pub(super) fn apply_toolbar_close_side_toolbar(&mut self) -> bool {
        self.toolbar_side_visible = false;
        self.toolbar_visible = self.toolbar_top_visible || self.toolbar_side_visible;
        true
    }

    pub(super) fn apply_toolbar_pin_top_toolbar(&mut self, pin: bool) -> bool {
        if self.toolbar_top_pinned != pin {
            self.toolbar_top_pinned = pin;
            // Show toast explaining what pinning does
            use crate::input::state::UiToastKind;
            if pin {
                self.set_ui_toast(UiToastKind::Info, "Top toolbar will open at startup");
            } else {
                self.set_ui_toast(UiToastKind::Info, "Top toolbar will be hidden at startup");
            }
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_pin_side_toolbar(&mut self, pin: bool) -> bool {
        if self.toolbar_side_pinned != pin {
            self.toolbar_side_pinned = pin;
            // Show toast explaining what pinning does
            use crate::input::state::UiToastKind;
            if pin {
                self.set_ui_toast(UiToastKind::Info, "Side toolbar will open at startup");
            } else {
                self.set_ui_toast(UiToastKind::Info, "Side toolbar will be hidden at startup");
            }
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_icon_mode(&mut self, use_icons: bool) -> bool {
        if self.toolbar_use_icons != use_icons {
            self.toolbar_use_icons = use_icons;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_more_colors(&mut self, show: bool) -> bool {
        if self.show_more_colors != show {
            self.show_more_colors = show;
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

    /// Section toggles record an explicit override in the item store (the
    /// single source of truth) and re-derive the mirror booleans, so the
    /// choice survives layout-mode switches.
    fn apply_section_flag(&mut self, flag: crate::config::ToolbarSectionFlag, show: bool) -> bool {
        let before = self.toolbar_items.clone();
        self.toolbar_items.set_hidden(flag.item_id(), !show);
        if self.toolbar_items == before {
            return false;
        }
        self.resolved_toolbar_items = self.toolbar_items.resolved();
        self.refresh_section_visibility();
        self.needs_redraw = true;
        true
    }

    pub(super) fn apply_toolbar_toggle_context_aware_ui(&mut self, enabled: bool) -> bool {
        if self.context_aware_ui != enabled {
            self.context_aware_ui = enabled;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_preset_toasts(&mut self, show: bool) -> bool {
        if self.show_preset_toasts != show {
            self.show_preset_toasts = show;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_tool_preview(&mut self, show: bool) -> bool {
        if self.presenter_mode && self.presenter_mode_config.hide_tool_preview {
            return false;
        }
        if self.show_tool_preview != show {
            self.show_tool_preview = show;
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
        if self.show_status_bar != show {
            self.show_status_bar = show;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_status_board_badge(&mut self, show: bool) -> bool {
        if self.show_status_board_badge != show {
            self.show_status_board_badge = show;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_status_page_badge(&mut self, show: bool) -> bool {
        if self.show_status_page_badge != show {
            self.show_status_page_badge = show;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_toggle_floating_badge_always(&mut self, show: bool) -> bool {
        if self.show_floating_badge_always != show {
            self.show_floating_badge_always = show;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_set_side_pane(
        &mut self,
        pane: crate::ui::toolbar::SidePane,
    ) -> bool {
        let mut changed = false;
        if self.toolbar_side_pane != pane {
            self.toolbar_side_pane = pane;
            changed = true;
        }
        // Leaving the Settings pane closes the customization sub-panel.
        if pane != crate::ui::toolbar::SidePane::Settings
            && (self.toolbar_customize_items_open || self.toolbar_customize_items_group.is_some())
        {
            self.toolbar_customize_items_open = false;
            self.toolbar_customize_items_group = None;
            changed = true;
        }
        if changed {
            self.needs_redraw = true;
        }
        changed
    }

    pub(super) fn apply_toolbar_scroll_side_pane(&mut self, offset: f64) -> bool {
        let pane = self.toolbar_side_pane.index();
        let offset = offset.max(0.0);
        if (self.toolbar_side_scroll[pane] - offset).abs() < 0.5 {
            return false;
        }
        self.toolbar_side_scroll[pane] = offset;
        self.needs_redraw = true;
        true
    }

    pub(super) fn apply_toolbar_toggle_side_section_collapsed(
        &mut self,
        section: ToolbarSideSection,
        collapsed: bool,
    ) -> bool {
        let changed = if collapsed {
            self.toolbar_collapsed_side_sections.insert(section)
        } else {
            self.toolbar_collapsed_side_sections.remove(&section)
        };
        if !changed {
            return false;
        }
        self.needs_redraw = true;
        true
    }

    pub(super) fn apply_toolbar_set_layout_mode(&mut self, mode: ToolbarLayoutMode) -> bool {
        if self.toolbar_layout_mode != mode {
            self.toolbar_layout_mode = mode;
            self.apply_toolbar_mode_defaults(mode);
            self.toolbar_shapes_expanded = false;
            true
        } else {
            false
        }
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
        if self.toolbar_customize_items_open == open {
            return false;
        }
        self.toolbar_customize_items_open = open;
        if !open {
            self.toolbar_customize_items_group = None;
        }
        self.toolbar_side_pane = crate::ui::toolbar::SidePane::Settings;
        self.needs_redraw = true;
        true
    }

    pub(super) fn apply_toolbar_set_item_customization_group(
        &mut self,
        group: Option<ToolbarItemCustomizeGroup>,
    ) -> bool {
        if self.toolbar_customize_items_group == group && self.toolbar_customize_items_open {
            return false;
        }
        self.toolbar_customize_items_open = true;
        self.toolbar_customize_items_group = group;
        self.toolbar_side_pane = crate::ui::toolbar::SidePane::Settings;
        self.needs_redraw = true;
        true
    }

    pub(super) fn apply_toolbar_toggle_shape_picker(&mut self, open: bool) -> bool {
        if self.toolbar_shapes_expanded != open {
            self.toolbar_shapes_expanded = open;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{ToolbarLayoutMode, ToolbarSectionFlag};
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::ToolbarEvent;

    #[test]
    fn section_toggles_survive_layout_mode_switches() {
        let mut state = make_test_input_state();
        assert!(state.show_zoom_actions);

        state.apply_toolbar_event(ToolbarEvent::ToggleZoomActions(false));
        assert!(!state.show_zoom_actions);

        // The old behavior recomputed all section booleans from mode
        // defaults on every switch, erasing the choice.
        state.apply_toolbar_event(ToolbarEvent::SetToolbarLayoutMode(
            ToolbarLayoutMode::Simple,
        ));
        assert!(!state.show_zoom_actions);
        state.apply_toolbar_event(ToolbarEvent::SetToolbarLayoutMode(
            ToolbarLayoutMode::Regular,
        ));
        assert!(!state.show_zoom_actions);

        // Simple hides presets by baseline; an explicit show survives the
        // round trip through other modes.
        state.apply_toolbar_event(ToolbarEvent::SetToolbarLayoutMode(
            ToolbarLayoutMode::Simple,
        ));
        assert!(!state.show_presets);
        state.apply_toolbar_event(ToolbarEvent::TogglePresets(true));
        assert!(state.show_presets);
        state.apply_toolbar_event(ToolbarEvent::SetToolbarLayoutMode(
            ToolbarLayoutMode::Advanced,
        ));
        state.apply_toolbar_event(ToolbarEvent::SetToolbarLayoutMode(
            ToolbarLayoutMode::Simple,
        ));
        assert!(state.show_presets);
        assert!(
            state
                .resolved_toolbar_items
                .shown
                .contains(&ToolbarSectionFlag::Presets.item_id())
        );
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
        assert!(!state.show_presets);

        // Re-enable through the Settings toggle — the hidden entry is
        // replaced, not fought, so the section comes back.
        state.apply_toolbar_event(ToolbarEvent::TogglePresets(true));
        assert!(state.show_presets);
        assert!(
            !state
                .resolved_toolbar_items
                .hidden
                .contains(&ToolbarSectionFlag::Presets.item_id())
        );
    }
}
