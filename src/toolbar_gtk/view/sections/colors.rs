//! Colors section: HSV gradient picker, preview swatch + hex row, and the
//! quick-color swatch rows with the more/less palette toggle.

use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

/// Combines the current HSV triple with a drag position into a new triple.
type HsvApply = fn((f64, f64, f64), f64, f64) -> (f64, f64, f64);

use gtk4::prelude::*;

use crate::config::{Action, QuickColorPalette, QuickColorPaletteEntry};
use crate::draw::Color;
use crate::draw::color::{hsv_to_rgb, rgb_to_hsv};
use crate::input::state::{color_to_hex, parse_hex_color};
use crate::label_format::format_binding_label;
use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection, ToolbarSnapshot};

use super::super::super::icons::IconPainter;
use super::super::super::widgets::{
    FeedbackSender, icon_button, rounded_rect_path, send_event, sized_button,
};
use super::{SectionCtx, scoped_title, section_card};

// Spec units mirroring `ToolbarLayoutSpec::SIDE_COLOR_*`.
const SV_HEIGHT: f64 = 72.0;
const HUE_HEIGHT: f64 = 14.0;
const PREVIEW_SIZE: f64 = 28.0;
const PREVIEW_GAP_TOP: f64 = 10.0;
const PREVIEW_GAP_BOTTOM: f64 = 8.0;
const HEX_INPUT_WIDTH: f64 = 70.0;
const HEX_INPUT_HEIGHT: f64 = 20.0;
const EXPAND_ICON_SIZE: f64 = 8.0;
const SWATCH: f64 = 24.0;
const SWATCH_GAP: f64 = 6.0;
const SWATCHES_PER_ROW: usize = 6;
/// Body spacing already contributed by `section_card` between rows.
const BODY_SPACING: f64 = 6.0;

/// `(color, label, bound quick-color action)` like the built-in swatch rows.
type ColorSwatch = (Color, String, Option<Action>);

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let card = section_card(
        ctx,
        ToolbarSideSection::Colors,
        &scoped_title("Color", ctx.snapshot),
    );
    build_picker(ctx, &card.body);
    build_preview_row(ctx, &card.body);
    build_swatch_rows(ctx, &card.body);
    Some(card.root.upcast())
}

// ===== HSV gradient picker =================================================

/// The 2-D saturation/value area plus the hue bar. Both share one HSV cell:
/// drags write it and emit `SetColorHsv`; snapshot updates only land when no
/// drag is in flight (same rule as `SliderRow`).
fn build_picker(ctx: &mut SectionCtx, body: &gtk4::Box) {
    let hsv = Rc::new(Cell::new(effective_hsv(ctx.snapshot)));
    let dragging = Rc::new(Cell::new(false));
    let scale = ctx.scale;

    let sv_area = gtk4::DrawingArea::new();
    sv_area.set_content_height(ctx.px(SV_HEIGHT));
    sv_area.set_hexpand(true);
    let draw_hsv = hsv.clone();
    sv_area.set_draw_func(move |_, cr, width, height| {
        let (h, s, v) = draw_hsv.get();
        let w = f64::from(width);
        let hgt = f64::from(height);
        draw_sat_val(cr, w, hgt, h);
        draw_indicator(cr, s * w, (1.0 - v) * hgt, hsv_to_rgb(h, s, v), scale);
    });

    let hue_area = gtk4::DrawingArea::new();
    hue_area.set_content_height(ctx.px(HUE_HEIGHT));
    hue_area.set_hexpand(true);
    let draw_hsv = hsv.clone();
    hue_area.set_draw_func(move |_, cr, width, height| {
        let (h, _, _) = draw_hsv.get();
        let w = f64::from(width);
        let hgt = f64::from(height);
        draw_hue_bar(cr, w, hgt);
        draw_indicator(cr, h * w, hgt / 2.0, hsv_to_rgb(h, 1.0, 1.0), scale);
    });

    // SV area varies s/v at the current hue; hue bar varies h keeping s/v.
    attach_picker_drag(
        &sv_area,
        &hue_area,
        &hsv,
        &dragging,
        ctx.feedback.clone(),
        |(h, _, _), tx, ty| (h, tx, 1.0 - ty),
    );
    attach_picker_drag(
        &hue_area,
        &sv_area,
        &hsv,
        &dragging,
        ctx.feedback.clone(),
        |(_, s, v), tx, _| (tx, s, v),
    );

    body.append(&sv_area);
    body.append(&hue_area);

    let update_hsv = hsv.clone();
    let update_dragging = dragging.clone();
    ctx.updaters.push(Box::new(move |snapshot| {
        if update_dragging.get() {
            return;
        }
        let next = effective_hsv(snapshot);
        if next != update_hsv.get() {
            update_hsv.set(next);
            sv_area.queue_draw();
            hue_area.queue_draw();
        }
    }));
}

