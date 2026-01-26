use crate::config::ToolbarLayoutMode;
use crate::input::Tool;
use crate::ui::toolbar::ToolbarSnapshot;

use super::ToolbarLayoutSpec;

impl ToolbarLayoutSpec {
    pub(in crate::backend::wayland::toolbar) const TOP_SIZE_ICONS: (u32, u32) = (735, 84);
    pub(in crate::backend::wayland::toolbar) const TOP_SIZE_TEXT: (u32, u32) = (875, 60);

    pub(in crate::backend::wayland::toolbar) const TOP_GAP: f64 = 5.0;
    pub(in crate::backend::wayland::toolbar) const TOP_START_X: f64 = 19.0;
    pub(in crate::backend::wayland::toolbar) const TOP_HANDLE_SIZE: f64 = 18.0;
    pub(in crate::backend::wayland::toolbar) const TOP_HANDLE_Y: f64 = 12.0;
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
    pub(in crate::backend::wayland::toolbar) const TOP_PIN_BUTTON_Y_ICON: f64 = 17.0;
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
        if self.layout_mode == ToolbarLayoutMode::Simple && self.shape_picker_open {
            let (_, btn_h) = if self.use_icons {
                (Self::TOP_ICON_BUTTON, Self::TOP_ICON_BUTTON)
            } else {
                (Self::TOP_TEXT_BUTTON_W, Self::TOP_TEXT_BUTTON_H)
            };
            height += btn_h + Self::TOP_SHAPE_ROW_GAP;
        }

        let gap = Self::TOP_GAP;
        let btn_w = if self.use_icons {
            Self::TOP_ICON_BUTTON
        } else {
            Self::TOP_TEXT_BUTTON_W
        };
        let tool_count = if self.layout_mode == ToolbarLayoutMode::Simple {
            5
        } else {
            9
        };
        let mut x = Self::TOP_START_X + Self::TOP_HANDLE_SIZE + gap;
        x += tool_count as f64 * (btn_w + gap);
        if self.layout_mode == ToolbarLayoutMode::Simple {
            x += btn_w + gap;
        }
        let fill_tool_active = matches!(snapshot.tool_override, Some(Tool::Rect | Tool::Ellipse))
            || matches!(snapshot.active_tool, Tool::Rect | Tool::Ellipse);
        let fill_visible = !self.use_icons
            && fill_tool_active
            && !(self.layout_mode == ToolbarLayoutMode::Simple && self.shape_picker_open);
        if fill_visible {
            x += Self::TOP_TEXT_FILL_W + gap;
        }
        x += btn_w + gap; // Text button
        x += btn_w + gap; // Note button
        if self.layout_mode != ToolbarLayoutMode::Simple {
            x += btn_w + gap; // Clear
            if self.use_icons {
                x += btn_w + gap; // Highlight
            }
        }
        let left_end = x + Self::TOP_TOGGLE_WIDTH;
        let right_controls = Self::TOP_PIN_BUTTON_SIZE * 2.0
            + Self::TOP_PIN_BUTTON_GAP
            + Self::TOP_PIN_BUTTON_MARGIN_RIGHT;
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
            (height - btn_h) / 2.0
        }
    }

    pub(in crate::backend::wayland::toolbar) fn top_pin_button_y(&self, height: f64) -> f64 {
        if self.use_icons {
            Self::TOP_PIN_BUTTON_Y_ICON
        } else {
            (height - Self::TOP_PIN_BUTTON_SIZE) / 2.0
        }
    }

    pub(in crate::backend::wayland::toolbar) fn top_pin_x(&self, width: f64) -> f64 {
        width
            - Self::TOP_PIN_BUTTON_SIZE * 2.0
            - Self::TOP_PIN_BUTTON_GAP
            - Self::TOP_PIN_BUTTON_MARGIN_RIGHT
    }

    pub(in crate::backend::wayland::toolbar) fn top_close_x(&self, width: f64) -> f64 {
        width - Self::TOP_PIN_BUTTON_SIZE - Self::TOP_PIN_BUTTON_MARGIN_RIGHT
    }
}
