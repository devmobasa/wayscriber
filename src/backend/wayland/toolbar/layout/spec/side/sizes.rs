use crate::backend::wayland::toolbar::rows::capped_grid_columns;
use crate::ui::toolbar::model::{
    ToolbarActionsModel, ToolbarCommandGroupKind, ToolbarSessionModel, ToolbarSettingsModel,
    toolbar_boards_model, toolbar_pages_model,
};
use crate::ui::toolbar::snapshot::ToolContext;
use crate::ui::toolbar::{ToolbarSideSection, ToolbarSnapshot};

use super::super::ToolbarLayoutSpec;

impl ToolbarLayoutSpec {
    pub(in crate::backend::wayland::toolbar) fn side_size(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> (u32, u32) {
        let base_height = self.side_content_start_y();
        let tool_context = ToolContext::from_snapshot(snapshot);

        let show_actions = ToolbarActionsModel::from_snapshot(snapshot).is_some();
        let show_pages = toolbar_pages_model(snapshot).is_some();
        let show_boards = toolbar_boards_model(snapshot).is_some();
        let show_presets = !snapshot.side_section_hidden(ToolbarSideSection::Presets)
            && snapshot.show_presets
            && snapshot.preset_slot_count.min(snapshot.presets.len()) > 0;
        let show_step_section = snapshot.show_step_section
            && snapshot.drawer_open
            && snapshot.drawer_tab == crate::input::ToolbarDrawerTab::App
            && !snapshot.side_section_hidden(ToolbarSideSection::StepUndo);
        let show_settings_section = snapshot.show_settings_section
            && snapshot.drawer_open
            && snapshot.drawer_tab == crate::input::ToolbarDrawerTab::App
            && !snapshot.side_section_hidden(ToolbarSideSection::Settings);
        let show_session_section = ToolbarSessionModel::from_snapshot(snapshot).is_some();

        let mut height: f64 = base_height;
        let add_section = |section_height: f64, height: &mut f64| {
            if section_height > 0.0 {
                *height += section_height + Self::SIDE_SECTION_GAP;
            }
        };

        if tool_context.needs_color && !snapshot.side_section_hidden(ToolbarSideSection::Colors) {
            let colors_h = self.side_colors_height(snapshot);
            add_section(colors_h, &mut height);
        }

        if show_presets {
            add_section(self.side_presets_height(snapshot), &mut height);
        }

        if tool_context.needs_thickness {
            if !snapshot.side_section_hidden(ToolbarSideSection::Thickness) {
                add_section(self.side_thickness_height(snapshot), &mut height);
            }
            if tool_context.show_eraser_mode
                && !snapshot.side_section_hidden(ToolbarSideSection::EraserMode)
            {
                add_section(self.side_eraser_mode_height(snapshot), &mut height);
            }
            if tool_context.show_polygon_sides_control
                && !snapshot.side_section_hidden(ToolbarSideSection::PolygonSides)
            {
                add_section(self.side_polygon_sides_height(snapshot), &mut height);
            }
        }

        if tool_context.show_arrow_labels
            && !snapshot.side_section_hidden(ToolbarSideSection::ArrowLabels)
        {
            add_section(self.side_arrow_labels_height(snapshot), &mut height);
        }

        if tool_context.show_step_counter
            && !snapshot.side_section_hidden(ToolbarSideSection::StepMarkers)
        {
            add_section(self.side_step_markers_height(snapshot), &mut height);
        }

        if tool_context.show_marker_opacity
            && !snapshot.side_section_hidden(ToolbarSideSection::MarkerOpacity)
        {
            add_section(self.side_marker_opacity_height(snapshot), &mut height);
        }

        if tool_context.show_font_controls {
            if !snapshot.side_section_hidden(ToolbarSideSection::TextSize) {
                add_section(self.side_text_size_height(snapshot), &mut height);
            }
            if !snapshot.side_section_hidden(ToolbarSideSection::Font) {
                add_section(self.side_font_height(snapshot), &mut height);
            }
        }

        if snapshot.drawer_open {
            let tabs_h = self.side_drawer_tabs_height(snapshot);
            add_section(tabs_h, &mut height);
        }

        if show_actions {
            let actions_card_h = self.side_actions_height(snapshot);
            add_section(actions_card_h, &mut height);
        }

        if show_boards {
            let boards_h = self.side_boards_height(snapshot);
            add_section(boards_h, &mut height);
        }

        if show_pages {
            let pages_h = self.side_pages_height(snapshot);
            add_section(pages_h, &mut height);
        }

        if show_step_section {
            let step_h = self.side_step_height(snapshot);
            add_section(step_h, &mut height);
        }

        if show_session_section {
            let session_h = self.side_session_height(snapshot);
            add_section(session_h, &mut height);
        }

        if show_settings_section {
            let settings_h = self.side_settings_height(snapshot);
            add_section(settings_h, &mut height);
        }

        height += Self::SIDE_FOOTER_PADDING;

        (Self::SIDE_WIDTH, height.ceil() as u32)
    }

    pub(in crate::backend::wayland::toolbar) fn side_content_width(&self, width: f64) -> f64 {
        width - Self::SIDE_CONTENT_PADDING_X
    }

    pub(in crate::backend::wayland::toolbar) fn side_color_picker_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        let extra = if snapshot.show_more_colors {
            Self::SIDE_COLOR_PICKER_EXTRA_HEIGHT
        } else {
            0.0
        };
        Self::SIDE_COLOR_PICKER_INPUT_HEIGHT + extra
    }

