use cairo::Context as CairoContext;
use cairo::FontWeight;

use super::super::super::base::InputState;
use super::super::metrics::{
    BODY_FONT, COLUMN_GAP, COLUMN_SPACING, EMPTY_HEIGHT, FOOTER_HEIGHT, HEADER_GAP, LOCK_INSET,
    LOCK_SIZE, MIN_WIDTH, PADDING_BOTTOM, PADDING_TOP, PADDING_X, PREVIEW_GAP, PREVIEW_WIDTH,
    ROWS_GAP, SEGMENT_ICON_GAP, SEGMENT_ICON_WIDTH, SEGMENT_PAD, SEGMENT_TEXT_PADDING,
    STEP_BUTTON_WIDTH, STEPPER_MIN_VALUE_WIDTH, STEPPER_VALUE_PADDING, STYLE_BUTTON_GAP,
    STYLE_BUTTON_MIN_WIDTH, SUBTITLE_FONT, SUBTITLE_STEP, SWITCH_VALUE_GAP, SWITCH_WIDTH,
    TITLE_FONT, TOOLTIP_FONT, TOOLTIP_GAP, TOOLTIP_PADDING_X, TOOLTIP_PADDING_Y, VALUE_FONT,
    text_style,
};
use super::super::types::{
    PanelRect, PropertiesPanelLayout, SelectionPropertyValue, ShapePropertiesPanel,
};
use super::geometry::{
    balanced_column_budget, column_slots, row_height, shows_thickness_preview, swatch_grid_width,
};
use super::{PANEL_ANCHOR_GAP, PANEL_MARGIN};
use crate::draw::ArrowStyle;
use crate::ui_text::UiTextEngine;
use crate::util::Rect;

/// What the layout needs measured from the panel's text.
struct Measured {
    title_width: f64,
    content_width: f64,
    stepper_value_width: f64,
    head_segment_width: f64,
}

fn measure_panel(
    engine: &UiTextEngine,
    ctx: &CairoContext,
    panel: &ShapePropertiesPanel,
) -> Measured {
    let width = |size: f64, weight: FontWeight, text: &str| {
        engine
            .layout(ctx, text_style(size, weight), text, None)
            .ink_extents()
            .x_advance()
    };

    let title_width = width(TITLE_FONT, FontWeight::Bold, &panel.title);
    let subtitle_width = panel.subtitle.as_deref().map_or(0.0, |subtitle| {
        width(SUBTITLE_FONT, FontWeight::Normal, subtitle)
    });

    let mut label_width: f64 = 0.0;
    let mut stepper_text_width: f64 = 0.0;
    let mut toggle_value_width: f64 = 0.0;
    for entry in &panel.entries {
        label_width = label_width.max(width(BODY_FONT, FontWeight::Normal, &entry.label));
        match entry.state {
            SelectionPropertyValue::Number(_) | SelectionPropertyValue::PressureVaries => {
                stepper_text_width = stepper_text_width.max(width(
                    VALUE_FONT,
                    FontWeight::Normal,
                    entry.stepper_text(),
                ));
            }
            SelectionPropertyValue::Toggle(_) => {
                toggle_value_width =
                    toggle_value_width.max(width(VALUE_FONT, FontWeight::Normal, &entry.value));
            }
            _ => {}
        }
    }
    let head_text_width = ["Start", "End"]
        .into_iter()
        .map(|text| width(VALUE_FONT, FontWeight::Normal, text))
        .fold(0.0, f64::max);
    let empty_width = if panel.entries.is_empty() {
        width(
            BODY_FONT,
            FontWeight::Normal,
            crate::ui::theme::overlay::EMPTY_PROPERTIES,
        )
    } else {
        0.0
    };

    let stepper_value_width = (stepper_text_width + STEPPER_VALUE_PADDING)
        .max(STEPPER_MIN_VALUE_WIDTH)
        .ceil();
    let head_segment_width =
        (head_text_width + SEGMENT_ICON_WIDTH + SEGMENT_ICON_GAP + SEGMENT_TEXT_PADDING * 2.0)
            .ceil();

    let mut content_width = (title_width + COLUMN_GAP + LOCK_SIZE)
        .max(subtitle_width)
        .max(empty_width);
    for entry in &panel.entries {
        let row_width = match entry.state {
            SelectionPropertyValue::Color(_) => {
                label_width.max(swatch_grid_width(panel.swatches.len()))
            }
            SelectionPropertyValue::ArrowStyle(_) => {
                let count = ArrowStyle::ALL.len() as f64;
                label_width.max(STYLE_BUTTON_MIN_WIDTH * count + STYLE_BUTTON_GAP * (count - 1.0))
            }
            SelectionPropertyValue::Number(_) => {
                let preview = if shows_thickness_preview(entry) {
                    PREVIEW_WIDTH + PREVIEW_GAP
                } else {
                    0.0
                };
                label_width + COLUMN_GAP + preview + STEP_BUTTON_WIDTH * 2.0 + stepper_value_width
            }
            SelectionPropertyValue::PressureVaries => {
                label_width + COLUMN_GAP + STEP_BUTTON_WIDTH * 2.0 + stepper_value_width
            }
            SelectionPropertyValue::Toggle(_) => {
                label_width + COLUMN_GAP + toggle_value_width + SWITCH_VALUE_GAP + SWITCH_WIDTH
            }
            SelectionPropertyValue::ArrowHead(_) => {
                label_width + COLUMN_GAP + head_segment_width * 2.0 + SEGMENT_PAD * 3.0
            }
        };
        content_width = content_width.max(row_width);
    }

    Measured {
        title_width,
        content_width,
        stepper_value_width,
        head_segment_width,
    }
}