/// Press-and-drag picking on one gradient area. `apply` folds the clamped
/// normalized pointer position into the shared HSV triple; the peer area
/// redraws too because hue changes recolor the SV gradient.
fn attach_picker_drag(
    area: &gtk4::DrawingArea,
    peer: &gtk4::DrawingArea,
    hsv: &Rc<Cell<(f64, f64, f64)>>,
    dragging: &Rc<Cell<bool>>,
    sender: FeedbackSender,
    apply: HsvApply,
) {
    let apply_at: Rc<dyn Fn(f64, f64)> = Rc::new({
        let hsv = hsv.clone();
        let area = area.clone();
        let peer = peer.clone();
        move |x: f64, y: f64| {
            let w = f64::from(area.width().max(1));
            let hgt = f64::from(area.height().max(1));
            let (h, s, v) = apply(
                hsv.get(),
                (x / w).clamp(0.0, 1.0),
                (y / hgt).clamp(0.0, 1.0),
            );
            hsv.set((h, s, v));
            area.queue_draw();
            peer.queue_draw();
            send_event(&sender, ToolbarEvent::SetColorHsv { h, s, v });
        }
    });

    let drag = gtk4::GestureDrag::new();
    let start = Rc::new(Cell::new((0.0f64, 0.0f64)));
    let begin_dragging = dragging.clone();
    let begin_start = start.clone();
    let begin_apply = apply_at.clone();
    drag.connect_drag_begin(move |_, x, y| {
        begin_dragging.set(true);
        begin_start.set((x, y));
        begin_apply(x, y);
    });
    let update_apply = apply_at.clone();
    drag.connect_drag_update(move |_, dx, dy| {
        let (sx, sy) = start.get();
        update_apply(sx + dx, sy + dy);
    });
    let end_dragging = dragging.clone();
    drag.connect_drag_end(move |_, _, _| {
        end_dragging.set(false);
    });
    area.add_controller(drag);
}

/// HSV triple the picker should display. The remembered picker triple wins
/// while it still resolves to the current color, so hue and saturation stay
/// put when the RGB value collapses to gray/black; any other color source
/// (swatch, hex, preset) falls back to a plain RGB→HSV conversion.
fn effective_hsv(snapshot: &ToolbarSnapshot) -> (f64, f64, f64) {
    if let Some((h, s, v)) = snapshot.picker_hsv {
        let remembered = hsv_to_rgb(h, s, v);
        let current = snapshot.color;
        if (remembered.r - current.r).abs() < 1e-3
            && (remembered.g - current.g).abs() < 1e-3
            && (remembered.b - current.b).abs() < 1e-3
        {
            return (h, s, v);
        }
    }
    rgb_to_hsv(snapshot.color.r, snapshot.color.g, snapshot.color.b)
}

fn draw_sat_val(cr: &cairo::Context, w: f64, h: f64, hue: f64) {
    let hue_color = hsv_to_rgb(hue, 1.0, 1.0);

    let sat_grad = cairo::LinearGradient::new(0.0, 0.0, w, 0.0);
    sat_grad.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 1.0);
    sat_grad.add_color_stop_rgba(1.0, hue_color.r, hue_color.g, hue_color.b, 1.0);
    cr.rectangle(0.0, 0.0, w, h);
    let _ = cr.set_source(&sat_grad);
    let _ = cr.fill();

    let val_grad = cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
    val_grad.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
    val_grad.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 1.0);
    cr.rectangle(0.0, 0.0, w, h);
    let _ = cr.set_source(&val_grad);
    let _ = cr.fill();

    stroke_picker_border(cr, w, h);
}

