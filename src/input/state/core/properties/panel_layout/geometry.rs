//! Where each part of the properties panel sits, derived from the cached
//! layout. Hit-testing and painting both read these rectangles, so a control
//! is clickable exactly where it is drawn.

use super::super::metrics::{
    ACTION_BUTTON_GAP, ACTION_BUTTON_HEIGHT, ACTION_ROW_GAP, ACTIONS_LABEL_WIDTH, ACTIONS_TOP_GAP,
    BLOCK_BOTTOM, BLOCK_GAP, BLOCK_LABEL_LINE, BLOCK_TOP, BODY_FONT, COLUMN_SPACING,
    PRESET_SAVE_WIDTH, ROW_HEIGHT, ROW_INSET, SEGMENT_HEIGHT, SEGMENT_PAD, SLIDER_HIT_HEIGHT,
    SLIDER_THUMB_RADIUS, SLIDER_VALUE_GAP, STEP_BUTTON_WIDTH, STEPPER_HEIGHT, STYLE_BUTTON_GAP,
    STYLE_BUTTON_HEIGHT, SWATCH_GAP, SWATCH_ITEMS_PER_LINE, SWATCH_LINE_GAP, SWATCH_SIZE,
    SWITCH_HEIGHT, SWITCH_WIDTH, TITLE_FONT,
};
use super::super::types::{
    PanelAction, PanelRect, PropertiesPanelHit, PropertiesPanelLayout, PropertiesPanelLock,
    PropertiesRowControl, PropertiesRowGeometry, SelectionPropertyEntry, SelectionPropertyKind,
    SelectionPropertyValue, ShapePropertiesPanel,
};
use super::super::utils::palette_position;
use crate::draw::ArrowStyle;

/// How a row presents its control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowShape {
    /// The control sits beside the label.
    Inline,
    /// The control sits on its own line under the label.
    Block,
}

fn row_shape(entry: &SelectionPropertyEntry) -> RowShape {
    match entry.state {
        SelectionPropertyValue::Color(_)
        | SelectionPropertyValue::Fill(_)
        | SelectionPropertyValue::ArrowStyle(_) => RowShape::Block,
        SelectionPropertyValue::Level(_)
        | SelectionPropertyValue::Number(_)
        | SelectionPropertyValue::PressureVaries
        | SelectionPropertyValue::Toggle(_)
        | SelectionPropertyValue::ArrowHead(_) => RowShape::Inline,
    }
}

/// Whether a swatch row leads with a "no fill" cell: the fill row does.
fn leads_with_no_fill(entry: &SelectionPropertyEntry) -> bool {
    matches!(entry.state, SelectionPropertyValue::Fill(_))
}

/// Cells in a swatch row: its swatches, the trailing "more colors" button,
/// and the fill row's leading "no fill".
pub(in crate::input::state::core::properties) fn swatch_cells(
    entry: &SelectionPropertyEntry,
    swatches: usize,
) -> usize {
    swatches + 1 + usize::from(leads_with_no_fill(entry))
}

fn swatch_lines(cells: usize) -> usize {
    cells.div_ceil(SWATCH_ITEMS_PER_LINE)
}

/// Width of a swatch grid's widest line.
pub(in crate::input::state::core::properties) fn swatch_grid_width(cells: usize) -> f64 {
    let items = cells.min(SWATCH_ITEMS_PER_LINE) as f64;
    items * SWATCH_SIZE + (items - 1.0) * SWATCH_GAP
}

pub(in crate::input::state::core::properties) fn row_height(
    entry: &SelectionPropertyEntry,
    swatches: usize,
) -> f64 {
    let control_height = match entry.state {
        SelectionPropertyValue::Color(_) | SelectionPropertyValue::Fill(_) => {
            let lines = swatch_lines(swatch_cells(entry, swatches)) as f64;
            lines * SWATCH_SIZE + (lines - 1.0) * SWATCH_LINE_GAP
        }
        SelectionPropertyValue::ArrowStyle(_) => STYLE_BUTTON_HEIGHT,
        _ => return ROW_HEIGHT,
    };
    BLOCK_TOP + BLOCK_LABEL_LINE + BLOCK_GAP + control_height + BLOCK_BOTTOM
}

