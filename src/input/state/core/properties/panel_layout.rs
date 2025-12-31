use super::super::base::InputState;
use super::types::PropertiesPanelLayout;
use crate::util::Rect;
use cairo::Context as CairoContext;

const PANEL_TITLE_FONT: f64 = 15.0;
const PANEL_BODY_FONT: f64 = 13.0;
const PANEL_LINE_HEIGHT: f64 = 18.0;
const PANEL_ROW_HEIGHT: f64 = 22.0;
const PANEL_PADDING_X: f64 = 16.0;
const PANEL_PADDING_Y: f64 = 12.0;
const PANEL_COLUMN_GAP: f64 = 16.0;
const PANEL_SECTION_GAP: f64 = 8.0;
const PANEL_MARGIN: f64 = 12.0;
const PANEL_INFO_OFFSET: f64 = 12.0;
const PANEL_ANCHOR_GAP: f64 = 12.0;
const PANEL_POINTER_OFFSET: f64 = 16.0;

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
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(PANEL_TITLE_FONT);
        if let Ok(extents) = ctx.text_extents(&panel.title) {
            max_line_width = max_line_width.max(extents.width());
        }

        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        ctx.set_font_size(PANEL_BODY_FONT);
        for line in &panel.lines {
            if let Ok(extents) = ctx.text_extents(line) {
                max_line_width = max_line_width.max(extents.width());
            }
        }
        for entry in &panel.entries {
            if let Ok(extents) = ctx.text_extents(&entry.label) {
                max_label_width = max_label_width.max(extents.width());
            }
            if let Ok(extents) = ctx.text_extents(&entry.value) {
                max_value_width = max_value_width.max(extents.width());
            }
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
            self.mark_properties_panel_region(layout);
        }
    }

    pub fn properties_panel_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.properties_panel_layout?;
        let panel = self.shape_properties_panel.as_ref()?;
        if panel.entries.is_empty() {
            return None;
        }

        let local_x = x as f64 - layout.origin_x;
        let local_y = y as f64 - layout.origin_y;
        if local_x < 0.0 || local_y < 0.0 || local_x > layout.width || local_y > layout.height {
            return None;
        }

        let row_y = y as f64 - layout.entry_start_y;
        if row_y < 0.0 {
            return None;
        }
        let index = (row_y / layout.entry_row_height).floor() as usize;
        if index >= panel.entries.len() {
            None
        } else {
            Some(index)
        }
    }

    fn update_properties_panel_hover_from_pointer_internal(
        &mut self,
        x: i32,
        y: i32,
        trigger_redraw: bool,
    ) {
        let new_hover = self.properties_panel_index_at(x, y);
        let Some(panel) = self.shape_properties_panel.as_mut() else {
            return;
        };

        let new_hover = new_hover.filter(|idx| *idx < panel.entries.len());
        let new_hover = new_hover.filter(|idx| !panel.entries[*idx].disabled);

        if panel.hover_index != new_hover {
            panel.hover_index = new_hover;
            if trigger_redraw {
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
        }
    }

    pub fn update_properties_panel_hover_from_pointer(&mut self, x: i32, y: i32) {
        self.update_properties_panel_hover_from_pointer_internal(x, y, true);
    }

    pub fn set_properties_panel_focus(&mut self, focus: Option<usize>) {
        if let Some(panel) = self.shape_properties_panel.as_mut() {
            panel.keyboard_focus = focus;
        }
    }

    pub(super) fn current_properties_focus_or_hover(&self) -> Option<usize> {
        self.shape_properties_panel
            .as_ref()
            .and_then(|panel| panel.keyboard_focus.or(panel.hover_index))
    }

    fn select_properties_edge_entry(&mut self, start_front: bool) -> bool {
        let Some(panel) = self.shape_properties_panel.as_ref() else {
            return false;
        };
        if panel.entries.is_empty() {
            return false;
        }

        let mut index = if start_front {
            0
        } else {
            panel.entries.len().saturating_sub(1)
        };
        loop {
            let Some(entry) = panel.entries.get(index) else {
                return false;
            };
            if !entry.disabled {
                break;
            }
            if start_front {
                index += 1;
                if index >= panel.entries.len() {
                    return false;
                }
            } else if index == 0 {
                return false;
            } else {
                index -= 1;
            }
        }

        self.set_properties_panel_focus(Some(index));
        true
    }

    pub(crate) fn focus_next_properties_entry(&mut self) -> bool {
        self.advance_properties_focus(true)
    }

    pub(crate) fn focus_previous_properties_entry(&mut self) -> bool {
        self.advance_properties_focus(false)
    }

    pub(crate) fn focus_first_properties_entry(&mut self) -> bool {
        self.select_properties_edge_entry(true)
    }

    pub(crate) fn focus_last_properties_entry(&mut self) -> bool {
        self.select_properties_edge_entry(false)
    }

    fn advance_properties_focus(&mut self, forward: bool) -> bool {
        let Some(panel) = self.shape_properties_panel.as_ref() else {
            return false;
        };
        if panel.entries.is_empty() {
            return false;
        }

        let index = match self.current_properties_focus_or_hover() {
            Some(index) => index,
            None => {
                return if forward {
                    self.select_properties_edge_entry(true)
                } else {
                    self.select_properties_edge_entry(false)
                };
            }
        };

        let mut next = if forward {
            index + 1
        } else {
            index.saturating_sub(1)
        };
        loop {
            let Some(entry) = panel.entries.get(next) else {
                return false;
            };
            if !entry.disabled {
                break;
            }
            if forward {
                next += 1;
            } else if next == 0 {
                return false;
            } else {
                next -= 1;
            }
        }

        self.set_properties_panel_focus(Some(next));
        true
    }

    fn mark_properties_panel_region(&mut self, layout: PropertiesPanelLayout) {
        let x = layout.origin_x.floor() as i32;
        let y = layout.origin_y.floor() as i32;
        let width = layout.width.ceil() as i32 + 2;
        let height = layout.height.ceil() as i32 + 2;
        let width = width.max(1);
        let height = height.max(1);

        if let Some(rect) = Rect::new(x, y, width, height) {
            self.dirty_tracker.mark_rect(rect);
        } else {
            self.dirty_tracker.mark_full();
        }
    }
}

pub(super) fn selection_panel_anchor(bounds: Option<Rect>, pointer: (i32, i32)) -> (f64, f64) {
    bounds
        .map(|rect| {
            (
                rect.x as f64 + rect.width as f64 + PANEL_ANCHOR_GAP,
                (rect.y as f64 - PANEL_ANCHOR_GAP).max(PANEL_MARGIN),
            )
        })
        .unwrap_or_else(|| {
            let (px, py) = pointer;
            (
                px as f64 + PANEL_POINTER_OFFSET,
                py as f64 - PANEL_POINTER_OFFSET,
            )
        })
}