fn draw_hue_bar(cr: &cairo::Context, w: f64, h: f64) {
    let hue_grad = cairo::LinearGradient::new(0.0, 0.0, w, 0.0);
    hue_grad.add_color_stop_rgba(0.0, 1.0, 0.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.17, 1.0, 1.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.33, 0.0, 1.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.5, 0.0, 1.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(0.66, 0.0, 0.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(0.83, 1.0, 0.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(1.0, 1.0, 0.0, 0.0, 1.0);
    cr.rectangle(0.0, 0.0, w, h);
    let _ = cr.set_source(&hue_grad);
    let _ = cr.fill();

    stroke_picker_border(cr, w, h);
}

fn stroke_picker_border(cr: &cairo::Context, w: f64, h: f64) {
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.4);
    cr.rectangle(0.5, 0.5, w - 1.0, h - 1.0);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
}

/// The color indicator dot the built-in picker draws at the current value.
fn draw_indicator(cr: &cairo::Context, x: f64, y: f64, color: Color, scale: f64) {
    let radius = 5.0 * scale;
    let ring = radius + 1.5 * scale;

    cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    cr.arc(x, y, ring, 0.0, PI * 2.0);
    let _ = cr.fill();

    cr.set_source_rgba(color.r, color.g, color.b, 1.0);
    cr.arc(x, y, radius, 0.0, PI * 2.0);
    let _ = cr.fill();

    cr.set_source_rgba(0.0, 0.0, 0.0, 0.3);
    cr.set_line_width(1.0);
    cr.arc(x, y, ring, 0.0, PI * 2.0);
    let _ = cr.stroke();
}

// ===== Preview swatch + hex row ============================================

fn build_preview_row(ctx: &mut SectionCtx, body: &gtk4::Box) {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(4.0));
    row.set_margin_top(ctx.px(PREVIEW_GAP_TOP - BODY_SPACING));

    // Current-color swatch with the expand-arrow badge; opens the popup.
    let preview = sized_button(ctx.sz(PREVIEW_SIZE), ctx.sz(PREVIEW_SIZE));
    preview.add_css_class("swatch");
    preview.set_tooltip_text(Some("Click to pick color"));
    let color_cell = Rc::new(Cell::new(color_key(ctx.snapshot.color)));
    let preview_area = gtk4::DrawingArea::new();
    preview_area.set_content_width(ctx.px(PREVIEW_SIZE));
    preview_area.set_content_height(ctx.px(PREVIEW_SIZE));
    preview_area.set_can_target(false);
    let scale = ctx.scale;
    let draw_color = color_cell.clone();
    preview_area.set_draw_func(move |_, cr, width, height| {
        let size = f64::from(width.min(height));
        let (r, g, b) = draw_color.get();
        draw_preview_swatch(cr, size, r, g, b, scale);
    });
    preview.set_child(Some(&preview_area));
    let sender = ctx.feedback.clone();
    preview.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::OpenColorPickerPopup);
    });
    row.append(&preview);
    ctx.updaters.push(Box::new(move |snapshot| {
        let key = color_key(snapshot.color);
        if color_cell.get() != key {
            color_cell.set(key);
            preview_area.queue_draw();
        }
    }));

    // Hex entry: Enter applies, invalid input is ignored. Snapshot updates
    // never overwrite the text while the user is editing.
    let entry = gtk4::Entry::new();
    entry.set_text(&color_to_hex(ctx.snapshot.color));
    entry.set_width_chars(7);
    entry.set_max_length(7);
    gtk4::prelude::EditableExt::set_alignment(&entry, 0.5);
    entry.set_size_request(ctx.px(HEX_INPUT_WIDTH), ctx.px(HEX_INPUT_HEIGHT));
    entry.set_valign(gtk4::Align::Center);
    entry.set_margin_start(ctx.px(4.0));
    entry.set_tooltip_text(Some("Type a hex color (Enter applies)"));
    let sender = ctx.feedback.clone();
    entry.connect_activate(move |entry| {
        if let Some(color) = parse_hex_color(&entry.text()) {
            send_event(&sender, ToolbarEvent::SetColor(color));
        }
    });
    row.append(&entry);
    let entry_handle = entry.clone();
    ctx.updaters.push(Box::new(move |snapshot| {
        if entry_handle.has_focus()
            || entry_handle
                .state_flags()
                .contains(gtk4::StateFlags::FOCUS_WITHIN)
        {
            return;
        }
        let hex = color_to_hex(snapshot.color);
        if entry_handle.text() != hex {
            entry_handle.set_text(&hex);
        }
    }));

    let copy = icon_button(
        toolbar_icons::draw_icon_copy,
        (ctx.sz(HEX_INPUT_HEIGHT), ctx.sz(HEX_INPUT_HEIGHT)),
        ctx.sz(10.0),
        "Copy hex color",
    );
    let sender = ctx.feedback.clone();
    copy.button.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::CopyHexColor);
    });
    row.append(&copy.button);

    let paste = icon_button(
        toolbar_icons::draw_icon_paste,
        (ctx.sz(HEX_INPUT_HEIGHT), ctx.sz(HEX_INPUT_HEIGHT)),
        ctx.sz(12.0),
        "Paste hex color from clipboard",
    );
    let sender = ctx.feedback.clone();
    paste.button.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::PasteHexColor);
    });
    row.append(&paste.button);

    body.append(&row);
}

