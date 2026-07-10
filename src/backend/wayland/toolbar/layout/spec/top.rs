use crate::config::ToolbarLayoutMode;
use crate::ui::toolbar::ToolbarSnapshot;
use crate::ui::toolbar::model;

use super::ToolbarLayoutSpec;

impl ToolbarLayoutSpec {
    pub(in crate::backend::wayland::toolbar) const TOP_SIZE_ICONS: (u32, u32) = (735, 72);
    /// Minimized top strip: the edge restore tab.
    pub(in crate::backend::wayland::toolbar) const TOP_MINIMIZED_SIZE: (u32, u32) = (64, 24);
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
    pub(in crate::backend::wayland::toolbar) const TOP_PIN_BUTTON_SIZE: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const TOP_PIN_BUTTON_GAP: f64 = 6.0;
    pub(in crate::backend::wayland::toolbar) const TOP_PIN_BUTTON_MARGIN_RIGHT: f64 = 15.0;
    pub(in crate::backend::wayland::toolbar) const TOP_PIN_BUTTON_Y_ICON: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const TOP_SHAPE_ROW_GAP: f64 = 6.0;

    pub(in crate::backend::wayland::toolbar) fn top_size(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> (u32, u32) {
        if snapshot.top_minimized {
            return Self::TOP_MINIMIZED_SIZE;
        }
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
            let row_count = model::visible_shape_picker_row_count(
                snapshot,
                self.layout_mode == ToolbarLayoutMode::Simple,
            ) as f64;
            height += row_count * (btn_h + Self::TOP_SHAPE_ROW_GAP);
        }

        height += crate::backend::wayland::toolbar::view::top::top_overflow_height(snapshot);

        // Width comes from the same tree walk the builder performs, so the
        // size math and the builder cannot drift apart.
        let width =
            crate::backend::wayland::toolbar::view::top::top_natural_width(snapshot, height);

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
