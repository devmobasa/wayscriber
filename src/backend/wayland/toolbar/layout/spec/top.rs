use crate::config::ToolbarLayoutMode;
use crate::ui::toolbar::ToolbarSnapshot;
use crate::ui::toolbar::model;

use super::ToolbarLayoutSpec;

impl ToolbarLayoutSpec {
    pub(in crate::backend::wayland::toolbar) const TOP_SIZE_ICONS: (u32, u32) = (735, 72);
    pub(in crate::backend::wayland::toolbar) const TOP_SIZE_TEXT: (u32, u32) = (875, 60);

    pub(in crate::backend::wayland::toolbar) const TOP_GAP: f64 = 5.0;
    pub(in crate::backend::wayland::toolbar) const TOP_START_X: f64 = 19.0;
    pub(in crate::backend::wayland::toolbar) const TOP_HANDLE_SIZE: f64 = 18.0;
    pub(in crate::backend::wayland::toolbar) const TOP_HANDLE_Y: f64 = 27.0;
    pub(in crate::backend::wayland::toolbar) const TOP_ICON_BUTTON: f64 = 46.0;
    pub(in crate::backend::wayland::toolbar) const TOP_ICON_BUTTON_Y: f64 = 8.0;
    pub(in crate::backend::wayland::toolbar) const TOP_ICON_SIZE: f64 = 28.0;
    pub(in crate::backend::wayland::toolbar) const TOP_ICON_FILL_HEIGHT: f64 = 18.0;
    pub(in crate::backend::wayland::toolbar) const TOP_ICON_FILL_OFFSET: f64 = 2.0;
    pub(in crate::backend::wayland::toolbar) const TOP_TEXT_BUTTON_W: f64 = 60.0;
    pub(in crate::backend::wayland::toolbar) const TOP_TEXT_BUTTON_H: f64 = 36.0;
    pub(in crate::backend::wayland::toolbar) const TOP_TEXT_FILL_W: f64 = 64.0;
    pub(in crate::backend::wayland::toolbar) const TOP_TOGGLE_WIDTH: f64 = 84.0;
    pub(in crate::backend::wayland::toolbar) const TOP_PIN_BUTTON_SIZE: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const TOP_PIN_BUTTON_GAP: f64 = 6.0;
    pub(in crate::backend::wayland::toolbar) const TOP_PIN_BUTTON_MARGIN_RIGHT: f64 = 15.0;
    pub(in crate::backend::wayland::toolbar) const TOP_PIN_BUTTON_Y_ICON: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const TOP_SHAPE_ROW_GAP: f64 = 6.0;

    pub(in crate::backend::wayland::toolbar) fn top_size(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> (u32, u32) {
        let base_height = if self.use_icons {
            Self::TOP_SIZE_ICONS.1
        } else {
            Self::TOP_SIZE_TEXT.1
        };
        let mut height = base_height as f64;
        if self.shape_picker_open && model::top_shape_picker_visible(snapshot) {
            let (_, btn_h) = if self.use_icons {
                (Self::TOP_ICON_BUTTON, Self::TOP_ICON_BUTTON)
            } else {
                (Self::TOP_TEXT_BUTTON_W, Self::TOP_TEXT_BUTTON_H)
            };
            let row_count = if self.layout_mode == ToolbarLayoutMode::Simple {
                let mut rows = 0.0;
                if model::visible_tool_count(model::common_shape_tools(), snapshot) > 0 {
                    rows += 1.0;
                }
                if model::visible_tool_count(model::polygon_tools(), snapshot) > 0 {
                    rows += 1.0;
                }
                rows
            } else if model::visible_tool_count(model::polygon_tools(), snapshot) > 0 {
                1.0
            } else {
                0.0
            };
            height += row_count * (btn_h + Self::TOP_SHAPE_ROW_GAP);
        }

        let gap = Self::TOP_GAP;
        let btn_w = if self.use_icons {
            Self::TOP_ICON_BUTTON
        } else {
            Self::TOP_TEXT_BUTTON_W
        };
        let tool_count = model::visible_top_tool_buttons(
            self.layout_mode == ToolbarLayoutMode::Simple,
            snapshot,
        )
        .count();
        let mut x = Self::TOP_START_X;
        if model::toolbar_item_visible(snapshot, "top.chrome.drag") {
            x += Self::TOP_HANDLE_SIZE + gap;
        }
        x += tool_count as f64 * (btn_w + gap);
        if model::top_shape_picker_visible(snapshot) {
            x += btn_w + gap; // Shapes/Polygons picker
        }
        let fill_tool_active =
            model::fill_tool_active(snapshot.active_tool, snapshot.tool_override);
        let fill_visible = !self.use_icons
            && fill_tool_active
            && !self.shape_picker_open
            && model::top_fill_visible(snapshot);
        if fill_visible {
            x += Self::TOP_TEXT_FILL_W + gap;
        }
        if model::top_text_visible(snapshot) {
            x += btn_w + gap; // Text button
        }
        if model::top_sticky_note_visible(snapshot) {
            x += btn_w + gap; // Note button
        }
        if self.layout_mode != ToolbarLayoutMode::Simple {
            if model::top_clear_canvas_visible(snapshot) {
                x += btn_w + gap; // Clear
            }
            if self.use_icons && model::top_highlight_visible(snapshot) {
                x += btn_w + gap; // Highlight
            }
        }
        let left_end = if model::top_icon_mode_toggle_visible(snapshot) {
            x + Self::TOP_TOGGLE_WIDTH
        } else {
            x
        };
        let right_control_count =
            usize::from(model::toolbar_item_visible(snapshot, "top.chrome.pin"))
                + usize::from(model::toolbar_item_visible(snapshot, "top.chrome.close"));
        let right_controls = if right_control_count == 0 {
            0.0
        } else {
            Self::TOP_PIN_BUTTON_SIZE * right_control_count as f64
                + Self::TOP_PIN_BUTTON_GAP * right_control_count.saturating_sub(1) as f64
                + Self::TOP_PIN_BUTTON_MARGIN_RIGHT
        };
        let width = left_end + gap + right_controls;

        (width.ceil() as u32, height.ceil() as u32)
    }

    pub(in crate::backend::wayland::toolbar) fn top_button_size(&self) -> (f64, f64) {
        if self.use_icons {
            (Self::TOP_ICON_BUTTON, Self::TOP_ICON_BUTTON)
        } else {
            (Self::TOP_TEXT_BUTTON_W, Self::TOP_TEXT_BUTTON_H)
        }
    }

    pub(in crate::backend::wayland::toolbar) fn top_button_y(&self, height: f64) -> f64 {
        if self.use_icons {
            Self::TOP_ICON_BUTTON_Y
        } else {
            let (_, btn_h) = self.top_button_size();
            let base_height = Self::TOP_SIZE_TEXT.1 as f64;
            let available = if self.shape_picker_open {
                base_height
            } else {
                height
            };
            (available - btn_h) / 2.0
        }
    }

    pub(in crate::backend::wayland::toolbar) fn top_pin_button_y(&self, height: f64) -> f64 {
        if self.use_icons {
            Self::TOP_PIN_BUTTON_Y_ICON
        } else {
            (height - Self::TOP_PIN_BUTTON_SIZE) / 2.0
        }
    }

}