impl InputState {
    pub fn clear_properties_panel_layout(&mut self) {
        self.properties.clear_layout();
    }

    pub fn update_properties_panel_layout(
        &mut self,
        ctx: &CairoContext,
        screen_width: u32,
        screen_height: u32,
    ) {
        self.update_properties_panel_layout_with_resources(
            &UiTextEngine::default(),
            &crate::draw::TextMeasurer::default(),
            ctx,
            screen_width,
            screen_height,
        );
    }

    pub(crate) fn update_properties_panel_layout_with_resources(
        &mut self,
        engine: &UiTextEngine,
        measurer: &crate::draw::TextMeasurer,
        ctx: &CairoContext,
        screen_width: u32,
        screen_height: u32,
    ) {
        if self.properties.needs_refresh() {
            self.refresh_properties_panel_with(measurer);
        }
        let Some(panel) = self.properties.panel.as_ref() else {
            self.properties.layout = None;
            return;
        };

        let _ = ctx.save();
        let measured = measure_panel(engine, ctx, panel);
        let _ = ctx.restore();

        let column_width = (measured.content_width + PADDING_X * 2.0)
            .max(MIN_WIDTH)
            .ceil()
            - PADDING_X * 2.0;
        let title_baseline = PADDING_TOP + TITLE_FONT;
        let last_baseline = if panel.subtitle.is_some() {
            title_baseline + SUBTITLE_STEP
        } else {
            title_baseline
        };
        let divider = last_baseline + HEADER_GAP;
        let rows_top = divider + ROWS_GAP;
        let heights: Vec<f64> = panel
            .entries
            .iter()
            .map(|entry| row_height(entry, panel.swatches.len()))
            .collect();
        let fit = fit_rows(&heights, rows_top, screen_height as f64);

        let columns = fit.columns as f64;
        let panel_width =
            PADDING_X * 2.0 + columns * column_width + (columns - 1.0) * COLUMN_SPACING;
        let tail = if panel.entries.is_empty() {
            EMPTY_HEIGHT
        } else if fit.footer {
            FOOTER_HEIGHT
        } else {
            0.0
        };
        let panel_height = (rows_top + fit.height + tail + PADDING_BOTTOM).ceil();

        let screen_w = screen_width as f64;
        let screen_h = screen_height as f64;
        let (origin_x, origin_y) = place_panel(
            panel.anchor,
            panel.anchor_rect,
            panel_width,
            panel_height,
            screen_w,
            screen_h,
        );

        let has_footer = !panel.entries.is_empty() && fit.footer;
        let has_subtitle = panel.subtitle.is_some();
        self.properties.layout = Some(PropertiesPanelLayout {
            origin_x,
            origin_y,
            width: panel_width,
            height: panel_height,
            padding_x: PADDING_X,
            title_baseline_y: origin_y + title_baseline,
            title_width: measured.title_width,
            subtitle_baseline_y: has_subtitle.then_some(origin_y + title_baseline + SUBTITLE_STEP),
            lock: PanelRect::new(
                origin_x + panel_width - LOCK_INSET - LOCK_SIZE,
                origin_y + LOCK_INSET,
                LOCK_SIZE,
                LOCK_SIZE,
            ),
            divider_y: origin_y + divider,
            rows_top: origin_y + rows_top,
            footer_top: has_footer.then_some(origin_y + rows_top + fit.height),
            stepper_value_width: measured.stepper_value_width,
            head_segment_width: measured.head_segment_width,
            column_width,
            column_budget: fit.budget,
            tooltip: None,
        });

        if self.properties.pending_hover_recalc {
            let focus_set = self
                .properties
                .panel
                .as_ref()
                .is_some_and(|panel| panel.keyboard_focus.is_some());
            if !focus_set {
                let (px, py) = self.pointer.screen();
                self.update_properties_panel_hover_from_pointer_internal(px, py, false);
            }
            self.properties.pending_hover_recalc = false;
        }

        let tooltip = self.properties_panel_tooltip_rect(engine, ctx, screen_w, screen_h);
        if let Some(layout) = self.properties.layout.as_mut() {
            layout.tooltip = tooltip;
        }

        if let Some(layout) = self.properties.layout {
            mark_properties_panel_region(self, layout);
        }
    }

