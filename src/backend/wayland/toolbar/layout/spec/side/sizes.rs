use crate::ui::toolbar::model::{
    ToolbarActionsModel, ToolbarSessionModel, ToolbarSettingsModel, toolbar_boards_model,
    toolbar_pages_model,
};
use crate::ui::toolbar::snapshot::ToolContext;
use crate::ui::toolbar::{ToolbarSideSection, ToolbarSnapshot};

use super::super::ToolbarLayoutSpec;

mod sections;

impl ToolbarLayoutSpec {
    pub(in crate::backend::wayland::toolbar) fn side_size(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> (u32, u32) {
        let natural = self.side_natural_height(snapshot);
        let height = match snapshot.side_viewport_max {
            Some(max) if max > 0.0 && natural > max => max,
            _ => natural,
        };
        (Self::SIDE_WIDTH, height.ceil() as u32)
    }

    /// Unclamped content height of the active pane; when it exceeds the
    /// viewport the pane scrolls instead of growing the surface.
    pub(in crate::backend::wayland::toolbar) fn side_natural_height(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        let base_height = self.side_content_start_y();
        let tool_context = ToolContext::from_snapshot(snapshot);

        let show_actions = ToolbarActionsModel::from_snapshot(snapshot).is_some();
        let show_pages = toolbar_pages_model(snapshot).is_some();
        let show_boards = toolbar_boards_model(snapshot).is_some();
        let show_presets = !snapshot.side_section_hidden(ToolbarSideSection::Presets)
            && snapshot.show_presets
            && snapshot.preset_slot_count.min(snapshot.presets.len()) > 0;
        let show_step_section = snapshot.show_step_section
            && !snapshot.side_section_hidden(ToolbarSideSection::StepUndo);
        let show_settings_section = ToolbarSettingsModel::from_snapshot(snapshot).is_some();
        let show_session_section = ToolbarSessionModel::from_snapshot(snapshot).is_some();

        let mut height: f64 = base_height;
        let add_section = |section_height: f64, height: &mut f64| {
            if section_height > 0.0 {
                *height += section_height + Self::SIDE_SECTION_GAP;
            }
        };

        let mut thickness_block_added = false;
        let mut text_block_added = false;
        for section in crate::ui::toolbar::model::ordered_pane_sections(snapshot) {
            match section {
                ToolbarSideSection::Colors
                    if tool_context.needs_color
                        && !snapshot.side_section_hidden(ToolbarSideSection::Colors) =>
                {
                    add_section(self.side_colors_height(snapshot), &mut height);
                }
                ToolbarSideSection::Presets if show_presets => {
                    add_section(self.side_presets_height(snapshot), &mut height);
                }
                ToolbarSideSection::Thickness
                | ToolbarSideSection::EraserMode
                | ToolbarSideSection::PolygonSides
                    if tool_context.needs_thickness && !thickness_block_added =>
                {
                    thickness_block_added = true;
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
                ToolbarSideSection::ArrowLabels
                    if tool_context.show_arrow_labels
                        && !snapshot.side_section_hidden(ToolbarSideSection::ArrowLabels) =>
                {
                    add_section(self.side_arrow_labels_height(snapshot), &mut height);
                }
                ToolbarSideSection::StepMarkers
                    if tool_context.show_step_counter
                        && !snapshot.side_section_hidden(ToolbarSideSection::StepMarkers) =>
                {
                    add_section(self.side_step_markers_height(snapshot), &mut height);
                }
                ToolbarSideSection::MarkerOpacity
                    if tool_context.show_marker_opacity
                        && !snapshot.side_section_hidden(ToolbarSideSection::MarkerOpacity) =>
                {
                    add_section(self.side_marker_opacity_height(snapshot), &mut height);
                }
                ToolbarSideSection::TextSize | ToolbarSideSection::Font
                    if tool_context.show_font_controls && !text_block_added =>
                {
                    text_block_added = true;
                    if !snapshot.side_section_hidden(ToolbarSideSection::TextSize) {
                        add_section(self.side_text_size_height(snapshot), &mut height);
                    }
                    if !snapshot.side_section_hidden(ToolbarSideSection::Font) {
                        add_section(self.side_font_height(snapshot), &mut height);
                    }
                }
                ToolbarSideSection::Actions if show_actions => {
                    add_section(self.side_actions_height(snapshot), &mut height);
                }
                ToolbarSideSection::Boards if show_boards => {
                    add_section(self.side_boards_height(snapshot), &mut height);
                }
                ToolbarSideSection::Pages if show_pages => {
                    add_section(self.side_pages_height(snapshot), &mut height);
                }
                ToolbarSideSection::StepUndo if show_step_section => {
                    add_section(self.side_step_height(snapshot), &mut height);
                }
                ToolbarSideSection::Session if show_session_section => {
                    add_section(self.side_session_height(snapshot), &mut height);
                }
                ToolbarSideSection::Settings if show_settings_section => {
                    add_section(self.side_settings_height(snapshot), &mut height);
                }
                _ => {}
            }
        }

        height + Self::SIDE_FOOTER_PADDING
    }

    pub(in crate::backend::wayland::toolbar) fn side_content_width(&self, width: f64) -> f64 {
        width - Self::SIDE_CONTENT_PADDING_X
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
        let visible_rows = if snapshot.show_more_colors {
            let color_count = snapshot.quick_colors.rendered_len().max(1);
            color_count.div_ceil(Self::SIDE_COLOR_SWATCHES_PER_ROW)
        } else {
            1
        };
        let rows = visible_rows as f64;
        let preview_row_h = Self::SIDE_COLOR_PREVIEW_SIZE
            + Self::SIDE_COLOR_PREVIEW_GAP_TOP
            + Self::SIDE_COLOR_PREVIEW_GAP_BOTTOM;
        Self::SIDE_COLOR_SECTION_LABEL_HEIGHT
            + Self::SIDE_COLOR_PICKER_BLOCK_HEIGHT
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

    /// Y position of the pane navigation row.
    pub(in crate::backend::wayland::toolbar) fn side_pane_nav_y(&self) -> f64 {
        Self::SIDE_TOP_PADDING + Self::SIDE_HEADER_ROW1_HEIGHT + Self::SIDE_PANE_NAV_GAP
    }

    /// Y position where pane content starts (after header + nav rows).
    /// = 12 + 30 + 6 + 26 + 8 = 82px
    pub(in crate::backend::wayland::toolbar) fn side_content_start_y(&self) -> f64 {
        Self::SIDE_TOP_PADDING
            + Self::SIDE_HEADER_ROW1_HEIGHT
            + Self::SIDE_PANE_NAV_GAP
            + Self::SIDE_PANE_NAV_HEIGHT
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
