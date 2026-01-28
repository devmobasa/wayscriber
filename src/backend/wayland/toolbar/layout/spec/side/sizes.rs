use crate::config::ToolbarLayoutMode;
use crate::ui::toolbar::ToolbarSnapshot;
use crate::ui::toolbar::snapshot::ToolContext;

use super::super::ToolbarLayoutSpec;

impl ToolbarLayoutSpec {
    pub(in crate::backend::wayland::toolbar) fn side_size(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> (u32, u32) {
        let base_height = self.side_content_start_y();
        let tool_context = ToolContext::from_snapshot(snapshot);

        let show_drawer_view =
            snapshot.drawer_open && snapshot.drawer_tab == crate::input::ToolbarDrawerTab::View;
        let show_advanced = snapshot.show_actions_advanced && show_drawer_view;
        let show_actions = snapshot.show_actions_section || show_advanced;
        let show_pages = snapshot.show_pages_section
            && snapshot.drawer_open
            && snapshot.drawer_tab == crate::input::ToolbarDrawerTab::View;
        let show_boards = snapshot.show_boards_section
            && snapshot.drawer_open
            && snapshot.drawer_tab == crate::input::ToolbarDrawerTab::View;
        let show_presets =
            snapshot.show_presets && snapshot.preset_slot_count.min(snapshot.presets.len()) > 0;
        let show_step_section = snapshot.show_step_section
            && snapshot.drawer_open
            && snapshot.drawer_tab == crate::input::ToolbarDrawerTab::App;
        let show_settings_section = snapshot.show_settings_section
            && snapshot.drawer_open
            && snapshot.drawer_tab == crate::input::ToolbarDrawerTab::App;

        let mut height: f64 = base_height;
        let add_section = |section_height: f64, height: &mut f64| {
            if section_height > 0.0 {
                *height += section_height + Self::SIDE_SECTION_GAP;
            }
        };

        // Color section: only when tool needs color
        if tool_context.needs_color {
            let colors_h = self.side_colors_height(snapshot);
            add_section(colors_h, &mut height);
        }

        if show_presets {
            add_section(Self::SIDE_PRESET_CARD_HEIGHT, &mut height);
        }

        // Thickness/size slider: only when tool needs it
        if tool_context.needs_thickness {
            add_section(Self::SIDE_SLIDER_CARD_HEIGHT, &mut height);
            if snapshot.thickness_targets_eraser {
                add_section(Self::SIDE_ERASER_MODE_CARD_HEIGHT, &mut height);
            }
        }

        // Arrow controls: only when arrow tool is active
        if tool_context.show_arrow_labels {
            let arrow_height = if snapshot.arrow_label_enabled {
                Self::SIDE_TOGGLE_CARD_HEIGHT_WITH_RESET
            } else {
                Self::SIDE_TOGGLE_CARD_HEIGHT
            };
            add_section(arrow_height, &mut height);
        }

        // Step marker controls: only when step marker is active
        if tool_context.show_step_counter {
            add_section(Self::SIDE_TOGGLE_CARD_HEIGHT_WITH_RESET, &mut height);
        }

        // Marker opacity: only when marker tool is active
        if tool_context.show_marker_opacity {
            add_section(Self::SIDE_SLIDER_CARD_HEIGHT, &mut height);
        }

        // Text controls: only when text/note mode is active
        if tool_context.show_font_controls {
            add_section(Self::SIDE_SLIDER_CARD_HEIGHT, &mut height); // Text size
            add_section(Self::SIDE_FONT_CARD_HEIGHT, &mut height);
        }

        if snapshot.drawer_open {
            let tabs_h = self.side_drawer_tabs_height(snapshot);
            add_section(tabs_h, &mut height);
        }

        if show_actions {
            let mut actions_snapshot = snapshot.clone();
            actions_snapshot.show_actions_advanced = show_advanced;
            let actions_card_h = self.side_actions_height(&actions_snapshot);
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

    pub(in crate::backend::wayland::toolbar) fn side_actions_content_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        let show_drawer_view =
            snapshot.drawer_open && snapshot.drawer_tab == crate::input::ToolbarDrawerTab::View;
        let show_actions_section = snapshot.show_actions_section;
        let show_actions_advanced = snapshot.show_actions_advanced && show_drawer_view;
        let show_view_actions = show_drawer_view
            && snapshot.show_zoom_actions
            && (show_actions_section || snapshot.show_actions_advanced);
        if !show_actions_section && !show_actions_advanced {
            return 0.0;
        }

        let basic_count: usize = if show_actions_section { 3 } else { 0 };
        let view_count: usize = if show_view_actions { 4 } else { 0 };
        let show_delay_actions = snapshot.delay_actions_enabled;
        let advanced_count: usize = if show_actions_advanced {
            if show_delay_actions { 5 } else { 3 }
        } else {
            0
        };

        if self.use_icons {
            let icon_btn = Self::SIDE_ACTION_BUTTON_HEIGHT_ICON;
            let icon_gap = Self::SIDE_ACTION_BUTTON_GAP;
            let mut height = 0.0;
            let mut has_group = false;
            if basic_count > 0 {
                height += icon_btn;
                has_group = true;
            }
            if view_count > 0 {
                if has_group {
                    height += icon_gap;
                }
                let rows = view_count.div_ceil(5);
                height += icon_btn * rows as f64 + icon_gap * (rows as f64 - 1.0);
                has_group = true;
            }
            if advanced_count > 0 {
                if has_group {
                    height += icon_gap;
                }
                let rows = advanced_count.div_ceil(5);
                height += icon_btn * rows as f64 + icon_gap * (rows as f64 - 1.0);
            }
            height
        } else {
            let action_h = Self::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
            let action_gap = Self::SIDE_ACTION_CONTENT_GAP_TEXT;
            let mut height = 0.0;
            let mut has_group = false;
            if basic_count > 0 {
                height += action_h * basic_count as f64 + action_gap * (basic_count as f64 - 1.0);
                has_group = true;
            }
            if view_count > 0 {
                if has_group {
                    height += Self::SIDE_ACTION_BUTTON_GAP;
                }
                let rows = view_count.div_ceil(2);
                height += action_h * rows as f64 + action_gap * (rows as f64 - 1.0);
                has_group = true;
            }
            if advanced_count > 0 {
                if has_group {
                    height += Self::SIDE_ACTION_BUTTON_GAP;
                }
                let rows = advanced_count.div_ceil(2);
                height += action_h * rows as f64 + action_gap * (rows as f64 - 1.0);
            }
            height
        }
    }

    pub(in crate::backend::wayland::toolbar) fn side_actions_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
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
        if !snapshot.show_pages_section {
            return 0.0;
        }
        let btn_h = if self.use_icons {
            Self::SIDE_ACTION_BUTTON_HEIGHT_ICON
        } else {
            Self::SIDE_ACTION_BUTTON_HEIGHT_TEXT
        };
        Self::SIDE_SECTION_TOGGLE_OFFSET_Y + btn_h + Self::SIDE_ACTION_BUTTON_GAP
    }

    pub(in crate::backend::wayland::toolbar) fn side_boards_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        if !snapshot.show_boards_section {
            return 0.0;
        }
        let btn_h = if self.use_icons {
            Self::SIDE_ACTION_BUTTON_HEIGHT_ICON
        } else {
            Self::SIDE_ACTION_BUTTON_HEIGHT_TEXT
        };
        Self::SIDE_SECTION_TOGGLE_OFFSET_Y + btn_h + Self::SIDE_ACTION_BUTTON_GAP
    }

    pub(in crate::backend::wayland::toolbar) fn side_step_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
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
        let toggle_h = Self::SIDE_TOGGLE_HEIGHT;
        let toggle_gap = Self::SIDE_TOGGLE_GAP;
        let mut toggle_count = 6; // Text controls + status bar + status badges + preset toasts
        if snapshot.layout_mode != ToolbarLayoutMode::Simple {
            toggle_count += 7; // presets, actions, zoom actions, advanced actions, pages, boards, step section
        }
        let rows = (toggle_count + 1) / 2;
        let toggle_rows_h = if rows > 0 {
            toggle_h * rows as f64 + toggle_gap * (rows as f64 - 1.0)
        } else {
            0.0
        };
        let buttons_h = Self::SIDE_SETTINGS_BUTTON_HEIGHT;
        let content_h = toggle_rows_h + toggle_gap + buttons_h;
        Self::SIDE_SECTION_TOGGLE_OFFSET_Y + content_h + Self::SIDE_SETTINGS_BUTTON_GAP
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
}
