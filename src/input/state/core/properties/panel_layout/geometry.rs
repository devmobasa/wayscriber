//! Where each part of the properties panel sits, derived from the cached
//! layout. Hit-testing and painting both read these rectangles, so a control
//! is clickable exactly where it is drawn.

use super::super::metrics::{
    BLOCK_BOTTOM, BLOCK_GAP, BLOCK_LABEL_LINE, BLOCK_TOP, BODY_FONT, COLUMN_SPACING, PREVIEW_GAP,
    PREVIEW_HEIGHT, PREVIEW_WIDTH, ROW_HEIGHT, ROW_INSET, SEGMENT_HEIGHT, SEGMENT_PAD,
    STEP_BUTTON_WIDTH, STEPPER_HEIGHT, STYLE_BUTTON_GAP, STYLE_BUTTON_HEIGHT, SWATCH_GAP,
    SWATCH_ITEMS_PER_LINE, SWATCH_LINE_GAP, SWATCH_SIZE, SWITCH_HEIGHT, SWITCH_WIDTH, TITLE_FONT,
};
use super::super::types::{
    PanelRect, PropertiesPanelHit, PropertiesPanelLayout, PropertiesPanelLock,
    PropertiesRowControl, PropertiesRowGeometry, SelectionPropertyEntry, SelectionPropertyKind,
    SelectionPropertyValue, ShapePropertiesPanel,
};
use super::super::utils::color_eq;
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
        SelectionPropertyValue::Color(_) | SelectionPropertyValue::ArrowStyle(_) => RowShape::Block,
        SelectionPropertyValue::Number(_)
        | SelectionPropertyValue::PressureVaries
        | SelectionPropertyValue::Toggle(_)
        | SelectionPropertyValue::ArrowHead(_) => RowShape::Inline,
    }
}

/// Lines the swatch grid needs for `swatches` plus its "more colors" button.
fn swatch_lines(swatches: usize) -> usize {
    (swatches + 1).div_ceil(SWATCH_ITEMS_PER_LINE)
}

/// Width of the swatch grid's widest line.
pub(in crate::input::state::core::properties) fn swatch_grid_width(swatches: usize) -> f64 {
    let items = (swatches + 1).min(SWATCH_ITEMS_PER_LINE) as f64;
    items * SWATCH_SIZE + (items - 1.0) * SWATCH_GAP
}

pub(in crate::input::state::core::properties) fn row_height(
    entry: &SelectionPropertyEntry,
    swatches: usize,
) -> f64 {
    let control_height = match entry.state {
        SelectionPropertyValue::Color(_) => {
            let lines = swatch_lines(swatches) as f64;
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

/// Whether a stepper row draws a stroke at its current thickness.
pub(in crate::input::state::core::properties) fn shows_thickness_preview(
    entry: &SelectionPropertyEntry,
) -> bool {
    entry.kind == SelectionPropertyKind::Thickness
        && matches!(entry.state, SelectionPropertyValue::Number(Some(_)))
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
                let top = self.rows_top + offset;
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
                _ => swatch_grid(left, control_top, swatches),
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
            _ => {
                let total = STEP_BUTTON_WIDTH * 2.0 + self.stepper_value_width;
                let x = right - total;
                let y = center_y - STEPPER_HEIGHT / 2.0;
                let down = PanelRect::new(x, y, STEP_BUTTON_WIDTH, STEPPER_HEIGHT);
                let value =
                    PanelRect::new(down.right(), y, self.stepper_value_width, STEPPER_HEIGHT);
                let up = PanelRect::new(value.right(), y, STEP_BUTTON_WIDTH, STEPPER_HEIGHT);
                let preview = shows_thickness_preview(entry).then(|| {
                    PanelRect::new(
                        x - PREVIEW_GAP - PREVIEW_WIDTH,
                        center_y - PREVIEW_HEIGHT / 2.0,
                        PREVIEW_WIDTH,
                        PREVIEW_HEIGHT,
                    )
                });
                PropertiesRowControl::Stepper {
                    down,
                    value,
                    up,
                    preview,
                }
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

        let row = self
            .rows(panel)
            .into_iter()
            .find(|row| row.rect.contains(x, y))?;
        let index = row.index;
        let hit = match &row.control {
            PropertiesRowControl::Swatches { swatches, more } => {
                if more.contains(x, y) {
                    Some(PropertiesPanelHit::MoreColors(index))
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
                more
            }
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

fn swatch_grid(x: f64, top: f64, swatches: usize) -> PropertiesRowControl {
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
    PropertiesRowControl::Swatches {
        swatches: (0..swatches).map(cell).collect(),
        more: cell(swatches),
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
        let SelectionPropertyValue::Color(Some(color)) = entry.state else {
            return None;
        };
        self.swatches
            .iter()
            .position(|swatch| color_eq(&swatch.color, &color))
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
            PropertiesPanelHit::MoreColors(_) => Some("More colors…".to_string()),
            _ => None,
        }
    }
}