/// Which column each row lands in and how far below the first row's top it
/// starts. Rows fill a column until the next would pass `budget`, then start
/// the next column; a row taller than the budget still gets one to itself.
pub(in crate::input::state::core::properties) fn column_slots(
    heights: impl IntoIterator<Item = f64>,
    budget: f64,
) -> Vec<(usize, f64)> {
    let mut column = 0;
    let mut top = 0.0;
    let mut slots = Vec::new();
    for height in heights {
        if top > 0.0 && top + height > budget + 1e-6 {
            column += 1;
            top = 0.0;
        }
        slots.push((column, top));
        top += height;
    }
    slots
}

/// The smallest budget that still fits the rows in as few columns as
/// `max_budget` allows, so the columns come out about even rather than one
/// full column and a short one.
pub(in crate::input::state::core::properties) fn balanced_column_budget(
    heights: &[f64],
    max_budget: f64,
) -> f64 {
    let columns = |budget: f64| {
        column_slots(heights.iter().copied(), budget)
            .last()
            .map_or(0, |(column, _)| column + 1)
    };
    let target = columns(max_budget);
    let mut low = heights.iter().copied().fold(0.0, f64::max);
    let mut high = max_budget.max(low);
    for _ in 0..32 {
        let middle = (low + high) / 2.0;
        if columns(middle) <= target {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

impl PropertiesPanelLayout {
    pub fn rect(&self) -> PanelRect {
        PanelRect::new(self.origin_x, self.origin_y, self.width, self.height)
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.rect().contains(x, y)
    }

    pub fn content_x(&self) -> f64 {
        self.origin_x + self.padding_x
    }

    pub fn content_right(&self) -> f64 {
        self.origin_x + self.width - self.padding_x
    }

    /// The band rows are drawn in: to the footer, the panel's bottom, or the
    /// scroll viewport's.
    pub fn rows_viewport(&self) -> PanelRect {
        let bottom = self
            .scroll
            .map_or(self.origin_y + self.height, |scroll| scroll.viewport_bottom);
        PanelRect::new(
            self.origin_x,
            self.rows_top,
            self.width,
            bottom - self.rows_top,
        )
    }

    fn rows_visible_at(&self, y: f64) -> bool {
        self.scroll
            .is_none_or(|scroll| y >= self.rows_top && y < scroll.viewport_bottom)
    }

    /// The actions area's buttons: the four ordering buttons, and Duplicate
    /// and Delete under them, both after the label column; then Save at the
    /// end of the presets' label line, and the preset slots under it across
    /// the full content width, which gives each chip room for its dot and
    /// number.
    pub fn action_buttons(&self) -> Vec<(PanelAction, PanelRect)> {
        let left = self.content_x() + ACTIONS_LABEL_WIDTH;
        let width = self.content_right() - left;
        let row = |top: f64, actions: &[PanelAction]| {
            let count = actions.len() as f64;
            let button = (width - ACTION_BUTTON_GAP * (count - 1.0)) / count;
            actions
                .iter()
                .enumerate()
                .map(|(index, action)| {
                    let x = left + index as f64 * (button + ACTION_BUTTON_GAP);
                    (
                        *action,
                        PanelRect::new(x, top, button, ACTION_BUTTON_HEIGHT),
                    )
                })
                .collect::<Vec<_>>()
        };
        let order_top = self.actions_top + ACTIONS_TOP_GAP;
        let edit_top = order_top + ACTION_BUTTON_HEIGHT + ACTION_ROW_GAP;
        let mut buttons = row(order_top, &PanelAction::ORDER);
        buttons.extend(row(edit_top, &PanelAction::EDIT));

        let preset_label_top = edit_top + ACTION_BUTTON_HEIGHT + ACTION_ROW_GAP;
        let chips_top = preset_label_top + ACTION_BUTTON_HEIGHT + ACTION_ROW_GAP;
        let slots = self.preset_slots.max(1) as f64;
        let chip =
            (self.content_right() - self.content_x() - ACTION_BUTTON_GAP * (slots - 1.0)) / slots;
        buttons.extend((0..self.preset_slots).map(|index| {
            let x = self.content_x() + index as f64 * (chip + ACTION_BUTTON_GAP);
            (
                PanelAction::Preset(index + 1),
                PanelRect::new(x, chips_top, chip, ACTION_BUTTON_HEIGHT),
            )
        }));
        buttons.push((
            PanelAction::SavePreset,
            PanelRect::new(
                self.content_right() - PRESET_SAVE_WIDTH,
                preset_label_top,
                PRESET_SAVE_WIDTH,
                ACTION_BUTTON_HEIGHT,
            ),
        ));
        buttons
    }

    /// Left edge of the content of `column`.
    pub fn column_x(&self, column: usize) -> f64 {
        self.content_x() + column as f64 * (self.column_width + COLUMN_SPACING)
    }

    /// The title's ink box, which shows the shape details on hover.
    pub fn title_rect(&self) -> PanelRect {
        PanelRect::new(
            self.content_x(),
            self.title_baseline_y - TITLE_FONT,
            self.title_width,
            TITLE_FONT + 5.0,
        )
    }

    /// Every row of `panel`, in column order, top to bottom.
    pub fn rows(&self, panel: &ShapePropertiesPanel) -> Vec<PropertiesRowGeometry> {
        let swatches = panel.swatches.len();
        let heights: Vec<f64> = panel
            .entries
            .iter()
            .map(|entry| row_height(entry, swatches))
            .collect();
        let slots = column_slots(heights.iter().copied(), self.column_budget);
        panel
            .entries
            .iter()
            .zip(heights)
            .zip(slots)
            .enumerate()
            .map(|(index, ((entry, height), (column, offset)))| {
                let top = self.rows_top + offset - self.scroll.map_or(0.0, |scroll| scroll.offset);
                self.row_geometry(index, entry, top, height, self.column_x(column), swatches)
            })
            .collect()
    }

    fn row_geometry(
        &self,
        index: usize,
        entry: &SelectionPropertyEntry,
        top: f64,
        height: f64,
        left: f64,
        swatches: usize,
    ) -> PropertiesRowGeometry {
        let reach = self.padding_x - ROW_INSET;
        let rect = PanelRect::new(left - reach, top, self.column_width + reach * 2.0, height);
        let right = left + self.column_width;

        if row_shape(entry) == RowShape::Block {
            let control_top = top + BLOCK_TOP + BLOCK_LABEL_LINE + BLOCK_GAP;
            let control = match entry.state {
                SelectionPropertyValue::ArrowStyle(_) => {
                    self.arrow_style_buttons(left, control_top)
                }
                _ => swatch_grid(left, control_top, swatches, leads_with_no_fill(entry)),
            };
            return PropertiesRowGeometry {
                index,
                rect,
                content_x: left,
                content_right: right,
                label_baseline_y: top + BLOCK_TOP + BLOCK_LABEL_LINE * 0.75,
                control,
            };
        }

        let center_y = top + height / 2.0;
        let control = match entry.state {
            SelectionPropertyValue::Toggle(_) => PropertiesRowControl::Toggle {
                switch: PanelRect::new(
                    right - SWITCH_WIDTH,
                    center_y - SWITCH_HEIGHT / 2.0,
                    SWITCH_WIDTH,
                    SWITCH_HEIGHT,
                ),
            },
            SelectionPropertyValue::ArrowHead(_) => {
                let segment = self.head_segment_width;
                let well_width = segment * 2.0 + SEGMENT_PAD * 3.0;
                let well_height = SEGMENT_HEIGHT + SEGMENT_PAD * 2.0;
                let well = PanelRect::new(
                    right - well_width,
                    center_y - well_height / 2.0,
                    well_width,
                    well_height,
                );
                let start = PanelRect::new(
                    well.x + SEGMENT_PAD,
                    well.y + SEGMENT_PAD,
                    segment,
                    SEGMENT_HEIGHT,
                );
                let end = PanelRect::new(
                    start.right() + SEGMENT_PAD,
                    start.y,
                    segment,
                    SEGMENT_HEIGHT,
                );
                PropertiesRowControl::ArrowHead { well, start, end }
            }
            SelectionPropertyValue::Level(_) => {
                let value = PanelRect::new(
                    right - self.slider_value_width,
                    center_y - SLIDER_HIT_HEIGHT / 2.0,
                    self.slider_value_width,
                    SLIDER_HIT_HEIGHT,
                );
                let track_x = left + self.label_column;
                PropertiesRowControl::Slider {
                    track: PanelRect::new(
                        track_x,
                        center_y - SLIDER_HIT_HEIGHT / 2.0,
                        (value.x - SLIDER_VALUE_GAP - track_x).max(SLIDER_THUMB_RADIUS * 2.0),
                        SLIDER_HIT_HEIGHT,
                    ),
                    value,
                }
            }
            _ => {
                let total = STEP_BUTTON_WIDTH * 2.0 + self.stepper_value_width;
                let x = right - total;
                let y = center_y - STEPPER_HEIGHT / 2.0;
                let down = PanelRect::new(x, y, STEP_BUTTON_WIDTH, STEPPER_HEIGHT);
                let value =
                    PanelRect::new(down.right(), y, self.stepper_value_width, STEPPER_HEIGHT);
                let up = PanelRect::new(value.right(), y, STEP_BUTTON_WIDTH, STEPPER_HEIGHT);
                PropertiesRowControl::Stepper { down, value, up }
            }
        };

        PropertiesRowGeometry {
            index,
            rect,
            content_x: left,
            content_right: right,
            label_baseline_y: center_y + BODY_FONT * 0.35,
            control,
        }
    }

    fn arrow_style_buttons(&self, left: f64, top: f64) -> PropertiesRowControl {
        let count = ArrowStyle::ALL.len() as f64;
        let width = (self.column_width - STYLE_BUTTON_GAP * (count - 1.0)) / count;
        let buttons = ArrowStyle::ALL
            .into_iter()
            .enumerate()
            .map(|(index, style)| {
                let x = left + index as f64 * (width + STYLE_BUTTON_GAP);
                (style, PanelRect::new(x, top, width, STYLE_BUTTON_HEIGHT))
            })
            .collect();
        PropertiesRowControl::ArrowStyles { buttons }
    }

    /// The panel part at `(x, y)`, or `None` off the panel or between parts.
    pub fn hit_at(
        &self,
        panel: &ShapePropertiesPanel,
        x: f64,
        y: f64,
    ) -> Option<PropertiesPanelHit> {
        if !self.contains(x, y) {
            return None;
        }
        if self.lock.contains(x, y) {
            return Some(PropertiesPanelHit::Lock);
        }
        if self.title_rect().contains(x, y) {
            return Some(PropertiesPanelHit::Title);
        }
        if y >= self.actions_top {
            return self
                .action_buttons()
                .into_iter()
                .find(|(_, rect)| rect.contains(x, y))
                .map(|(action, _)| PropertiesPanelHit::Action(action));
        }
        // Rows scrolled out of the viewport are clipped, so they take no clicks.
        if !self.rows_visible_at(y) {
            return None;
        }

        let row = self
            .rows(panel)
            .into_iter()
            .find(|row| row.rect.contains(x, y))?;
        let index = row.index;
        let hit = match &row.control {
            PropertiesRowControl::Swatches {
                swatches,
                none,
                more,
            } => {
                if more.is_some_and(|more| more.contains(x, y)) {
                    Some(PropertiesPanelHit::MoreColors(index))
                } else if none.is_some_and(|none| none.contains(x, y)) {
                    Some(PropertiesPanelHit::NoFill(index))
                } else {
                    swatches
                        .iter()
                        .position(|rect| rect.contains(x, y))
                        .map(|swatch| PropertiesPanelHit::Swatch {
                            row: index,
                            index: swatch,
                        })
                }
            }
            PropertiesRowControl::Slider { track, .. } => track
                .contains(x, y)
                .then_some(PropertiesPanelHit::Slider(index)),
            PropertiesRowControl::Stepper { down, up, .. } => {
                if down.contains(x, y) {
                    Some(PropertiesPanelHit::StepDown(index))
                } else if up.contains(x, y) {
                    Some(PropertiesPanelHit::StepUp(index))
                } else {
                    None
                }
            }
            PropertiesRowControl::Toggle { switch } => switch
                .contains(x, y)
                .then_some(PropertiesPanelHit::Toggle(index)),
            PropertiesRowControl::ArrowHead { start, end, .. } => {
                if start.contains(x, y) {
                    Some(PropertiesPanelHit::ArrowHead {
                        row: index,
                        at_end: false,
                    })
                } else if end.contains(x, y) {
                    Some(PropertiesPanelHit::ArrowHead {
                        row: index,
                        at_end: true,
                    })
                } else {
                    None
                }
            }
            PropertiesRowControl::ArrowStyles { buttons } => buttons
                .iter()
                .find(|(_, rect)| rect.contains(x, y))
                .map(|(style, _)| PropertiesPanelHit::ArrowStyle {
                    row: index,
                    style: *style,
                }),
        };
        Some(hit.unwrap_or(PropertiesPanelHit::Row(index)))
    }

    /// Where `hit` is drawn: its tooltip's anchor, and where a click on it
    /// lands.
    pub fn hit_rect(
        &self,
        panel: &ShapePropertiesPanel,
        hit: PropertiesPanelHit,
    ) -> Option<PanelRect> {
        match hit {
            PropertiesPanelHit::Title => return Some(self.title_rect()),
            PropertiesPanelHit::Lock => return Some(self.lock),
            PropertiesPanelHit::Action(action) => {
                return self
                    .action_buttons()
                    .into_iter()
                    .find(|(candidate, _)| *candidate == action)
                    .map(|(_, rect)| rect);
            }
            _ => {}
        }
        let row = self.rows(panel).into_iter().nth(hit.row()?)?;
        let rect = match (hit, row.control) {
            (PropertiesPanelHit::Row(_), _) => row.rect,
            (
                PropertiesPanelHit::Swatch { index, .. },
                PropertiesRowControl::Swatches { swatches, .. },
            ) => *swatches.get(index)?,
            (PropertiesPanelHit::MoreColors(_), PropertiesRowControl::Swatches { more, .. }) => {
                more?
            }
            (PropertiesPanelHit::NoFill(_), PropertiesRowControl::Swatches { none, .. }) => none?,
            (PropertiesPanelHit::Slider(_), PropertiesRowControl::Slider { track, .. }) => track,
            (PropertiesPanelHit::StepDown(_), PropertiesRowControl::Stepper { down, .. }) => down,
            (PropertiesPanelHit::StepUp(_), PropertiesRowControl::Stepper { up, .. }) => up,
            (PropertiesPanelHit::Toggle(_), PropertiesRowControl::Toggle { switch }) => switch,
            (
                PropertiesPanelHit::ArrowHead { at_end, .. },
                PropertiesRowControl::ArrowHead { start, end, .. },
            ) => {
                if at_end {
                    end
                } else {
                    start
                }
            }
            (
                PropertiesPanelHit::ArrowStyle { style, .. },
                PropertiesRowControl::ArrowStyles { buttons },
            ) => buttons
                .into_iter()
                .find(|(candidate, _)| *candidate == style)
                .map(|(_, rect)| rect)?,
            _ => return None,
        };
        Some(rect)
    }
}

/// A swatch row's cells: an optional leading "no fill", the swatches, and a
/// trailing "more colors" button that opens the full color picker.
fn swatch_grid(x: f64, top: f64, swatches: usize, no_fill: bool) -> PropertiesRowControl {
    let cell = |item: usize| {
        let line = item / SWATCH_ITEMS_PER_LINE;
        let column = item % SWATCH_ITEMS_PER_LINE;
        PanelRect::new(
            x + column as f64 * (SWATCH_SIZE + SWATCH_GAP),
            top + line as f64 * (SWATCH_SIZE + SWATCH_LINE_GAP),
            SWATCH_SIZE,
            SWATCH_SIZE,
        )
    };
    let first = usize::from(no_fill);
    PropertiesRowControl::Swatches {
        swatches: (first..first + swatches).map(cell).collect(),
        none: no_fill.then(|| cell(0)),
        more: Some(cell(first + swatches)),
    }
}

impl SelectionPropertyEntry {
    /// What a stepper shows between its buttons. A pressure stroke's width
    /// varies along it, which a stepper readout can only say briefly.
    pub fn stepper_text(&self) -> &str {
        match self.state {
            SelectionPropertyValue::PressureVaries if !self.disabled => "Varies",
            _ => &self.value,
        }
    }
}

impl ShapePropertiesPanel {
    /// The swatch holding the row's single color, which gets the selection
    /// ring. `None` for a mixed, locked, or custom color.
    pub fn current_swatch(&self, entry: &SelectionPropertyEntry) -> Option<usize> {
        let (SelectionPropertyValue::Color(Some(color))
        | SelectionPropertyValue::Fill(Some(Some(color)))) = entry.state
        else {
            return None;
        };
        palette_position(self.swatches.iter().map(|swatch| swatch.color), color)
    }

    /// The text shown while the pointer rests on `hit`, if it has any.
    pub fn tooltip(&self, hit: PropertiesPanelHit) -> Option<String> {
        match hit {
            PropertiesPanelHit::Title => self.details.clone(),
            PropertiesPanelHit::Lock => Some(
                match (self.lock, self.multiple_selection) {
                    (PropertiesPanelLock::Locked, false) => "Unlock shape",
                    (PropertiesPanelLock::Locked, true) => "Unlock shapes",
                    (PropertiesPanelLock::Partial, _) => "Lock all shapes",
                    (PropertiesPanelLock::Unlocked, false) => "Lock shape",
                    (PropertiesPanelLock::Unlocked, true) => "Lock shapes",
                }
                .to_string(),
            ),
            PropertiesPanelHit::Swatch { index, .. } => {
                self.swatches.get(index).map(|swatch| swatch.label.clone())
            }
            PropertiesPanelHit::MoreColors(row) => Some(
                match self.entries.get(row).map(|entry| entry.kind) {
                    Some(SelectionPropertyKind::Fill) => "More fill colors…",
                    _ => "More colors…",
                }
                .to_string(),
            ),
            PropertiesPanelHit::NoFill(_) => Some("No fill".to_string()),
            PropertiesPanelHit::Action(action) => match action {
                PanelAction::ToBack => Some("Send to back".to_string()),
                PanelAction::Backward => Some("Send backward".to_string()),
                PanelAction::Forward => Some("Bring forward".to_string()),
                PanelAction::ToFront => Some("Bring to front".to_string()),
                PanelAction::Duplicate | PanelAction::Delete => None,
                PanelAction::SavePreset => Some(
                    if self.preset_save_mode {
                        "Pick a slot to save into"
                    } else {
                        "Save the selection's style as a preset"
                    }
                    .to_string(),
                ),
                PanelAction::Preset(slot) => Some(if self.preset_save_mode {
                    format!("Save to preset {slot}")
                } else {
                    match self.actions.presets.get(slot - 1) {
                        Some(Some(preset)) => match &preset.name {
                            Some(name) => format!("Apply preset {slot}: {name}"),
                            None => format!("Apply preset {slot}"),
                        },
                        _ => format!("Preset {slot} is empty"),
                    }
                }),
            },
            _ => None,
        }
    }
}
