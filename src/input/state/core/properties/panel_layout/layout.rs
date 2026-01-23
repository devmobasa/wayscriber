use cairo::Context as CairoContext;

use super::super::super::base::InputState;
use super::super::types::PropertiesPanelLayout;
use super::{
    PANEL_ANCHOR_GAP, PANEL_BODY_FONT, PANEL_COLUMN_GAP, PANEL_INFO_OFFSET, PANEL_LINE_HEIGHT,
    PANEL_MARGIN, PANEL_PADDING_X, PANEL_PADDING_Y, PANEL_ROW_HEIGHT, PANEL_SECTION_GAP,
    PANEL_TITLE_FONT,
};
use crate::ui_text::{UiTextStyle, text_layout};
use crate::util::Rect;

impl InputState {
    pub fn clear_properties_panel_layout(&mut self) {
        self.properties_panel_layout = None;
        self.pending_properties_hover_recalc = false;
    }

    pub fn update_properties_panel_layout(
        &mut self,
        ctx: &CairoContext,
        screen_width: u32,
        screen_height: u32,
    ) {
        if self.properties_panel_needs_refresh {
            self.refresh_properties_panel();
        }
        let Some(panel) = self.shape_properties_panel.as_ref() else {
            self.properties_panel_layout = None;
            return;
        };

        let mut max_line_width: f64 = 0.0;
        let mut max_label_width: f64 = 0.0;
        let mut max_value_width: f64 = 0.0;

        let _ = ctx.save();
        let title_style = UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Bold,
            size: PANEL_TITLE_FONT,
        };
        let body_style = UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Normal,
            size: PANEL_BODY_FONT,
        };
        let extents = text_layout(ctx, title_style, &panel.title, None).ink_extents();
        max_line_width = max_line_width.max(extents.width());

        for line in &panel.lines {
            let extents = text_layout(ctx, body_style, line, None).ink_extents();
            max_line_width = max_line_width.max(extents.width());
        }
        for entry in &panel.entries {
            let extents = text_layout(ctx, body_style, &entry.label, None).ink_extents();
            max_label_width = max_label_width.max(extents.width());
            let extents = text_layout(ctx, body_style, &entry.value, None).ink_extents();
            max_value_width = max_value_width.max(extents.width());
        }
        let _ = ctx.restore();

        let entries_width = if panel.entries.is_empty() {
            0.0
        } else {
            max_label_width + PANEL_COLUMN_GAP + max_value_width
        };
        let panel_width = (max_line_width.max(entries_width) + PANEL_PADDING_X * 2.0).ceil();

        let title_height = PANEL_TITLE_FONT + 4.0;
        let info_height = if panel.lines.is_empty() {
            0.0
        } else {
            PANEL_INFO_OFFSET + PANEL_LINE_HEIGHT * panel.lines.len() as f64
        };
        let entries_height = if panel.entries.is_empty() {
            0.0
        } else {
            PANEL_SECTION_GAP + PANEL_ROW_HEIGHT * panel.entries.len() as f64
        };
        let panel_height =
            (PANEL_PADDING_Y * 2.0 + title_height + info_height + entries_height).ceil();

        let screen_w = screen_width as f64;
        let screen_h = screen_height as f64;

        let (mut origin_x, mut origin_y) = if screen_w > 0.0 && screen_h > 0.0 {
            if let Some(bounds) = panel.anchor_rect {
                let rect_x = bounds.x as f64;
                let rect_y = bounds.y as f64;
                let rect_w = bounds.width.max(1) as f64;
                let rect_h = bounds.height.max(1) as f64;
                let center_x = rect_x + rect_w / 2.0;
                let center_y = rect_y + rect_h / 2.0;

                let candidates = [
                    (
                        rect_x + rect_w + PANEL_ANCHOR_GAP,
                        center_y - panel_height / 2.0,
                    ),
                    (
                        rect_x - panel_width - PANEL_ANCHOR_GAP,
                        center_y - panel_height / 2.0,
                    ),
                    (
                        center_x - panel_width / 2.0,
                        rect_y + rect_h + PANEL_ANCHOR_GAP,
                    ),
                    (
                        center_x - panel_width / 2.0,
                        rect_y - panel_height - PANEL_ANCHOR_GAP,
                    ),
                ];

                let max_x = screen_w - PANEL_MARGIN;
                let max_y = screen_h - PANEL_MARGIN;
                let overflow = |x: f64, y: f64| -> f64 {
                    let mut overflow = 0.0;
                    if x < PANEL_MARGIN {
                        overflow += PANEL_MARGIN - x;
                    }
                    if y < PANEL_MARGIN {
                        overflow += PANEL_MARGIN - y;
                    }
                    if x + panel_width > max_x {
                        overflow += x + panel_width - max_x;
                    }
                    if y + panel_height > max_y {
                        overflow += y + panel_height - max_y;
                    }
                    overflow
                };

                let mut best = candidates[0];
                let mut best_overflow = overflow(best.0, best.1);
                for (x, y) in candidates.into_iter().skip(1) {
                    let candidate_overflow = overflow(x, y);
                    if candidate_overflow < best_overflow {
                        best = (x, y);
                        best_overflow = candidate_overflow;
                    }
                }
                best
            } else {
                panel.anchor
            }
        } else {
            panel.anchor
        };
        if origin_x + panel_width > screen_w - PANEL_MARGIN {
            origin_x = (screen_w - panel_width - PANEL_MARGIN).max(PANEL_MARGIN);
        }
        if origin_y + panel_height > screen_h - PANEL_MARGIN {
            origin_y = (screen_h - panel_height - PANEL_MARGIN).max(PANEL_MARGIN);
        }
        if origin_x < PANEL_MARGIN {
            origin_x = PANEL_MARGIN;
        }
        if origin_y < PANEL_MARGIN {
            origin_y = PANEL_MARGIN;
        }

        let title_baseline_y = origin_y + PANEL_PADDING_Y + PANEL_TITLE_FONT;
        let info_start_y = title_baseline_y + PANEL_INFO_OFFSET;
        let mut entry_start_y = origin_y + PANEL_PADDING_Y + title_height + info_height;
        if !panel.entries.is_empty() {
            entry_start_y += PANEL_SECTION_GAP;
        }

        let label_x = origin_x + PANEL_PADDING_X;
        let value_x = origin_x + panel_width - PANEL_PADDING_X - max_value_width;

        self.properties_panel_layout = Some(PropertiesPanelLayout {
            origin_x,
            origin_y,
            width: panel_width,
            height: panel_height,
            title_baseline_y,
            info_start_y,
            entry_start_y,
            entry_row_height: PANEL_ROW_HEIGHT,
            padding_x: PANEL_PADDING_X,
            label_x,
            value_x,
        });

        if self.pending_properties_hover_recalc {
            let focus_set = panel.keyboard_focus.is_some();
            if !focus_set {
                let (px, py) = self.last_pointer_position;
                self.update_properties_panel_hover_from_pointer_internal(px, py, false);
            }
            self.pending_properties_hover_recalc = false;
        }

        if let Some(layout) = self.properties_panel_layout {
            mark_properties_panel_region(self, layout);
        }
    }
}

fn mark_properties_panel_region(state: &mut InputState, layout: PropertiesPanelLayout) {
    let x = layout.origin_x.floor() as i32;
    let y = layout.origin_y.floor() as i32;
    let width = layout.width.ceil() as i32 + 2;
    let height = layout.height.ceil() as i32 + 2;
    let width = width.max(1);
    let height = height.max(1);

    if let Some(rect) = Rect::new(x, y, width, height) {
        state.dirty_tracker.mark_rect(rect);
    } else {
        state.dirty_tracker.mark_full();
    }
}
