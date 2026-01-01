use crate::config::ToolbarLayoutMode;
use crate::ui::toolbar::ToolbarSnapshot;

use super::super::ToolbarLayoutSpec;

impl ToolbarLayoutSpec {
    pub(in crate::backend::wayland::toolbar) fn side_size(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> (u32, u32) {
        let base_height = self.side_content_start_y();
        let colors_h = self.side_colors_height(snapshot);
        let show_marker_opacity =
            snapshot.show_marker_opacity_section || snapshot.thickness_targets_marker;
        let show_text_controls =
            snapshot.text_active || snapshot.note_active || snapshot.show_text_controls;
        let show_actions = snapshot.show_actions_section || snapshot.show_actions_advanced;
        let show_pages = snapshot.show_actions_advanced && snapshot.show_pages_section;
        let show_presets =
            snapshot.show_presets && snapshot.preset_slot_count.min(snapshot.presets.len()) > 0;
        let show_step_section = snapshot.show_step_section;
        let show_settings_section = snapshot.show_settings_section;

        let mut height: f64 = base_height;
        let add_section = |section_height: f64, height: &mut f64| {
            if section_height > 0.0 {
                *height += section_height + Self::SIDE_SECTION_GAP;
            }
        };

        add_section(colors_h, &mut height);
        if show_presets {
            add_section(Self::SIDE_PRESET_CARD_HEIGHT, &mut height);
        }
        add_section(Self::SIDE_SLIDER_CARD_HEIGHT, &mut height); // Thickness
        if snapshot.thickness_targets_eraser {
            add_section(Self::SIDE_ERASER_MODE_CARD_HEIGHT, &mut height);
        }
        if show_marker_opacity {
            add_section(Self::SIDE_SLIDER_CARD_HEIGHT, &mut height);
        }
        if show_text_controls {
            add_section(Self::SIDE_SLIDER_CARD_HEIGHT, &mut height); // Text size
            add_section(Self::SIDE_FONT_CARD_HEIGHT, &mut height);
        }

        if show_actions {
            let actions_card_h = self.side_actions_height(snapshot);
            add_section(actions_card_h, &mut height);
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

    pub(in crate::backend::wayland::toolbar) fn side_header_button_positions(
        &self,
        width: f64,
    ) -> (f64, f64, f64) {
        let close_x = width - Self::SIDE_HEADER_BUTTON_MARGIN_RIGHT - Self::SIDE_HEADER_BUTTON_SIZE;
        let pin_x = close_x - Self::SIDE_HEADER_BUTTON_SIZE - Self::SIDE_HEADER_BUTTON_GAP;
        (pin_x, close_x, self.side_header_y())
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
        Self::SIDE_COLOR_SECTION_LABEL_HEIGHT
            + Self::SIDE_COLOR_PICKER_INPUT_HEIGHT
            + Self::SIDE_COLOR_SECTION_BOTTOM_PADDING
            + (Self::SIDE_COLOR_SWATCH + Self::SIDE_COLOR_SWATCH_GAP) * rows
    }

    pub(in crate::backend::wayland::toolbar) fn side_actions_content_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        let show_actions_section = snapshot.show_actions_section;
        let show_actions_advanced = snapshot.show_actions_advanced;
        if !show_actions_section && !show_actions_advanced {
            return 0.0;
        }

        let basic_count: usize = if show_actions_section { 3 } else { 0 };
        let show_delay_actions = snapshot.show_step_section && snapshot.show_delay_sliders;
        let advanced_count: usize = if show_actions_advanced {
            if show_delay_actions { 9 } else { 7 }
        } else {
            0
        };

        if self.use_icons {
            let icon_btn = Self::SIDE_ACTION_BUTTON_HEIGHT_ICON;
            let icon_gap = Self::SIDE_ACTION_BUTTON_GAP;
            let total_icons = basic_count + advanced_count;
            let icons_per_row = 6usize;
            let rows = if total_icons > 0 {
                total_icons.div_ceil(icons_per_row)
            } else {
                0
            };
            if rows > 0 {
                icon_btn * rows as f64 + icon_gap * (rows as f64 - 1.0)
            } else {
                0.0
            }
        } else {
            let action_h = Self::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
            let action_gap = Self::SIDE_ACTION_CONTENT_GAP_TEXT;
            let basic_h = if basic_count > 0 {
                action_h * basic_count as f64 + action_gap * (basic_count as f64 - 1.0)
            } else {
                0.0
            };
            let advanced_rows = if advanced_count > 0 {
                advanced_count.div_ceil(2)
            } else {
                0
            };
            let advanced_h = if advanced_rows > 0 {
                action_h * advanced_rows as f64 + action_gap * (advanced_rows as f64 - 1.0)
            } else {
                0.0
            };
            let gap_between = if show_actions_section && show_actions_advanced {
                Self::SIDE_ACTION_BUTTON_GAP
            } else {
                0.0
            };
            basic_h + gap_between + advanced_h
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

    pub(in crate::backend::wayland::toolbar) fn side_pages_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        if !snapshot.show_actions_advanced || !snapshot.show_pages_section {
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
        let mut toggle_count = 2; // Tool preview + preset toasts
        if snapshot.layout_mode == ToolbarLayoutMode::Advanced {
            toggle_count += 6; // presets, actions, advanced actions, pages, step section, text controls
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

    pub(in crate::backend::wayland::toolbar) fn side_header_y(&self) -> f64 {
        Self::SIDE_TOP_PADDING + Self::SIDE_HEADER_HANDLE_SIZE + Self::SIDE_HEADER_HANDLE_GAP
    }

    pub(in crate::backend::wayland::toolbar) fn side_content_start_y(&self) -> f64 {
        self.side_header_y() + Self::SIDE_HEADER_ROW_HEIGHT + Self::SIDE_HEADER_BOTTOM_GAP
    }

    pub(in crate::backend::wayland::toolbar) fn side_card_x(&self) -> f64 {
        Self::SIDE_START_X - Self::SIDE_CARD_INSET
    }

    pub(in crate::backend::wayland::toolbar) fn side_card_width(&self, width: f64) -> f64 {
        width - 2.0 * Self::SIDE_START_X + Self::SIDE_CARD_INSET * 2.0
    }
}
