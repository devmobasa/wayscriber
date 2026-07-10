use crate::backend::wayland::toolbar::rows::capped_grid_columns;
use crate::ui::toolbar::model::{
    ToolbarActionsModel, ToolbarCommandGroupKind, ToolbarSessionModel, ToolbarSettingsModel,
    toolbar_boards_model, toolbar_pages_model,
};
use crate::ui::toolbar::{ToolbarSideSection, ToolbarSnapshot};

use super::super::super::ToolbarLayoutSpec;

impl ToolbarLayoutSpec {
    pub(in crate::backend::wayland::toolbar) fn side_actions_content_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        let Some(model) = ToolbarActionsModel::from_snapshot(snapshot) else {
            return 0.0;
        };

        if self.use_icons {
            let icon_btn = Self::SIDE_ACTION_BUTTON_HEIGHT_ICON;
            let icon_gap = Self::SIDE_ACTION_BUTTON_GAP;
            let mut height = 0.0;
            let mut has_group = false;
            for group in model.groups() {
                if group.buttons.is_empty() {
                    continue;
                }
                if has_group {
                    height += icon_gap;
                }
                let columns = match group.kind {
                    ToolbarCommandGroupKind::BasicActions => group.buttons.len(),
                    ToolbarCommandGroupKind::ViewActions
                    | ToolbarCommandGroupKind::AdvancedActions => 5,
                    ToolbarCommandGroupKind::Pages | ToolbarCommandGroupKind::Boards => {
                        group.buttons.len()
                    }
                };
                let rows = group.buttons.len().div_ceil(columns);
                height += icon_btn * rows as f64 + icon_gap * (rows as f64 - 1.0);
                has_group = true;
            }
            height
        } else {
            let action_h = Self::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
            let action_gap = Self::SIDE_ACTION_CONTENT_GAP_TEXT;
            let mut height = 0.0;
            let mut has_group = false;
            for group in model.groups() {
                if group.buttons.is_empty() {
                    continue;
                }
                if has_group {
                    height += Self::SIDE_ACTION_BUTTON_GAP;
                }
                let columns = match group.kind {
                    ToolbarCommandGroupKind::BasicActions => 1,
                    ToolbarCommandGroupKind::ViewActions
                    | ToolbarCommandGroupKind::AdvancedActions => 2,
                    ToolbarCommandGroupKind::Pages | ToolbarCommandGroupKind::Boards => {
                        group.buttons.len().max(1)
                    }
                };
                let rows = group.buttons.len().div_ceil(columns);
                height += action_h * rows as f64 + action_gap * (rows as f64 - 1.0);
                has_group = true;
            }
            height
        }
    }

    pub(in crate::backend::wayland::toolbar) fn side_actions_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        if snapshot.side_section_collapsed(ToolbarSideSection::Actions) {
            return Self::SIDE_COLLAPSED_SECTION_HEIGHT;
        }
        let content = self.side_actions_content_height(snapshot);
        if content <= 0.0 {
            0.0
        } else {
            Self::SIDE_SECTION_TOGGLE_OFFSET_Y + content + Self::SIDE_ACTION_BUTTON_GAP
        }
    }

    pub(in crate::backend::wayland::toolbar) fn side_pages_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        let Some(pages) = toolbar_pages_model(snapshot) else {
            return 0.0;
        };
        if snapshot.side_section_collapsed(ToolbarSideSection::Pages) {
            return Self::SIDE_COLLAPSED_SECTION_HEIGHT;
        }
        let btn_h = if self.use_icons {
            Self::SIDE_ACTION_BUTTON_HEIGHT_ICON
        } else {
            Self::SIDE_ACTION_BUTTON_HEIGHT_TEXT
        };
        let columns = pages.buttons.len().max(1);
        let rows = pages.buttons.len().div_ceil(columns);
        Self::SIDE_SECTION_TOGGLE_OFFSET_Y
            + btn_h * rows as f64
            + Self::SIDE_ACTION_BUTTON_GAP * rows as f64
    }

    pub(in crate::backend::wayland::toolbar) fn side_boards_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        let Some(boards) = toolbar_boards_model(snapshot) else {
            return 0.0;
        };
        if snapshot.side_section_collapsed(ToolbarSideSection::Boards) {
            return Self::SIDE_COLLAPSED_SECTION_HEIGHT;
        }
        let btn_h = if self.use_icons {
            Self::SIDE_ACTION_BUTTON_HEIGHT_ICON
        } else {
            Self::SIDE_ACTION_BUTTON_HEIGHT_TEXT
        };
        let columns = capped_grid_columns(boards.buttons.len(), 5);
        let rows = boards.buttons.len().div_ceil(columns);
        Self::SIDE_SECTION_TOGGLE_OFFSET_Y
            + btn_h * rows as f64
            + Self::SIDE_ACTION_BUTTON_GAP * rows as f64
    }

    pub(in crate::backend::wayland::toolbar) fn side_step_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        if snapshot.side_section_hidden(ToolbarSideSection::StepUndo) {
            return 0.0;
        }
        if snapshot.side_section_collapsed(ToolbarSideSection::StepUndo) {
            return Self::SIDE_COLLAPSED_SECTION_HEIGHT;
        }
        let delay_h = if snapshot.show_delay_sliders {
            Self::SIDE_DELAY_SECTION_HEIGHT
        } else {
            0.0
        };
        let toggles_h = Self::SIDE_TOGGLE_HEIGHT * 2.0 + Self::SIDE_TOGGLE_GAP;
        Self::SIDE_STEP_HEADER_HEIGHT
            + toggles_h
            + if snapshot.custom_section_enabled {
                Self::SIDE_CUSTOM_SECTION_HEIGHT
            } else {
                0.0
            }
            + delay_h
    }

    pub(in crate::backend::wayland::toolbar) fn side_settings_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        let dedicated_panel = snapshot.customize_items_open;
        if !dedicated_panel && snapshot.side_section_collapsed(ToolbarSideSection::Settings) {
            return Self::SIDE_COLLAPSED_SECTION_HEIGHT;
        }
        let toggle_h = Self::SIDE_TOGGLE_HEIGHT;
        let toggle_gap = Self::SIDE_TOGGLE_GAP;
        let Some(settings) = ToolbarSettingsModel::from_snapshot(snapshot) else {
            return 0.0;
        };
        let toggle_count = settings.toggles().len();
        let rows = toggle_count.div_ceil(2);
        let toggle_rows_h = if rows > 0 {
            toggle_h * rows as f64 + toggle_gap * (rows as f64 - 1.0)
        } else {
            0.0
        };
        let button_rows = settings.buttons().len().div_ceil(2);
        let buttons_h = if button_rows > 0 {
            Self::SIDE_SETTINGS_BUTTON_HEIGHT * button_rows as f64
        } else {
            0.0
        };
        let group_rows = settings.groups().len().div_ceil(2);
        let group_rows_h = if group_rows > 0 {
            toggle_h
                + toggle_gap
                + Self::SIDE_SETTINGS_BUTTON_HEIGHT * group_rows as f64
                + Self::SIDE_SETTINGS_BUTTON_GAP * (group_rows as f64 - 1.0)
        } else {
            0.0
        };
        let item_rows = settings.item_overrides().len();
        let item_rows_h = if item_rows > 0 {
            toggle_h
                + toggle_gap
                + toggle_h * item_rows as f64
                + toggle_gap * (item_rows as f64 - 1.0)
        } else {
            0.0
        };
        let customize_h = group_rows_h + item_rows_h;
        let customize_gap = if customize_h > 0.0 { toggle_gap } else { 0.0 };
        // Interim Simple/Full layout-mode row at the top of the pane (only
        // outside the customization sub-panel).
        let mode_row_h = if dedicated_panel {
            0.0
        } else {
            Self::SIDE_SEGMENT_HEIGHT + toggle_gap
        };
        let content_h =
            mode_row_h + toggle_rows_h + toggle_gap + buttons_h + customize_gap + customize_h;
        Self::SIDE_SECTION_TOGGLE_OFFSET_Y + content_h + Self::SIDE_SETTINGS_BUTTON_GAP
    }

    pub(in crate::backend::wayland::toolbar) fn side_session_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        let Some(session) = ToolbarSessionModel::from_snapshot(snapshot) else {
            return 0.0;
        };
        if snapshot.side_section_collapsed(ToolbarSideSection::Session) {
            return Self::SIDE_COLLAPSED_SECTION_HEIGHT;
        }
        let button_rows = session.button_rows();
        let buttons_h = if button_rows == 0 {
            0.0
        } else {
            Self::SIDE_SESSION_BUTTON_HEIGHT * button_rows as f64
                + Self::SIDE_SESSION_ROW_GAP * (button_rows as f64 - 1.0)
        };
        let recents_h = if session.has_recent_sessions() {
            Self::SIDE_SESSION_ROW_GAP
                + session.recents.len() as f64
                    * (Self::SIDE_SESSION_RECENT_HEIGHT + Self::SIDE_SESSION_ROW_GAP)
        } else {
            0.0
        };
        Self::SIDE_SECTION_TOGGLE_OFFSET_Y
            + Self::SIDE_SESSION_META_HEIGHT
            + Self::SIDE_SESSION_ROW_GAP
            + buttons_h
            + recents_h
            + Self::SIDE_SESSION_ROW_GAP
    }
}
