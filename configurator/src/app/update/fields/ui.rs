use super::*;

impl ConfiguratorApp {
    pub(in crate::app::update) fn handle_status_position_changed(
        &mut self,
        option: StatusPositionOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.ui_status_position = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_input_hud_mode_changed(
        &mut self,
        option: InputHudModeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.input_hud_mode = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_input_hud_position_changed(
        &mut self,
        option: InputHudPositionOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.input_hud_position = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_ui_theme_changed(
        &mut self,
        option: UiThemeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.ui_theme = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_ui_reduced_motion_changed(
        &mut self,
        option: ReducedMotionOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.ui_reduced_motion = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_toolbar_layout_mode_changed(
        &mut self,
        option: ToolbarLayoutModeOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.apply_toolbar_layout_mode(option);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_toolbar_side_layout_changed(
        &mut self,
        option: ToolbarSideLayoutOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.ui_toolbar_side_layout = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_toolbar_zoom_chip_display_changed(
        &mut self,
        option: ZoomChipDisplayOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.ui_toolbar_zoom_chip_display = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_toolbar_rebind_modifier_changed(
        &mut self,
        option: ToolbarRebindModifierOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.ui_toolbar_rebind_modifier = option;
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_toolbar_override_mode_changed(
        &mut self,
        option: ToolbarLayoutModeOption,
    ) -> Vec<Effect> {
        self.override_mode = option;
        Vec::new()
    }

    pub(in crate::app::update) fn handle_toolbar_override_changed(
        &mut self,
        field: ToolbarOverrideField,
        option: OverrideOption,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft
            .set_toolbar_override(self.override_mode, field, option);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_toolbar_item_visibility_changed(
        &mut self,
        id: ToolbarItemId,
        visible: bool,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.set_toolbar_item_visible(id, visible);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_toolbar_item_move_requested(
        &mut self,
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
        delta: isize,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.move_toolbar_item(group, id, delta);
        self.refresh_dirty_flag();
        Vec::new()
    }

    pub(in crate::app::update) fn handle_toolbar_item_order_reset(
        &mut self,
        group: ToolbarItemOrderGroup,
    ) -> Vec<Effect> {
        self.status = StatusMessage::idle();
        self.draft.reset_toolbar_item_order(group);
        self.refresh_dirty_flag();
        Vec::new()
    }
}
