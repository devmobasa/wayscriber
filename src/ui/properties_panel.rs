//! The shape properties panel: a header naming the selection, one control per
//! property (swatches, steppers, switches, the arrow-head segments, drawn
//! arrow styles), and a keyboard hint strip.
//!
//! Every rectangle comes from the panel layout's geometry, the same one the
//! input side hit-tests against.

use cairo::FontWeight;

use crate::input::InputState;
use crate::input::state::properties_panel_metrics::{
    BODY_FONT, FOOTER_FONT, FOOTER_HEIGHT, SUBTITLE_FONT, SWITCH_VALUE_GAP, TITLE_FONT,
    TOOLTIP_FONT, TOOLTIP_PADDING_X, TOOLTIP_PADDING_Y, VALUE_FONT, text_style,
};
use crate::input::state::{
    PropertiesPanelHit, PropertiesPanelLayout, PropertiesRowControl, PropertiesRowGeometry,
    ShapePropertiesPanel,
};
use crate::input::{SelectionPropertyEntry, SelectionPropertyValue};
use crate::ui::primitives::{draw_keycap_with_engine, draw_rounded_rect, keycap_size_with_engine};
use crate::ui::theme::overlay::{
    BG_HOVER_WASH, BORDER_FOCUS, BORDER_PROPERTIES, DIVIDER, DIVIDER_LIGHT, EMPTY_PROPERTIES,
    FOCUS_RING_WIDTH, PANEL_BG_CONTEXT_SUBMENU, RADIUS_PANEL, RADIUS_STD, TEXT_DISABLED, TEXT_HINT,
    TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY,
};
use crate::ui::theme::{Rgba, set_color, with_alpha};
use crate::ui_text::UiTextEngine;

mod controls;

use controls::{
    ControlState, draw_arrow_head_segments, draw_arrow_styles, draw_lock, draw_stepper,
    draw_swatches, draw_switch,
};

/// Wash behind the hovered row: a quieter `BG_HOVER`, so the control under
/// the pointer still stands out inside it.
const ROW_HOVER: Rgba = (0.25, 0.32, 0.45, 0.45);

pub fn render_properties_panel(
    ctx: &cairo::Context,
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) {
    render_properties_panel_with_engine(
        &UiTextEngine::default(),
        ctx,
        input_state,
        screen_width,
        screen_height,
    );
}