    pub(in crate::backend::wayland::toolbar) fn side_colors_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        if snapshot.side_section_hidden(ToolbarSideSection::Colors) {
            return 0.0;
        }
        if snapshot.side_section_collapsed(ToolbarSideSection::Colors) {
            return Self::SIDE_COLLAPSED_SECTION_HEIGHT;
        }
        let rows = 1.0 + if snapshot.show_more_colors { 1.0 } else { 0.0 };
        let preview_row_h = Self::SIDE_COLOR_PREVIEW_SIZE
            + Self::SIDE_COLOR_PREVIEW_GAP_TOP
            + Self::SIDE_COLOR_PREVIEW_GAP_BOTTOM;
        Self::SIDE_COLOR_SECTION_LABEL_HEIGHT
            + Self::SIDE_COLOR_PICKER_INPUT_HEIGHT
            + preview_row_h
            + Self::SIDE_COLOR_SECTION_BOTTOM_PADDING
            + (Self::SIDE_COLOR_SWATCH + Self::SIDE_COLOR_SWATCH_GAP) * rows
    }

    pub(in crate::backend::wayland::toolbar) fn side_presets_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        self.collapsible_section_height(
            snapshot,
            ToolbarSideSection::Presets,
            Self::SIDE_PRESET_CARD_HEIGHT,
        )
    }

    pub(in crate::backend::wayland::toolbar) fn side_thickness_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        self.collapsible_section_height(
            snapshot,
            ToolbarSideSection::Thickness,
            Self::SIDE_SLIDER_CARD_HEIGHT,
        )
    }

    pub(in crate::backend::wayland::toolbar) fn side_eraser_mode_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        self.collapsible_section_height(
            snapshot,
            ToolbarSideSection::EraserMode,
            Self::SIDE_ERASER_MODE_CARD_HEIGHT,
        )
    }

    pub(in crate::backend::wayland::toolbar) fn side_polygon_sides_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        self.collapsible_section_height(
            snapshot,
            ToolbarSideSection::PolygonSides,
            Self::SIDE_SLIDER_CARD_HEIGHT,
        )
    }

    pub(in crate::backend::wayland::toolbar) fn side_arrow_labels_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        let expanded = if snapshot.arrow_label_enabled {
            Self::SIDE_TOGGLE_CARD_HEIGHT_WITH_RESET
        } else {
            Self::SIDE_TOGGLE_CARD_HEIGHT
        };
        self.collapsible_section_height(snapshot, ToolbarSideSection::ArrowLabels, expanded)
    }

    pub(in crate::backend::wayland::toolbar) fn side_step_markers_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        self.collapsible_section_height(
            snapshot,
            ToolbarSideSection::StepMarkers,
            Self::SIDE_TOGGLE_CARD_HEIGHT_WITH_RESET,
        )
    }

    pub(in crate::backend::wayland::toolbar) fn side_marker_opacity_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        self.collapsible_section_height(
            snapshot,
            ToolbarSideSection::MarkerOpacity,
            Self::SIDE_SLIDER_CARD_HEIGHT,
        )
    }

    pub(in crate::backend::wayland::toolbar) fn side_text_size_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        self.collapsible_section_height(
            snapshot,
            ToolbarSideSection::TextSize,
            Self::SIDE_SLIDER_CARD_HEIGHT,
        )
    }

    pub(in crate::backend::wayland::toolbar) fn side_font_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        self.collapsible_section_height(
            snapshot,
            ToolbarSideSection::Font,
            Self::SIDE_FONT_CARD_HEIGHT,
        )
    }

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

    pub(in crate::backend::wayland::toolbar) fn side_drawer_tabs_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        if !snapshot.drawer_open {
            return 0.0;
        }
        Self::SIDE_SECTION_TOGGLE_OFFSET_Y + Self::SIDE_TOGGLE_HEIGHT + Self::SIDE_ACTION_BUTTON_GAP
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
        if snapshot.side_section_collapsed(ToolbarSideSection::Settings) {
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
        let buttons_h = Self::SIDE_SETTINGS_BUTTON_HEIGHT;
        let content_h = toggle_rows_h + toggle_gap + buttons_h;
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

    /// Y position where Row 2 (mode controls row) starts
    pub(in crate::backend::wayland::toolbar) fn side_header_row2_y(&self) -> f64 {
        Self::SIDE_TOP_PADDING + Self::SIDE_HEADER_ROW1_HEIGHT
    }

    /// Y position where Row 3 (board row) starts
    pub(in crate::backend::wayland::toolbar) fn side_header_row3_y(&self) -> f64 {
        Self::SIDE_TOP_PADDING + Self::SIDE_HEADER_ROW1_HEIGHT + Self::SIDE_HEADER_ROW2_HEIGHT
    }

    /// Y position where content starts (after all header rows)
    pub(in crate::backend::wayland::toolbar) fn side_content_start_y(&self) -> f64 {
        // After header rows + bottom gap
        // = 12 + 30 + 28 + 24 + 8 = 102px
        Self::SIDE_TOP_PADDING
            + Self::SIDE_HEADER_ROW1_HEIGHT
            + Self::SIDE_HEADER_ROW2_HEIGHT
            + Self::SIDE_HEADER_ROW3_HEIGHT
            + Self::SIDE_HEADER_BOTTOM_GAP
    }

    pub(in crate::backend::wayland::toolbar) fn side_card_x(&self) -> f64 {
        Self::SIDE_START_X - Self::SIDE_CARD_INSET
    }

    pub(in crate::backend::wayland::toolbar) fn side_card_width(&self, width: f64) -> f64 {
        width - 2.0 * Self::SIDE_START_X + Self::SIDE_CARD_INSET * 2.0
    }

    fn collapsible_section_height(
        &self,
        snapshot: &ToolbarSnapshot,
        section: ToolbarSideSection,
        expanded_height: f64,
    ) -> f64 {
        if snapshot.side_section_hidden(section) {
            return 0.0;
        }
        if snapshot.side_section_collapsed(section) {
            Self::SIDE_COLLAPSED_SECTION_HEIGHT
        } else {
            expanded_height
        }
    }
}