    /// Where the hovered part's tooltip goes: centered under the part, or
    /// above it when there is no room below, and always on screen. The
    /// header's tooltips go above it instead, clear of the subtitle.
    fn properties_panel_tooltip_rect(
        &self,
        engine: &UiTextEngine,
        ctx: &CairoContext,
        screen_w: f64,
        screen_h: f64,
    ) -> Option<PanelRect> {
        let panel = self.properties.panel.as_ref()?;
        let layout = self.properties.layout.as_ref()?;
        let hit = panel.hover?;
        let text = panel.tooltip(hit)?;
        let anchor = layout.hit_rect(panel, hit)?;

        let _ = ctx.save();
        let text_width = engine
            .layout(
                ctx,
                text_style(TOOLTIP_FONT, FontWeight::Normal),
                &text,
                None,
            )
            .ink_extents()
            .x_advance();
        let _ = ctx.restore();

        let width = (text_width + TOOLTIP_PADDING_X * 2.0).ceil();
        let height = (TOOLTIP_FONT + 4.0 + TOOLTIP_PADDING_Y * 2.0).ceil();
        let (center_x, _) = anchor.center();
        let max_x = (screen_w - PANEL_MARGIN - width).max(PANEL_MARGIN);
        let x = (center_x - width / 2.0).clamp(PANEL_MARGIN, max_x);
        let below = anchor.bottom() + TOOLTIP_GAP;
        let above = anchor.y - TOOLTIP_GAP - height;
        let header = hit.row().is_none();
        let fits_below = below + height <= screen_h - PANEL_MARGIN;
        let fits_above = above >= PANEL_MARGIN;
        let y = if (header && fits_above) || !fits_below {
            above.max(PANEL_MARGIN)
        } else {
            below
        };
        Some(PanelRect::new(x, y, width, height))
    }
}

/// How the rows fit the screen's height.
struct RowFit {
    footer: bool,
    columns: usize,
    /// The column budget the geometry fills rows against.
    budget: f64,
    /// Height of the tallest column.
    height: f64,
}

/// Fits the rows under a header of `rows_top` into `screen_h`: in one column
/// with the keyboard hints when they fit, then without the hints, and
/// otherwise in as few even columns as fit. Every row stays on screen and
/// the wheel keeps stepping the row under it, which scrolling would take away.
fn fit_rows(heights: &[f64], rows_top: f64, screen_h: f64) -> RowFit {
    let total: f64 = heights.iter().sum();
    let available = if screen_h > 0.0 {
        screen_h - PANEL_MARGIN * 2.0
    } else {
        f64::INFINITY
    };
    let single = |footer: bool| RowFit {
        footer,
        columns: 1,
        budget: f64::INFINITY,
        height: total,
    };

    if rows_top + total + FOOTER_HEIGHT + PADDING_BOTTOM <= available {
        return single(true);
    }
    if rows_top + total + PADDING_BOTTOM <= available {
        return single(false);
    }

    let budget = balanced_column_budget(heights, available - rows_top - PADDING_BOTTOM);
    let mut column_heights: Vec<f64> = Vec::new();
    for ((column, offset), height) in column_slots(heights.iter().copied(), budget)
        .into_iter()
        .zip(heights)
    {
        if column_heights.len() <= column {
            column_heights.push(0.0);
        }
        column_heights[column] = offset + height;
    }
    RowFit {
        footer: false,
        columns: column_heights.len().max(1),
        budget,
        height: column_heights.iter().copied().fold(0.0, f64::max),
    }
}

/// Picks the panel's top-left corner: beside the selection on whichever side
/// leaves the most of it on screen, then clamped inside the screen margin.
fn place_panel(
    anchor: (f64, f64),
    anchor_rect: Option<Rect>,
    panel_width: f64,
    panel_height: f64,
    screen_w: f64,
    screen_h: f64,
) -> (f64, f64) {
    let (mut origin_x, mut origin_y) = if screen_w > 0.0 && screen_h > 0.0 {
        if let Some(bounds) = anchor_rect {
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
            anchor
        }
    } else {
        anchor
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
    (origin_x, origin_y)
}

fn mark_properties_panel_region(state: &mut InputState, layout: PropertiesPanelLayout) {
    let region = [Some(layout.rect()), layout.tooltip]
        .into_iter()
        .flatten()
        .reduce(|a, b| {
            let x = a.x.min(b.x);
            let y = a.y.min(b.y);
            PanelRect::new(
                x,
                y,
                a.right().max(b.right()) - x,
                a.bottom().max(b.bottom()) - y,
            )
        })
        .unwrap_or_else(|| layout.rect());
    let x = region.x.floor() as i32;
    let y = region.y.floor() as i32;
    let width = (region.width.ceil() as i32 + 2).max(1);
    let height = (region.height.ceil() as i32 + 2).max(1);

    if let Some(rect) = Rect::new(x, y, width, height) {
        state.dirty_tracker.mark_rect(rect);
    } else {
        state.dirty_tracker.mark_full();
    }
}
