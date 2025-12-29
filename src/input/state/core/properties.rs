use super::base::{InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS, UiToastKind};
use crate::draw::{
    BLACK, BLUE, Color, Frame, GREEN, ORANGE, PINK, RED, Shape, ShapeId, WHITE, YELLOW,
};
use crate::time_utils::format_unix_millis;
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

const SELECTION_THICKNESS_STEP: f64 = 1.0;
const SELECTION_FONT_SIZE_STEP: f64 = 2.0;
const SELECTION_ARROW_LENGTH_STEP: f64 = 2.0;
const SELECTION_ARROW_ANGLE_STEP: f64 = 2.0;
const MIN_FONT_SIZE: f64 = 8.0;
const MAX_FONT_SIZE: f64 = 72.0;
const MIN_ARROW_LENGTH: f64 = 5.0;
const MAX_ARROW_LENGTH: f64 = 50.0;
const MIN_ARROW_ANGLE: f64 = 15.0;
const MAX_ARROW_ANGLE: f64 = 60.0;

const SELECTION_COLORS: [(&str, Color); 8] = [
    ("Red", RED),
    ("Green", GREEN),
    ("Blue", BLUE),
    ("Yellow", YELLOW),
    ("Orange", ORANGE),
    ("Pink", PINK),
    ("White", WHITE),
    ("Black", BLACK),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPropertyKind {
    Color,
    Thickness,
    Fill,
    FontSize,
    ArrowHead,
    ArrowLength,
    ArrowAngle,
    TextBackground,
}

#[derive(Debug, Clone)]
pub struct SelectionPropertyEntry {
    pub label: String,
    pub value: String,
    pub kind: SelectionPropertyKind,
    pub disabled: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PropertiesPanelLayout {
    pub origin_x: f64,
    pub origin_y: f64,
    pub width: f64,
    pub height: f64,
    pub title_baseline_y: f64,
    pub info_start_y: f64,
    pub entry_start_y: f64,
    pub entry_row_height: f64,
    pub padding_x: f64,
    pub label_x: f64,
    pub value_x: f64,
}

#[derive(Debug, Clone)]
pub struct ShapePropertiesPanel {
    pub title: String,
    pub anchor: (f64, f64),
    pub lines: Vec<String>,
    pub entries: Vec<SelectionPropertyEntry>,
    pub hover_index: Option<usize>,
    pub keyboard_focus: Option<usize>,
    pub multiple_selection: bool,
}

#[derive(Default)]
struct SelectionApplyResult {
    changed: usize,
    locked: usize,
    applicable: usize,
}

#[derive(Debug)]
struct PropertySummary<T> {
    applicable: bool,
    editable: bool,
    mixed: bool,
    value: Option<T>,
}

impl InputState {
    pub fn properties_panel(&self) -> Option<&ShapePropertiesPanel> {
        self.shape_properties_panel.as_ref()
    }

    pub fn properties_panel_layout(&self) -> Option<&PropertiesPanelLayout> {
        self.properties_panel_layout.as_ref()
    }

    pub fn is_properties_panel_open(&self) -> bool {
        self.shape_properties_panel.is_some()
    }

    pub fn close_properties_panel(&mut self) {
        if self.shape_properties_panel.take().is_some() {
            self.clear_properties_panel_layout();
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    pub(super) fn set_properties_panel(&mut self, panel: ShapePropertiesPanel) {
        self.shape_properties_panel = Some(panel);
        self.properties_panel_layout = None;
        self.pending_properties_hover_recalc = true;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn show_properties_panel(&mut self) -> bool {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return false;
        }

        let frame = self.canvas_set.active_frame();
        let anchor_rect = self.selection_bounding_box(&ids);
        let anchor = anchor_rect
            .map(|rect| {
                (
                    (rect.x + rect.width + 12) as f64,
                    (rect.y - 12).max(12) as f64,
                )
            })
            .unwrap_or_else(|| {
                let (px, py) = self.last_pointer_position;
                ((px + 16) as f64, (py - 16) as f64)
            });

        let entries = self.build_selection_property_entries(&ids);

        if ids.len() > 1 {
            let total = ids.len();
            let locked = ids
                .iter()
                .filter(|id| frame.shape(**id).map(|shape| shape.locked).unwrap_or(false))
                .count();
            let mut lines = Vec::new();
            lines.push(format!("Shapes selected: {total}"));
            if locked > 0 {
                lines.push(format!("Locked: {locked}/{total}"));
            }
            if let Some(bounds) = anchor_rect {
                lines.push(format!(
                    "Bounds: {}×{} px",
                    bounds.width.max(0),
                    bounds.height.max(0)
                ));
            }
            self.set_properties_panel(ShapePropertiesPanel {
                title: "Selection Properties".to_string(),
                anchor,
                lines,
                entries,
                hover_index: None,
                keyboard_focus: None,
                multiple_selection: true,
            });
            return true;
        }

        let shape_id = ids[0];
        let index = match frame.find_index(shape_id) {
            Some(idx) => idx,
            None => return false,
        };
        let drawn = match frame.shape(shape_id) {
            Some(shape) => shape,
            None => return false,
        };

        let mut lines = Vec::new();
        lines.push(format!("Shape ID: {shape_id}"));
        lines.push(format!("Type: {}", drawn.shape.kind_name()));
        lines.push(format!("Layer: {} of {}", index + 1, frame.shapes.len()));
        lines.push(format!(
            "Locked: {}",
            if drawn.locked { "Yes" } else { "No" }
        ));
        if let Some(timestamp) = format_timestamp(drawn.created_at) {
            lines.push(format!("Created: {timestamp}"));
        }
        if let Some(bounds) = drawn.shape.bounding_box() {
            lines.push(format!("Bounds: {}×{} px", bounds.width, bounds.height));
        }

        self.set_properties_panel(ShapePropertiesPanel {
            title: "Shape Properties".to_string(),
            anchor,
            lines,
            entries,
            hover_index: None,
            keyboard_focus: None,
            multiple_selection: false,
        });
        true
    }

    pub(crate) fn refresh_properties_panel(&mut self) {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return;
        }
        let entries = self.build_selection_property_entries(&ids);

        let Some(panel) = self.shape_properties_panel.as_mut() else {
            return;
        };
        panel.entries = entries;

        let valid_focus = panel
            .keyboard_focus
            .filter(|idx| *idx < panel.entries.len())
            .filter(|idx| !panel.entries[*idx].disabled);
        panel.keyboard_focus = valid_focus;
        if panel.hover_index.is_some()
            && panel
                .hover_index
                .is_some_and(|idx| idx >= panel.entries.len())
        {
            panel.hover_index = None;
        }

        self.pending_properties_hover_recalc = true;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

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

        let mut origin_x = panel.anchor.0;
        let mut origin_y = panel.anchor.1;

        let screen_w = screen_width as f64;
        let screen_h = screen_height as f64;
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
        if panel.hover_index != new_hover {
            panel.hover_index = new_hover;
            if new_hover.is_some() {
                panel.keyboard_focus = None;
            }
            if trigger_redraw {
                self.needs_redraw = true;
            }
        }
    }

    pub fn update_properties_panel_hover_from_pointer(&mut self, x: i32, y: i32) {
        self.update_properties_panel_hover_from_pointer_internal(x, y, true);
    }

    pub fn set_properties_panel_focus(&mut self, focus: Option<usize>) {
        if let Some(panel) = self.shape_properties_panel.as_mut() {
            let changed = panel.keyboard_focus != focus;
            panel.keyboard_focus = focus;
            if focus.is_some() {
                panel.hover_index = None;
            }
            if changed {
                self.needs_redraw = true;
            }
        }
    }

    fn current_properties_focus_or_hover(&self) -> Option<usize> {
        self.shape_properties_panel
            .as_ref()
            .and_then(|panel| panel.hover_index.or(panel.keyboard_focus))
    }

    fn select_properties_edge_entry(&mut self, start_front: bool) -> bool {
        let target = {
            let Some(panel) = self.shape_properties_panel.as_ref() else {
                return false;
            };
            if panel.entries.is_empty() {
                return false;
            }
            let iter: Box<dyn Iterator<Item = usize>> = if start_front {
                Box::new(0..panel.entries.len())
            } else {
                Box::new((0..panel.entries.len()).rev())
            };
            let mut target = None;
            for index in iter {
                if !panel.entries[index].disabled {
                    target = Some(index);
                    break;
                }
            }
            target
        };

        if let Some(index) = target {
            self.set_properties_panel_focus(Some(index));
            true
        } else {
            false
        }
    }

    pub(crate) fn focus_next_properties_entry(&mut self) -> bool {
        self.advance_properties_focus(true)
    }

    pub(crate) fn focus_previous_properties_entry(&mut self) -> bool {
        self.advance_properties_focus(false)
    }

    fn advance_properties_focus(&mut self, forward: bool) -> bool {
        let Some(panel) = self.shape_properties_panel.as_ref() else {
            return false;
        };
        if panel.entries.is_empty() {
            return false;
        }

        let len = panel.entries.len();
        let mut index = self
            .current_properties_focus_or_hover()
            .unwrap_or_else(|| if forward { len - 1 } else { 0 });

        for _ in 0..len {
            index = if forward {
                (index + 1) % len
            } else {
                (index + len - 1) % len
            };
            if !panel.entries[index].disabled {
                self.set_properties_panel_focus(Some(index));
                return true;
            }
        }
        false
    }

    pub(crate) fn focus_first_properties_entry(&mut self) -> bool {
        self.select_properties_edge_entry(true)
    }

    pub(crate) fn focus_last_properties_entry(&mut self) -> bool {
        self.select_properties_edge_entry(false)
    }

    pub(crate) fn activate_properties_panel_entry(&mut self) -> bool {
        let index = match self.current_properties_focus_or_hover() {
            Some(idx) => idx,
            None => return false,
        };
        self.apply_properties_entry(index, 0)
    }

    pub(crate) fn adjust_properties_panel_entry(&mut self, direction: i32) -> bool {
        let index = match self.current_properties_focus_or_hover() {
            Some(idx) => idx,
            None => return false,
        };
        self.apply_properties_entry(index, direction)
    }

    fn apply_properties_entry(&mut self, index: usize, direction: i32) -> bool {
        let entry = {
            let Some(panel) = self.shape_properties_panel.as_ref() else {
                return false;
            };
            let Some(entry) = panel.entries.get(index) else {
                return false;
            };
            if entry.disabled {
                return false;
            }
            entry.clone()
        };

        let changed = match entry.kind {
            SelectionPropertyKind::Color => self.apply_selection_color(direction),
            SelectionPropertyKind::Thickness => {
                self.apply_selection_thickness(direction_or_default(direction))
            }
            SelectionPropertyKind::Fill => self.apply_selection_fill(direction),
            SelectionPropertyKind::FontSize => {
                self.apply_selection_font_size(direction_or_default(direction))
            }
            SelectionPropertyKind::ArrowHead => self.apply_selection_arrow_head(direction),
            SelectionPropertyKind::ArrowLength => {
                self.apply_selection_arrow_length(direction_or_default(direction))
            }
            SelectionPropertyKind::ArrowAngle => {
                self.apply_selection_arrow_angle(direction_or_default(direction))
            }
            SelectionPropertyKind::TextBackground => {
                self.apply_selection_text_background(direction)
            }
        };

        if changed {
            self.refresh_properties_panel();
        }

        changed
    }

    fn apply_selection_color(&mut self, direction: i32) -> bool {
        let base_color = self.selection_primary_color().unwrap_or(RED);
        let index = color_palette_index(base_color).unwrap_or(0);
        let offset = if direction == 0 { 1 } else { direction };
        let next = cycle_index(index, SELECTION_COLORS.len(), offset);
        let target = SELECTION_COLORS[next].1;

        let result = self.apply_selection_change(
            |shape| {
                matches!(
                    shape,
                    Shape::Freehand { .. }
                        | Shape::Line { .. }
                        | Shape::Rect { .. }
                        | Shape::Ellipse { .. }
                        | Shape::Arrow { .. }
                        | Shape::MarkerStroke { .. }
                        | Shape::Text { .. }
                        | Shape::StickyNote { .. }
                )
            },
            |shape| match shape {
                Shape::Freehand { color, .. }
                | Shape::Line { color, .. }
                | Shape::Rect { color, .. }
                | Shape::Ellipse { color, .. }
                | Shape::Arrow { color, .. }
                | Shape::Text { color, .. } => {
                    if *color != target {
                        *color = target;
                        true
                    } else {
                        false
                    }
                }
                Shape::MarkerStroke { color, .. } => {
                    let new_color = Color {
                        a: color.a,
                        ..target
                    };
                    if *color != new_color {
                        *color = new_color;
                        true
                    } else {
                        false
                    }
                }
                Shape::StickyNote { background, .. } => {
                    if *background != target {
                        *background = target;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "color")
    }

    fn apply_selection_thickness(&mut self, direction: i32) -> bool {
        let delta = SELECTION_THICKNESS_STEP * direction as f64;
        let result = self.apply_selection_change(
            |shape| {
                matches!(
                    shape,
                    Shape::Freehand { .. }
                        | Shape::Line { .. }
                        | Shape::Rect { .. }
                        | Shape::Ellipse { .. }
                        | Shape::Arrow { .. }
                        | Shape::MarkerStroke { .. }
                )
            },
            |shape| match shape {
                Shape::Freehand { thick, .. }
                | Shape::Line { thick, .. }
                | Shape::Rect { thick, .. }
                | Shape::Ellipse { thick, .. }
                | Shape::Arrow { thick, .. }
                | Shape::MarkerStroke { thick, .. } => {
                    let next = (*thick + delta).clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
                    if (next - *thick).abs() > f64::EPSILON {
                        *thick = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "thickness")
    }

    fn apply_selection_font_size(&mut self, direction: i32) -> bool {
        let delta = SELECTION_FONT_SIZE_STEP * direction as f64;
        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Text { .. } | Shape::StickyNote { .. }),
            |shape| match shape {
                Shape::Text { size, .. } | Shape::StickyNote { size, .. } => {
                    let next = (*size + delta).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
                    if (next - *size).abs() > f64::EPSILON {
                        *size = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "font size")
    }

    fn apply_selection_fill(&mut self, direction: i32) -> bool {
        let target = if direction == 0 {
            self.selection_bool_target(|shape| match shape {
                Shape::Rect { fill, .. } | Shape::Ellipse { fill, .. } => Some(*fill),
                _ => None,
            })
        } else {
            Some(direction > 0)
        };

        let Some(target) = target else {
            self.set_ui_toast(UiToastKind::Warning, "No fill-capable shapes selected.");
            return false;
        };

        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Rect { .. } | Shape::Ellipse { .. }),
            |shape| match shape {
                Shape::Rect { fill, .. } | Shape::Ellipse { fill, .. } => {
                    if *fill != target {
                        *fill = target;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "fill")
    }

    fn apply_selection_arrow_head(&mut self, direction: i32) -> bool {
        let target = if direction == 0 {
            self.selection_bool_target(|shape| match shape {
                Shape::Arrow { head_at_end, .. } => Some(*head_at_end),
                _ => None,
            })
        } else {
            Some(direction > 0)
        };

        let Some(target) = target else {
            self.set_ui_toast(UiToastKind::Warning, "No arrows selected.");
            return false;
        };

        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { head_at_end, .. } => {
                    if *head_at_end != target {
                        *head_at_end = target;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow head")
    }

    fn apply_selection_arrow_length(&mut self, direction: i32) -> bool {
        let delta = SELECTION_ARROW_LENGTH_STEP * direction as f64;
        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { arrow_length, .. } => {
                    let next = (*arrow_length + delta).clamp(MIN_ARROW_LENGTH, MAX_ARROW_LENGTH);
                    if (next - *arrow_length).abs() > f64::EPSILON {
                        *arrow_length = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow length")
    }

    fn apply_selection_arrow_angle(&mut self, direction: i32) -> bool {
        let delta = SELECTION_ARROW_ANGLE_STEP * direction as f64;
        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { arrow_angle, .. } => {
                    let next = (*arrow_angle + delta).clamp(MIN_ARROW_ANGLE, MAX_ARROW_ANGLE);
                    if (next - *arrow_angle).abs() > f64::EPSILON {
                        *arrow_angle = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow angle")
    }

    fn apply_selection_text_background(&mut self, direction: i32) -> bool {
        let target = if direction == 0 {
            self.selection_bool_target(|shape| match shape {
                Shape::Text {
                    background_enabled, ..
                } => Some(*background_enabled),
                _ => None,
            })
        } else {
            Some(direction > 0)
        };

        let Some(target) = target else {
            self.set_ui_toast(UiToastKind::Warning, "No text shapes selected.");
            return false;
        };

        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Text { .. }),
            |shape| match shape {
                Shape::Text {
                    background_enabled, ..
                } => {
                    if *background_enabled != target {
                        *background_enabled = target;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "text background")
    }

    fn selection_primary_color(&self) -> Option<Color> {
        let frame = self.canvas_set.active_frame();
        for id in self.selected_shape_ids() {
            let Some(drawn) = frame.shape(*id) else {
                continue;
            };
            if drawn.locked {
                continue;
            }
            if let Some(color) = shape_color(&drawn.shape) {
                return Some(color);
            }
        }
        None
    }

    fn selection_bool_target<F>(&self, mut extract: F) -> Option<bool>
    where
        F: FnMut(&Shape) -> Option<bool>,
    {
        let frame = self.canvas_set.active_frame();
        let mut applicable = 0;
        let mut editable_values = Vec::new();
        for id in self.selected_shape_ids() {
            if let Some(drawn) = frame.shape(*id)
                && let Some(value) = extract(&drawn.shape)
            {
                applicable += 1;
                if !drawn.locked {
                    editable_values.push(value);
                }
            }
        }
        if applicable == 0 {
            return None;
        }
        if editable_values.is_empty() {
            return Some(true);
        }
        let first = editable_values[0];
        let mixed = editable_values.iter().any(|v| *v != first);
        if mixed { Some(true) } else { Some(!first) }
    }

    fn build_selection_property_entries(&self, ids: &[ShapeId]) -> Vec<SelectionPropertyEntry> {
        let frame = self.canvas_set.active_frame();
        let mut entries = Vec::new();

        let color_summary = summarize_property(frame, ids, shape_color, color_eq);
        if color_summary.applicable {
            let value = if !color_summary.editable {
                "Locked".to_string()
            } else if color_summary.mixed {
                "Mixed".to_string()
            } else {
                color_summary
                    .value
                    .map(color_label)
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Color".to_string(),
                value,
                kind: SelectionPropertyKind::Color,
                disabled: !color_summary.editable,
            });
        }

        let thickness_summary = summarize_property(frame, ids, shape_thickness, approx_eq);
        if thickness_summary.applicable {
            let value = if !thickness_summary.editable {
                "Locked".to_string()
            } else if thickness_summary.mixed {
                "Mixed".to_string()
            } else {
                thickness_summary
                    .value
                    .map(|v| format!("{v:.1}px"))
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Thickness".to_string(),
                value,
                kind: SelectionPropertyKind::Thickness,
                disabled: !thickness_summary.editable,
            });
        }

        let fill_summary = summarize_property(frame, ids, shape_fill, |a, b| a == b);
        if fill_summary.applicable {
            let value = if !fill_summary.editable {
                "Locked".to_string()
            } else if fill_summary.mixed {
                "Mixed".to_string()
            } else {
                fill_summary
                    .value
                    .map(|v| if v { "On" } else { "Off" }.to_string())
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Fill".to_string(),
                value,
                kind: SelectionPropertyKind::Fill,
                disabled: !fill_summary.editable,
            });
        }

        let font_summary = summarize_property(frame, ids, shape_font_size, approx_eq);
        if font_summary.applicable {
            let value = if !font_summary.editable {
                "Locked".to_string()
            } else if font_summary.mixed {
                "Mixed".to_string()
            } else {
                font_summary
                    .value
                    .map(|v| format!("{v:.0}pt"))
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Font size".to_string(),
                value,
                kind: SelectionPropertyKind::FontSize,
                disabled: !font_summary.editable,
            });
        }

        let head_summary = summarize_property(frame, ids, shape_arrow_head, |a, b| a == b);
        if head_summary.applicable {
            let value = if !head_summary.editable {
                "Locked".to_string()
            } else if head_summary.mixed {
                "Mixed".to_string()
            } else {
                head_summary
                    .value
                    .map(|v| if v { "End" } else { "Start" }.to_string())
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Arrow head".to_string(),
                value,
                kind: SelectionPropertyKind::ArrowHead,
                disabled: !head_summary.editable,
            });
        }

        let length_summary = summarize_property(frame, ids, shape_arrow_length, approx_eq);
        if length_summary.applicable {
            let value = if !length_summary.editable {
                "Locked".to_string()
            } else if length_summary.mixed {
                "Mixed".to_string()
            } else {
                length_summary
                    .value
                    .map(|v| format!("{v:.0}px"))
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Arrow length".to_string(),
                value,
                kind: SelectionPropertyKind::ArrowLength,
                disabled: !length_summary.editable,
            });
        }

        let angle_summary = summarize_property(frame, ids, shape_arrow_angle, approx_eq);
        if angle_summary.applicable {
            let value = if !angle_summary.editable {
                "Locked".to_string()
            } else if angle_summary.mixed {
                "Mixed".to_string()
            } else {
                angle_summary
                    .value
                    .map(|v| format!("{v:.0} deg"))
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Arrow angle".to_string(),
                value,
                kind: SelectionPropertyKind::ArrowAngle,
                disabled: !angle_summary.editable,
            });
        }

        let text_bg_summary = summarize_property(frame, ids, shape_text_background, |a, b| a == b);
        if text_bg_summary.applicable {
            let value = if !text_bg_summary.editable {
                "Locked".to_string()
            } else if text_bg_summary.mixed {
                "Mixed".to_string()
            } else {
                text_bg_summary
                    .value
                    .map(|v| if v { "On" } else { "Off" }.to_string())
                    .unwrap_or_else(|| "Mixed".to_string())
            };
            entries.push(SelectionPropertyEntry {
                label: "Text background".to_string(),
                value,
                kind: SelectionPropertyKind::TextBackground,
                disabled: !text_bg_summary.editable,
            });
        }

        entries
    }

    fn apply_selection_change<A, F>(
        &mut self,
        mut applicable: A,
        mut apply: F,
    ) -> SelectionApplyResult
    where
        A: FnMut(&Shape) -> bool,
        F: FnMut(&mut Shape) -> bool,
    {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return SelectionApplyResult::default();
        }

        let mut result = SelectionApplyResult::default();
        let mut actions = Vec::new();
        let mut dirty_regions = Vec::new();

        {
            let frame = self.canvas_set.active_frame_mut();
            for id in ids {
                let Some(drawn) = frame.shape_mut(id) else {
                    continue;
                };
                if !applicable(&drawn.shape) {
                    continue;
                }
                result.applicable += 1;
                if drawn.locked {
                    result.locked += 1;
                    continue;
                }

                let before_bounds = drawn.shape.bounding_box();
                let before_snapshot = crate::draw::frame::ShapeSnapshot {
                    shape: drawn.shape.clone(),
                    locked: drawn.locked,
                };

                let changed = apply(&mut drawn.shape);
                if !changed {
                    continue;
                }

                let after_bounds = drawn.shape.bounding_box();
                let after_snapshot = crate::draw::frame::ShapeSnapshot {
                    shape: drawn.shape.clone(),
                    locked: drawn.locked,
                };

                actions.push(crate::draw::frame::UndoAction::Modify {
                    shape_id: drawn.id,
                    before: before_snapshot,
                    after: after_snapshot,
                });
                dirty_regions.push((drawn.id, before_bounds, after_bounds));
                result.changed += 1;
            }
        }

        if actions.is_empty() {
            return result;
        }

        let undo_action = if actions.len() == 1 {
            actions.into_iter().next().unwrap()
        } else {
            crate::draw::frame::UndoAction::Compound(actions)
        };

        self.canvas_set
            .active_frame_mut()
            .push_undo_action(undo_action, self.undo_stack_limit);

        for (shape_id, before, after) in dirty_regions {
            self.mark_selection_dirty_region(before);
            self.mark_selection_dirty_region(after);
            self.invalidate_hit_cache_for(shape_id);
        }
        self.needs_redraw = true;

        result
    }

    fn report_selection_apply_result(&mut self, result: SelectionApplyResult, label: &str) -> bool {
        if result.applicable == 0 {
            self.set_ui_toast(
                UiToastKind::Warning,
                format!("No {label} to edit in selection."),
            );
            return false;
        }

        if result.changed == 0 {
            if result.locked == result.applicable {
                self.set_ui_toast(
                    UiToastKind::Warning,
                    format!("All {label} shapes are locked."),
                );
            } else {
                self.set_ui_toast(UiToastKind::Info, "No changes applied.");
            }
            return false;
        }

        if result.locked > 0 {
            self.set_ui_toast(
                UiToastKind::Warning,
                format!("{} locked shape(s) unchanged.", result.locked),
            );
        }
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

fn direction_or_default(direction: i32) -> i32 {
    if direction < 0 { -1 } else { 1 }
}

fn cycle_index(index: usize, len: usize, offset: i32) -> usize {
    if len == 0 {
        return 0;
    }
    let len_i = len as i32;
    let mut next = index as i32 + offset;
    if next < 0 {
        next = (next % len_i + len_i) % len_i;
    } else {
        next %= len_i;
    }
    next as usize
}

fn color_palette_index(color: Color) -> Option<usize> {
    SELECTION_COLORS
        .iter()
        .position(|(_, candidate)| color_eq(candidate, &color))
}

fn color_label(color: Color) -> String {
    for (name, candidate) in SELECTION_COLORS {
        if color_eq(&candidate, &color) {
            return name.to_string();
        }
    }
    "Custom".to_string()
}

fn color_eq(a: &Color, b: &Color) -> bool {
    approx_eq(&a.r, &b.r) && approx_eq(&a.g, &b.g) && approx_eq(&a.b, &b.b)
}

fn approx_eq(a: &f64, b: &f64) -> bool {
    (*a - *b).abs() <= 0.01
}

fn summarize_property<T, F, Eq>(
    frame: &Frame,
    ids: &[ShapeId],
    mut extract: F,
    mut eq: Eq,
) -> PropertySummary<T>
where
    T: Clone,
    F: FnMut(&Shape) -> Option<T>,
    Eq: FnMut(&T, &T) -> bool,
{
    let mut values = Vec::new();
    let mut applicable = 0;
    let mut editable = 0;

    for id in ids {
        let Some(drawn) = frame.shape(*id) else {
            continue;
        };
        let Some(value) = extract(&drawn.shape) else {
            continue;
        };
        applicable += 1;
        if drawn.locked {
            continue;
        }
        editable += 1;
        values.push(value);
    }

    if applicable == 0 {
        return PropertySummary {
            applicable: false,
            editable: false,
            mixed: false,
            value: None,
        };
    }

    if editable == 0 || values.is_empty() {
        return PropertySummary {
            applicable: true,
            editable: false,
            mixed: false,
            value: None,
        };
    }

    let first = values[0].clone();
    let mixed = values.iter().any(|v| !eq(&first, v));
    let value = if mixed { None } else { Some(first) };

    PropertySummary {
        applicable: true,
        editable: true,
        mixed,
        value,
    }
}

fn shape_color(shape: &Shape) -> Option<Color> {
    match shape {
        Shape::Freehand { color, .. }
        | Shape::Line { color, .. }
        | Shape::Rect { color, .. }
        | Shape::Ellipse { color, .. }
        | Shape::Arrow { color, .. }
        | Shape::MarkerStroke { color, .. }
        | Shape::Text { color, .. } => Some(*color),
        Shape::StickyNote { background, .. } => Some(*background),
        Shape::EraserStroke { .. } => None,
    }
}

fn shape_thickness(shape: &Shape) -> Option<f64> {
    match shape {
        Shape::Freehand { thick, .. }
        | Shape::Line { thick, .. }
        | Shape::Rect { thick, .. }
        | Shape::Ellipse { thick, .. }
        | Shape::Arrow { thick, .. }
        | Shape::MarkerStroke { thick, .. } => Some(*thick),
        _ => None,
    }
}

fn shape_fill(shape: &Shape) -> Option<bool> {
    match shape {
        Shape::Rect { fill, .. } | Shape::Ellipse { fill, .. } => Some(*fill),
        _ => None,
    }
}

fn shape_font_size(shape: &Shape) -> Option<f64> {
    match shape {
        Shape::Text { size, .. } | Shape::StickyNote { size, .. } => Some(*size),
        _ => None,
    }
}

fn shape_arrow_head(shape: &Shape) -> Option<bool> {
    match shape {
        Shape::Arrow { head_at_end, .. } => Some(*head_at_end),
        _ => None,
    }
}

fn shape_arrow_length(shape: &Shape) -> Option<f64> {
    match shape {
        Shape::Arrow { arrow_length, .. } => Some(*arrow_length),
        _ => None,
    }
}

fn shape_arrow_angle(shape: &Shape) -> Option<f64> {
    match shape {
        Shape::Arrow { arrow_angle, .. } => Some(*arrow_angle),
        _ => None,
    }
}

fn shape_text_background(shape: &Shape) -> Option<bool> {
    match shape {
        Shape::Text {
            background_enabled, ..
        } => Some(*background_enabled),
        _ => None,
    }
}

fn format_timestamp(ms: u64) -> Option<String> {
    format_unix_millis(ms, "%Y-%m-%d %H:%M:%S")
}