pub(crate) fn render_properties_panel_with_engine(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    input_state: &InputState,
    _screen_width: u32,
    _screen_height: u32,
) {
    let (Some(panel), Some(layout)) = (
        input_state.properties_panel(),
        input_state.properties_panel_layout(),
    ) else {
        return;
    };

    let _ = ctx.save();
    // Background and hairline border (popover radius, matching the other
    // overlay popups).
    draw_rounded_rect(
        ctx,
        layout.origin_x,
        layout.origin_y,
        layout.width,
        layout.height,
        RADIUS_PANEL,
    );
    set_color(ctx, crate::ui::theme::popup::bg_properties());
    let _ = ctx.fill_preserve();
    set_color(ctx, crate::ui::theme::popup::border_properties());
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    // Header: the shape type (or count), its layer and size, and the lock.
    set_color(
        ctx,
        if panel.multiple_selection {
            TEXT_SECONDARY
        } else {
            TEXT_PRIMARY
        },
    );
    engine.draw_baseline(
        ctx,
        text_style(TITLE_FONT, FontWeight::Bold),
        &panel.title,
        layout.content_x(),
        layout.title_baseline_y,
        None,
    );
    if let (Some(subtitle), Some(baseline)) = (&panel.subtitle, layout.subtitle_baseline_y) {
        set_color(ctx, TEXT_TERTIARY);
        engine.draw_baseline(
            ctx,
            text_style(SUBTITLE_FONT, FontWeight::Normal),
            subtitle,
            layout.content_x(),
            baseline,
            None,
        );
    }
    draw_lock(
        ctx,
        layout.lock,
        panel.lock,
        panel.hover == Some(PropertiesPanelHit::Lock),
    );

    set_color(ctx, DIVIDER);
    ctx.set_line_width(1.0);
    ctx.move_to(layout.content_x(), layout.divider_y + 0.5);
    ctx.line_to(layout.content_right(), layout.divider_y + 0.5);
    let _ = ctx.stroke();

    if panel.entries.is_empty() {
        let empty_style = crate::ui_text::UiTextStyle {
            slant: cairo::FontSlant::Italic,
            ..text_style(BODY_FONT, FontWeight::Normal)
        };
        set_color(ctx, TEXT_TERTIARY);
        engine.draw_baseline(
            ctx,
            empty_style,
            EMPTY_PROPERTIES,
            layout.content_x(),
            layout.rows_top + BODY_FONT + 4.0,
            None,
        );
    } else {
        for row in layout.rows(panel) {
            let entry = &panel.entries[row.index];
            draw_row(engine, ctx, panel, &row, entry);
        }
    }

    if let Some(footer_top) = layout.footer_top {
        draw_footer(engine, ctx, layout, footer_top);
    }

    if let (Some(hit), Some(rect)) = (panel.hover, layout.tooltip)
        && let Some(text) = panel.tooltip(hit)
    {
        draw_rounded_rect(ctx, rect.x, rect.y, rect.width, rect.height, RADIUS_STD);
        set_color(ctx, PANEL_BG_CONTEXT_SUBMENU);
        let _ = ctx.fill_preserve();
        set_color(ctx, BORDER_PROPERTIES);
        ctx.set_line_width(1.0);
        let _ = ctx.stroke();
        set_color(ctx, TEXT_PRIMARY);
        engine.draw_baseline(
            ctx,
            text_style(TOOLTIP_FONT, FontWeight::Normal),
            &text,
            rect.x + TOOLTIP_PADDING_X,
            rect.y + TOOLTIP_PADDING_Y + TOOLTIP_FONT,
            None,
        );
    }

    let _ = ctx.restore();
}

fn draw_row(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    panel: &ShapePropertiesPanel,
    row: &PropertiesRowGeometry,
    entry: &SelectionPropertyEntry,
) {
    let index = row.index;
    let enabled = !entry.disabled;
    let hover = panel.hover.filter(|hit| hit.row() == Some(index));
    let rect = row.rect;

    if enabled && hover.is_some() {
        set_color(ctx, ROW_HOVER);
        draw_rounded_rect(ctx, rect.x, rect.y, rect.width, rect.height, RADIUS_STD);
        let _ = ctx.fill();
    }
    if enabled && panel.focus_visible && panel.keyboard_focus == Some(index) {
        set_color(ctx, BORDER_FOCUS);
        ctx.set_line_width(FOCUS_RING_WIDTH);
        draw_rounded_rect(
            ctx,
            rect.x + 1.0,
            rect.y + 1.0,
            rect.width - 2.0,
            rect.height - 2.0,
            RADIUS_STD,
        );
        let _ = ctx.stroke();
    }

    set_color(ctx, if enabled { TEXT_PRIMARY } else { TEXT_DISABLED });
    engine.draw_baseline(
        ctx,
        text_style(BODY_FONT, FontWeight::Normal),
        &entry.label,
        row.content_x,
        row.label_baseline_y,
        None,
    );

    let state = ControlState { enabled, hover };
    match &row.control {
        PropertiesRowControl::Swatches { swatches, more } => {
            draw_value_right(engine, ctx, row, entry, enabled);
            let current = panel.current_swatch(entry);
            draw_swatches(ctx, &panel.swatches, swatches, *more, current, state);
        }
        PropertiesRowControl::ArrowStyles { buttons } => {
            draw_value_right(engine, ctx, row, entry, enabled);
            let current = match entry.state {
                SelectionPropertyValue::ArrowStyle(style) => style,
                _ => None,
            };
            draw_arrow_styles(engine, ctx, buttons, current, state);
        }
        PropertiesRowControl::Stepper {
            down,
            value,
            up,
            preview,
        } => {
            let thickness = match entry.state {
                SelectionPropertyValue::Number(value) => value,
                _ => None,
            };
            draw_stepper(
                engine,
                ctx,
                (*down, *value, *up),
                entry.stepper_text(),
                preview.zip(thickness),
                panel.preview_color,
                state,
            );
        }
        PropertiesRowControl::Toggle { switch } => {
            let on = match entry.state {
                SelectionPropertyValue::Toggle(on) => on,
                _ => None,
            };
            set_color(ctx, if enabled { TEXT_HINT } else { TEXT_DISABLED });
            let style = text_style(VALUE_FONT, FontWeight::Normal);
            let width = engine
                .layout(ctx, style, &entry.value, None)
                .ink_extents()
                .x_advance();
            engine.draw_baseline(
                ctx,
                style,
                &entry.value,
                switch.x - SWITCH_VALUE_GAP - width,
                row.label_baseline_y,
                None,
            );
            draw_switch(ctx, *switch, on, state);
        }
        PropertiesRowControl::ArrowHead { well, start, end } => {
            let at_end = match entry.state {
                SelectionPropertyValue::ArrowHead(at_end) => at_end,
                _ => None,
            };
            draw_arrow_head_segments(engine, ctx, *well, *start, *end, at_end, state);
        }
    }
}