fn color_key(color: Color) -> (f64, f64, f64) {
    (color.r, color.g, color.b)
}

/// Rounded-square preview like the built-in `draw_swatch`, drawn one pixel
/// inset so the resting outline stays inside the widget, with the popup
/// expand-arrow badge in the bottom-right corner.
fn draw_preview_swatch(cr: &cairo::Context, size: f64, r: f64, g: f64, b: f64, scale: f64) {
    cr.set_source_rgba(0.5, 0.55, 0.6, 0.6);
    cr.set_line_width(1.0);
    rounded_rect_path(cr, 0.5, 0.5, size - 1.0, size - 1.0, 5.0);
    let _ = cr.stroke();

    cr.set_source_rgba(r, g, b, 1.0);
    rounded_rect_path(cr, 1.5, 1.5, size - 3.0, size - 3.0, 4.0);
    let _ = cr.fill();

    let luminance = 0.299 * r + 0.587 * g + 0.114 * b;
    if luminance < 0.3 {
        cr.set_source_rgba(0.5, 0.5, 0.5, 0.8);
        cr.set_line_width(1.5);
        rounded_rect_path(cr, 1.5, 1.5, size - 3.0, size - 3.0, 4.0);
        let _ = cr.stroke();
    }

    let icon = EXPAND_ICON_SIZE * scale;
    let icon_x = size - icon - 2.0;
    let icon_y = size - icon - 2.0;
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.4);
    cr.arc(
        icon_x + icon / 2.0,
        icon_y + icon / 2.0,
        icon / 2.0 + 1.0,
        0.0,
        PI * 2.0,
    );
    let _ = cr.fill();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    cr.set_line_width(1.2);
    cr.set_line_cap(cairo::LineCap::Round);
    let margin = icon * 0.2;
    let arrow_x1 = icon_x + margin;
    let arrow_y1 = icon_y + icon - margin;
    let arrow_x2 = icon_x + icon - margin;
    let arrow_y2 = icon_y + margin;
    cr.move_to(arrow_x1, arrow_y1);
    cr.line_to(arrow_x2, arrow_y2);
    let _ = cr.stroke();
    let head = icon * 0.3;
    cr.move_to(arrow_x2 - head, arrow_y2);
    cr.line_to(arrow_x2, arrow_y2);
    cr.line_to(arrow_x2, arrow_y2 + head);
    let _ = cr.stroke();
}

// ===== Quick-color swatch rows =============================================

fn build_swatch_rows(ctx: &mut SectionCtx, body: &gtk4::Box) {
    let snapshot = ctx.snapshot;
    let compact = compact_palette_swatches(&snapshot.quick_colors);
    let expanded = expanded_palette_swatches(&snapshot.quick_colors);

    let column = gtk4::Box::new(gtk4::Orientation::Vertical, ctx.px(SWATCH_GAP));
    column.set_margin_top(ctx.px(PREVIEW_GAP_BOTTOM - BODY_SPACING));
    let mut tracked: Vec<(Color, Rc<Cell<bool>>, gtk4::DrawingArea)> = Vec::new();

    let compact_toggle: Option<(ToolbarEvent, &'static str, IconPainter)> =
        if snapshot.show_more_colors || expanded.is_empty() {
            None
        } else {
            Some((
                ToolbarEvent::ToggleMoreColors(true),
                "More colors",
                toolbar_icons::draw_icon_plus,
            ))
        };
    column.append(&swatch_row(ctx, &compact, compact_toggle, &mut tracked));

    if snapshot.show_more_colors {
        let rows = expanded.chunks(SWATCHES_PER_ROW).collect::<Vec<_>>();
        let row_count = rows.len();
        for (row_index, row_colors) in rows.iter().enumerate() {
            let toggle = (row_index + 1 == row_count).then_some((
                ToolbarEvent::ToggleMoreColors(false),
                "Hide colors",
                toolbar_icons::draw_icon_minus as IconPainter,
            ));
            column.append(&swatch_row(ctx, row_colors, toggle, &mut tracked));
        }
    }
    body.append(&column);

    ctx.updaters.push(Box::new(move |snapshot| {
        for (color, selected, area) in &tracked {
            let now = *color == snapshot.color;
            if selected.get() != now {
                selected.set(now);
                area.queue_draw();
            }
        }
    }));
}

fn swatch_row(
    ctx: &SectionCtx,
    colors: &[ColorSwatch],
    toggle: Option<(ToolbarEvent, &'static str, IconPainter)>,
    tracked: &mut Vec<(Color, Rc<Cell<bool>>, gtk4::DrawingArea)>,
) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(SWATCH_GAP));
    for (color, name, action) in colors {
        let binding =
            action.and_then(|action| ctx.snapshot.binding_hints.binding_for_action(action));
        let tooltip = format_binding_label(name, binding);
        let color = *color;
        let (button, selected, area) =
            rect_swatch(ctx, color, color == ctx.snapshot.color, &tooltip);
        let sender = ctx.feedback.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::SetColor(color));
        });
        row.append(&button);
        tracked.push((color, selected, area));
    }

    if let Some((event, tooltip, painter)) = toggle {
        let toggle_button = icon_button(
            painter,
            (ctx.sz(SWATCH), ctx.sz(SWATCH)),
            ctx.sz(14.0),
            tooltip,
        );
        let sender = ctx.feedback.clone();
        toggle_button.button.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        row.append(&toggle_button.button);
    }
    row
}