/// A block row's value, right-aligned on its label line: which color or
/// style the selection has, or "Mixed" / "Locked".
fn draw_value_right(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    row: &PropertiesRowGeometry,
    entry: &SelectionPropertyEntry,
    enabled: bool,
) {
    let style = text_style(VALUE_FONT, FontWeight::Normal);
    let width = engine
        .layout(ctx, style, &entry.value, None)
        .ink_extents()
        .x_advance();
    set_color(ctx, if enabled { TEXT_HINT } else { TEXT_DISABLED });
    engine.draw_baseline(
        ctx,
        style,
        &entry.value,
        row.content_right - width,
        row.label_baseline_y,
        None,
    );
}

/// The keyboard hint strip: arrow keys pick a row and adjust it, and the
/// wheel steps whichever row is under the pointer.
fn draw_footer(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    layout: &PropertiesPanelLayout,
    footer_top: f64,
) {
    set_color(ctx, with_alpha(DIVIDER_LIGHT, 0.4));
    ctx.set_line_width(1.0);
    ctx.move_to(layout.content_x(), footer_top + 4.5);
    ctx.line_to(layout.content_right(), footer_top + 4.5);
    let _ = ctx.stroke();

    // Keys and words share one center line, whatever each keycap's ink.
    let style = text_style(FOOTER_FONT, FontWeight::Normal);
    let center_y = footer_top + 4.0 + (FOOTER_HEIGHT - 4.0) / 2.0;
    let baseline = center_y + FOOTER_FONT * 0.35;
    let mut x = layout.content_x();
    let segments: [(Option<&str>, &str); 3] = [
        (Some("↑↓"), "row"),
        (Some("←→"), "adjust"),
        (None, "scroll to step"),
    ];
    for (key, text) in segments {
        if let Some(key) = key {
            let (_, height) = keycap_size_with_engine(engine, ctx, key, FOOTER_FONT);
            let (width, _) = draw_keycap_with_engine(
                engine,
                ctx,
                x,
                center_y - height / 2.0,
                key,
                FOOTER_FONT,
                BG_HOVER_WASH,
                TEXT_SECONDARY,
            );
            x += width + 4.0;
        }
        set_color(ctx, TEXT_TERTIARY);
        let extents = engine.draw_baseline(ctx, style, text, x, baseline, None);
        x += extents.x_advance() + 12.0;
    }
}