/// Rounded-square quick-color swatch mirroring the built-in `draw_swatch`:
/// fill, a gray outline for low-luminance colors, and the white selection
/// ring (drawn inset because the built-in paints it outside the bounds).
fn rect_swatch(
    ctx: &SectionCtx,
    color: Color,
    selected: bool,
    tooltip: &str,
) -> (gtk4::Button, Rc<Cell<bool>>, gtk4::DrawingArea) {
    let button = sized_button(ctx.sz(SWATCH), ctx.sz(SWATCH));
    button.add_css_class("swatch");
    button.set_tooltip_text(Some(tooltip));
    let selected_cell = Rc::new(Cell::new(selected));
    let area = gtk4::DrawingArea::new();
    area.set_content_width(ctx.px(SWATCH));
    area.set_content_height(ctx.px(SWATCH));
    area.set_can_target(false);
    let draw_selected = selected_cell.clone();
    area.set_draw_func(move |_, cr, width, height| {
        let size = f64::from(width.min(height));
        cr.set_source_rgba(color.r, color.g, color.b, 1.0);
        rounded_rect_path(cr, 3.0, 3.0, size - 6.0, size - 6.0, 4.0);
        let _ = cr.fill();

        let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
        if luminance < 0.3 {
            cr.set_source_rgba(0.5, 0.5, 0.5, 0.8);
            cr.set_line_width(1.5);
            rounded_rect_path(cr, 3.0, 3.0, size - 6.0, size - 6.0, 4.0);
            let _ = cr.stroke();
        }

        if draw_selected.get() {
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
            cr.set_line_width(2.0);
            rounded_rect_path(cr, 1.0, 1.0, size - 2.0, size - 2.0, 5.0);
            let _ = cr.stroke();
        }
    });
    button.set_child(Some(&area));
    (button, selected_cell, area)
}

// ===== Palette selection (ported from the built-in colors section) ========

/// Legacy one-click colors keep their compact-row spots (Red, Green, Blue,
/// Yellow, White, Black in the default palette).
const COMPACT_PALETTE_INDICES: [usize; SWATCHES_PER_ROW] = [0, 1, 2, 3, 6, 7];

fn compact_palette_swatches(palette: &QuickColorPalette) -> Vec<ColorSwatch> {
    let entries = palette.rendered_entries();
    compact_palette_indices(palette)
        .into_iter()
        .filter_map(|index| {
            entries
                .get(index)
                .map(|entry| palette_swatch((index, entry)))
        })
        .collect()
}

fn compact_palette_indices(palette: &QuickColorPalette) -> Vec<usize> {
    let mut indices = Vec::with_capacity(SWATCHES_PER_ROW);
    let rendered_len = palette.rendered_len();
    for index in COMPACT_PALETTE_INDICES {
        if index < rendered_len {
            indices.push(index);
        }
    }

    if indices.len() < SWATCHES_PER_ROW {
        for index in 0..rendered_len {
            if indices.contains(&index) {
                continue;
            }
            indices.push(index);
            if indices.len() == SWATCHES_PER_ROW {
                break;
            }
        }
    }

    indices
}

fn expanded_palette_swatches(palette: &QuickColorPalette) -> Vec<ColorSwatch> {
    let compact_indices = compact_palette_indices(palette);
    palette
        .rendered_entries()
        .iter()
        .enumerate()
        .filter(|(index, _)| !compact_indices.contains(index))
        .map(palette_swatch)
        .collect()
}

fn palette_swatch((index, entry): (usize, &QuickColorPaletteEntry)) -> ColorSwatch {
    (
        entry.color,
        entry.label.clone(),
        QuickColorPalette::action_for_index(index),
    )
}
