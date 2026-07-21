//! Presets section: numbered slots that apply, save, or clear tool presets.
//!
//! Filled slots redraw the built-in slot face (color tint, keycap, tool
//! icon, thickness preview, color swatch) in a `DrawingArea`; the clear
//! badge is revealed through an `EventControllerMotion` because GTK CSS
//! `:hover` cannot toggle a sibling's visibility. Slot filled/empty shape
//! is structural (rebuild); preset content, the active highlight, and the
//! transient feedback flash flow through updaters.

/// Feedback flash tint, RGBA; `None` while no preset feedback is active.
type FeedbackTint = std::rc::Rc<std::cell::Cell<Option<(f64, f64, f64, f64)>>>;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;

use crate::config::action_label;
use crate::draw::EraserKind;
use crate::input::EraserMode;
use crate::input::state::PresetFeedbackKind;
use crate::label_format::format_binding_label;
use crate::toolbar_icons;
use crate::ui::theme::toolbar::{
    COLOR_CARD_BACKGROUND, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY, FONT_FAMILY_DEFAULT,
    FONT_SIZE_SECONDARY,
};
use crate::ui::theme::{Rgb, Rgba, set_color, set_color_alpha, with_alpha};
use crate::ui::toolbar::bindings::{action_for_clear_preset, action_for_save_preset, tool_label};
use crate::ui::toolbar::{PresetSlotSnapshot, ToolbarEvent, ToolbarSideSection, ToolbarSnapshot};
use crate::ui_text::{UiTextStyle, text_layout};
use crate::util::color_to_name;

use super::super::super::icons::{IconPainter, tool_icon_painter};
use super::super::super::widgets::{
    COLOR_SWATCH_HAIRLINE_DARK, rounded_rect_path, send_event, set_active_class, sized_button,
};
use super::{SectionCard, SectionCtx, section_card};

/// Built-in slot geometry (spec units, multiplied by the toolbar scale).
const SLOT_SIZE: f64 = 40.0;
const SLOT_GAP: f64 = 8.0;
/// Size of the hover-revealed clear (✕) badge in a filled slot's corner.
const CLEAR_BADGE_SIZE: f64 = 14.0;

// Slot chrome tints mirroring the built-in `side_palette::presets::slot`;
// specific values with no theme token, kept to avoid a visible shift.
// TODO(theme-consolidation): every const below is a name-for-name copy of a
// built-in side_palette const — hoist the pairs into `theme::toolbar` so the
// two frontends cannot drift.
/// Empty slot well: near-black tint one step darker than
/// COLOR_PANEL_BACKGROUND (alpha varies with hover).
const EMPTY_SLOT_BG_RGB: Rgb = (0.05, 0.05, 0.07);
/// Dashed outline marking an empty slot.
const COLOR_EMPTY_SLOT_DASH: Rgba = (1.0, 1.0, 1.0, 0.35);
/// Dim save-hint plus glyph in an idle empty slot.
const COLOR_EMPTY_SLOT_PLUS: Rgba = (1.0, 1.0, 1.0, 0.45);
/// Clear badge fill: muted destructive red, quieter than the destructive
/// root. TODO(theme-consolidation): near COLOR_CLOSE_HOVER.
const COLOR_CLEAR_BADGE: Rgba = (0.75, 0.2, 0.2, 0.9);
/// Thickness preview line across a filled slot's bottom edge.
const COLOR_PREVIEW_LINE: Rgba = (1.0, 1.0, 1.0, 0.8);
/// Outline around the mini color swatch in a filled slot.
const COLOR_SWATCH_OUTLINE: Rgba = (1.0, 1.0, 1.0, 0.75);
/// Preset color tint over a filled slot's face (fill and outline alphas).
const PRESET_TINT_FILL_ALPHA: f64 = 0.12;
const PRESET_TINT_BORDER_ALPHA: f64 = 0.35;
/// White root for the keycap's border/text alpha ladder.
const WHITE_RGB: Rgb = (1.0, 1.0, 1.0);
/// Per-kind feedback flash tints (apply/save/clear); mirror the built-in
/// `presets::slot::feedback`. TODO(theme-consolidation): near the overlay
/// toast palette but not equal.
const FEEDBACK_APPLY_RGB: Rgb = (0.35, 0.55, 0.95);
const FEEDBACK_SAVE_RGB: Rgb = (0.25, 0.75, 0.4);
const FEEDBACK_CLEAR_RGB: Rgb = (0.9, 0.3, 0.3);

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let snapshot = ctx.snapshot;
    let slot_count = snapshot.preset_slot_count.min(snapshot.presets.len());
    if !snapshot.show_presets || slot_count == 0 {
        return None;
    }

    let card = section_card(
        ctx,
        ToolbarSideSection::Presets,
        ToolbarSideSection::Presets.label(),
    );
    attach_header_hint(ctx, &card, slot_count);

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(SLOT_GAP));
    for slot_index in 0..slot_count {
        let preset = snapshot
            .presets
            .get(slot_index)
            .and_then(|slot| slot.as_ref());
        let widget = match preset {
            Some(preset) => filled_slot(ctx, slot_index, preset),
            None => empty_slot(ctx, slot_index),
        };
        row.append(&widget);
    }
    card.body.append(&row);
    Some(card.root.upcast())
}

/// Right-align the built-in "Keys 1-N apply" hint inside the card header
/// (next to the collapse chevron, visible while collapsed); falls back to
/// the body if the header shape ever changes.
fn attach_header_hint(ctx: &SectionCtx, card: &SectionCard, slot_count: usize) {
    let hint = gtk4::Label::new(Some(&apply_hint_text(ctx.snapshot, slot_count)));
    hint.add_css_class("hint");
    hint.set_xalign(1.0);
    let header_row = card
        .root
        .first_child()
        .and_then(|header| header.downcast::<gtk4::Button>().ok())
        .and_then(|header| header.child())
        .and_then(|row| row.downcast::<gtk4::Box>().ok());
    match header_row {
        Some(row) => row.insert_child_after(&hint, row.first_child().as_ref()),
        None => card.body.prepend(&hint),
    }
}

/// "Keys 1-N apply" when every slot keeps its default digit binding,
/// otherwise the generic reminder — mirroring the built-in header.
fn apply_hint_text(snapshot: &ToolbarSnapshot, slot_count: usize) -> String {
    let uses_digit_bindings = (1..=slot_count)
        .all(|slot| snapshot.binding_hints.apply_preset(slot) == Some(slot.to_string().as_str()));
    if uses_digit_bindings {
        format!("Keys 1-{slot_count} apply")
    } else {
        "Keys apply presets".to_string()
    }
}

/// Filled slot: applies the preset on click; hover reveals the clear badge.
fn filled_slot(
    ctx: &mut SectionCtx,
    slot_index: usize,
    preset: &PresetSlotSnapshot,
) -> gtk4::Widget {
    let slot = slot_index + 1;
    let size = ctx.sz(SLOT_SIZE);
    let button = sized_button(size, size);
    button.set_tooltip_text(Some(&preset_tooltip_text(
        preset,
        slot,
        ctx.snapshot.binding_hints.apply_preset(slot),
    )));
    let face = FilledFace::new(preset, slot, size);
    button.set_child(Some(&face.area));
    let sender = ctx.feedback.clone();
    button.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::ApplyPreset(slot));
    });

    let overlay = gtk4::Overlay::new();
    overlay.set_child(Some(&button));
    let badge = clear_badge(ctx, slot);
    overlay.add_overlay(&badge);
    let motion = gtk4::EventControllerMotion::new();
    let enter_badge = badge.clone();
    motion.connect_enter(move |_, _, _| enter_badge.set_visible(true));
    let leave_badge = badge.clone();
    motion.connect_leave(move |_| leave_badge.set_visible(false));
    overlay.add_controller(motion);

    // Content, tooltip, active highlight, and feedback flash track later
    // snapshots (filled/empty flips are structural and rebuild instead).
    let last = RefCell::new(preset.clone());
    let handle = button.clone();
    ctx.updaters.push(Box::new(move |snapshot| {
        set_active_class(&handle, snapshot.active_preset_slot == Some(slot));
        if let Some(Some(current)) = snapshot.presets.get(slot_index)
            && *last.borrow() != *current
        {
            face.apply(current);
            handle.set_tooltip_text(Some(&preset_tooltip_text(
                current,
                slot,
                snapshot.binding_hints.apply_preset(slot),
            )));
            *last.borrow_mut() = current.clone();
        }
        face.set_feedback(feedback_overlay(snapshot, slot_index));
    }));

    overlay.upcast()
}

/// Empty slot: dashed outline with a "+", saves the current setup on click.
fn empty_slot(ctx: &mut SectionCtx, slot_index: usize) -> gtk4::Widget {
    let slot = slot_index + 1;
    let size = ctx.sz(SLOT_SIZE);
    let button = sized_button(size, size);
    button.set_tooltip_text(Some(&format_binding_label(
        action_for_save_preset(slot)
            .map(action_label)
            .unwrap_or("Save Preset"),
        ctx.snapshot.binding_hints.save_preset(slot),
    )));

    let hovered = Rc::new(Cell::new(false));
    let feedback: FeedbackTint = Rc::new(Cell::new(None));
    let area = slot_area(size);
    let label = slot.to_string();
    let draw_hovered = hovered.clone();
    let draw_feedback_cell = feedback.clone();
    area.set_draw_func(move |_, ctx, width, height| {
        let s = width.min(height) as f64;
        let k = s / SLOT_SIZE;
        let hover = draw_hovered.get();
        // Dark backing + dashed outline the built-in empty slot draws.
        set_color_alpha(ctx, EMPTY_SLOT_BG_RGB, if hover { 0.45 } else { 0.35 });
        rounded_rect_path(ctx, k, k, s - 2.0 * k, s - 2.0 * k, 6.0 * k);
        let _ = ctx.fill();
        set_color(ctx, COLOR_EMPTY_SLOT_DASH);
        ctx.set_line_width(1.0);
        ctx.set_dash(&[3.0 * k, 2.0 * k], 0.0);
        rounded_rect_path(ctx, k, k, s - 2.0 * k, s - 2.0 * k, 6.0 * k);
        let _ = ctx.stroke();
        ctx.set_dash(&[], 0.0);
        let plus = (s * 0.45).round().min(14.0 * k);
        set_color(
            ctx,
            if hover {
                COLOR_TEXT_SECONDARY
            } else {
                COLOR_EMPTY_SLOT_PLUS
            },
        );
        toolbar_icons::draw_icon_plus(ctx, (s - plus) / 2.0, (s - plus) / 2.0, plus);
        draw_keycap(ctx, s, &label, false);
        draw_feedback(ctx, s, draw_feedback_cell.get());
    });
    button.set_child(Some(&area));

    let motion = gtk4::EventControllerMotion::new();
    let enter_hovered = hovered.clone();
    let enter_area = area.clone();
    motion.connect_enter(move |_, _, _| {
        enter_hovered.set(true);
        enter_area.queue_draw();
    });
    let leave_area = area.clone();
    motion.connect_leave(move |_| {
        hovered.set(false);
        leave_area.queue_draw();
    });
    button.add_controller(motion);

    let sender = ctx.feedback.clone();
    button.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::SavePreset(slot));
    });

    ctx.updaters.push(Box::new(move |snapshot| {
        let overlay = feedback_overlay(snapshot, slot_index);
        if feedback.get() != overlay {
            feedback.set(overlay);
            area.queue_draw();
        }
    }));

    button.upcast()
}

/// The destructive clear badge: a red circle with a white ✕, exactly like
/// the built-in hover badge, delivered as a corner overlay button.
fn clear_badge(ctx: &SectionCtx, slot: usize) -> gtk4::Button {
    let size = ctx.sz(CLEAR_BADGE_SIZE);
    let badge = sized_button(size, size);
    badge.add_css_class("swatch");
    badge.set_halign(gtk4::Align::End);
    badge.set_valign(gtk4::Align::Start);
    badge.set_margin_top(ctx.px(2.0));
    badge.set_margin_end(ctx.px(2.0));
    badge.set_visible(false);
    badge.set_tooltip_text(Some(&format_binding_label(
        action_for_clear_preset(slot)
            .map(action_label)
            .unwrap_or("Clear Preset"),
        ctx.snapshot.binding_hints.clear_preset(slot),
    )));
    let area = slot_area(size);
    area.set_draw_func(|_, ctx, width, height| {
        let size = width.min(height) as f64;
        set_color(ctx, COLOR_CLEAR_BADGE);
        ctx.arc(
            size / 2.0,
            size / 2.0,
            size / 2.0,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        let _ = ctx.fill();
        set_color(ctx, COLOR_TEXT_PRIMARY);
        let inset = size * 0.3;
        ctx.set_line_width(1.6 * size / CLEAR_BADGE_SIZE);
        ctx.set_line_cap(cairo::LineCap::Round);
        ctx.move_to(inset, inset);
        ctx.line_to(size - inset, size - inset);
        let _ = ctx.stroke();
        ctx.move_to(size - inset, inset);
        ctx.line_to(inset, size - inset);
        let _ = ctx.stroke();
    });
    badge.set_child(Some(&area));
    let sender = ctx.feedback.clone();
    badge.connect_clicked(move |_| {
        send_event(&sender, ToolbarEvent::ClearPreset(slot));
    });
    badge
}

/// A filled slot's drawn face; content-only preset edits update the cells
/// and redraw instead of rebuilding the widget tree.
struct FilledFace {
    area: gtk4::DrawingArea,
    color: Rc<Cell<(f64, f64, f64)>>,
    thickness: Rc<Cell<f64>>,
    painter: Rc<Cell<IconPainter>>,
    feedback: FeedbackTint,
}

impl FilledFace {
    fn new(preset: &PresetSlotSnapshot, slot: usize, size: f64) -> Self {
        let area = slot_area(size);
        let color = Rc::new(Cell::new((preset.color.r, preset.color.g, preset.color.b)));
        let thickness = Rc::new(Cell::new(preset.size));
        let painter = Rc::new(Cell::new(tool_icon_painter(preset.tool)));
        let feedback: FeedbackTint = Rc::new(Cell::new(None));
        let label = slot.to_string();
        let draw_color = color.clone();
        let draw_thickness = thickness.clone();
        let draw_painter = painter.clone();
        let draw_feedback_cell = feedback.clone();
        area.set_draw_func(move |_, ctx, width, height| {
            let s = width.min(height) as f64;
            let k = s / SLOT_SIZE;
            let (r, g, b) = draw_color.get();
            // Preset color tint over the button base.
            set_color_alpha(ctx, (r, g, b), PRESET_TINT_FILL_ALPHA);
            rounded_rect_path(ctx, k, k, s - 2.0 * k, s - 2.0 * k, 6.0 * k);
            let _ = ctx.fill();
            set_color_alpha(ctx, (r, g, b), PRESET_TINT_BORDER_ALPHA);
            ctx.set_line_width(1.0);
            rounded_rect_path(ctx, k, k, s - 2.0 * k, s - 2.0 * k, 6.0 * k);
            let _ = ctx.stroke();
            // Centered tool icon.
            let icon = (s * 0.45).round();
            set_color(ctx, COLOR_TEXT_SECONDARY);
            (draw_painter.get())(ctx, (s - icon) / 2.0, (s - icon) / 2.0, icon);
            // Thickness preview line along the bottom.
            let preview = (draw_thickness.get() / 50.0 * 6.0).clamp(1.0, 6.0) * k;
            set_color(ctx, COLOR_PREVIEW_LINE);
            ctx.set_line_width(preview);
            ctx.move_to(4.0 * k, s - 6.0 * k);
            ctx.line_to(s - 4.0 * k, s - 6.0 * k);
            let _ = ctx.stroke();
            // Color swatch in the bottom-right corner.
            let sw = (s * 0.35).round();
            let (sx, sy) = (s - sw - 4.0 * k, s - sw - 4.0 * k);
            ctx.set_source_rgba(r, g, b, 1.0);
            rounded_rect_path(ctx, sx, sy, sw, sw, 4.0 * k);
            let _ = ctx.fill();
            let luminance = 0.299 * r + 0.587 * g + 0.114 * b;
            if luminance < 0.3 {
                set_color(ctx, COLOR_SWATCH_HAIRLINE_DARK);
                ctx.set_line_width(1.5);
                rounded_rect_path(ctx, sx, sy, sw, sw, 4.0 * k);
                let _ = ctx.stroke();
            }
            set_color(ctx, COLOR_SWATCH_OUTLINE);
            ctx.set_line_width(1.0);
            rounded_rect_path(ctx, sx, sy, sw, sw, 4.0 * k);
            let _ = ctx.stroke();
            draw_keycap(ctx, s, &label, true);
            draw_feedback(ctx, s, draw_feedback_cell.get());
        });
        Self {
            area,
            color,
            thickness,
            painter,
            feedback,
        }
    }

    fn apply(&self, preset: &PresetSlotSnapshot) {
        self.color
            .set((preset.color.r, preset.color.g, preset.color.b));
        self.thickness.set(preset.size);
        self.painter.set(tool_icon_painter(preset.tool));
        self.area.queue_draw();
    }

    fn set_feedback(&self, overlay: Option<(f64, f64, f64, f64)>) {
        if self.feedback.get() != overlay {
            self.feedback.set(overlay);
            self.area.queue_draw();
        }
    }
}

/// Square, click-transparent canvas for a slot face or badge glyph.
fn slot_area(size: f64) -> gtk4::DrawingArea {
    let area = gtk4::DrawingArea::new();
    let px = size.round() as i32;
    area.set_content_width(px);
    area.set_content_height(px);
    area.set_can_target(false);
    area
}

/// The numbered keycap in the slot's top-left corner (bright when the
/// slot holds a preset, dim when empty), matching the built-in alphas.
fn draw_keycap(ctx: &cairo::Context, s: f64, label: &str, active: bool) {
    let k = s / SLOT_SIZE;
    let number_box = (s * 0.4).round();
    let pad = (s * 0.1).round().max(3.0);
    let radius = (number_box * 0.25).max(3.0);
    let (bg_alpha, border_alpha, text_alpha) = if active {
        (0.65, 0.50, 0.95)
    } else {
        (0.30, 0.25, 0.55)
    };
    set_color(ctx, with_alpha(COLOR_CARD_BACKGROUND, bg_alpha));
    rounded_rect_path(ctx, pad, pad, number_box, number_box, radius);
    let _ = ctx.fill();
    set_color_alpha(ctx, WHITE_RGB, border_alpha);
    ctx.set_line_width(1.0);
    rounded_rect_path(ctx, pad, pad, number_box, number_box, radius);
    let _ = ctx.stroke();
    let style = UiTextStyle {
        family: FONT_FAMILY_DEFAULT,
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: FONT_SIZE_SECONDARY * k,
    };
    let layout = text_layout(ctx, style, label, None);
    let ext = layout.ink_extents();
    let tx = pad + (number_box - ext.width()) / 2.0 - ext.x_bearing();
    let ty = pad + (number_box - ext.height()) / 2.0 - ext.y_bearing();
    set_color_alpha(ctx, WHITE_RGB, text_alpha);
    layout.show_at_baseline(ctx, tx, ty);
}

/// The transient apply/save/clear flash the built-in palette fades out.
fn draw_feedback(ctx: &cairo::Context, s: f64, overlay: Option<(f64, f64, f64, f64)>) {
    if let Some(tint) = overlay {
        let k = s / SLOT_SIZE;
        set_color(ctx, tint);
        rounded_rect_path(ctx, k, k, s - 2.0 * k, s - 2.0 * k, 6.0 * k);
        let _ = ctx.fill();
    }
}

fn feedback_overlay(snapshot: &ToolbarSnapshot, slot_index: usize) -> Option<(f64, f64, f64, f64)> {
    let feedback = snapshot.preset_feedback.get(slot_index)?.as_ref()?;
    let fade = (1.0 - feedback.progress as f64).clamp(0.0, 1.0);
    if fade <= 0.0 {
        return None;
    }
    let (r, g, b) = match feedback.kind {
        PresetFeedbackKind::Apply => FEEDBACK_APPLY_RGB,
        PresetFeedbackKind::Save => FEEDBACK_SAVE_RGB,
        PresetFeedbackKind::Clear => FEEDBACK_CLEAR_RGB,
    };
    Some((r, g, b, 0.35 * fade))
}

// ===== Tooltip text (replicates the built-in presets/format.rs) =========

fn preset_tooltip_text(preset: &PresetSlotSnapshot, slot: usize, binding: Option<&str>) -> String {
    let preset_name = preset
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let mut extra_details = Vec::new();
    if let Some(fill) = preset.fill_enabled {
        extra_details.push(format!("fill:{}", on_off(fill)));
    }
    if let Some(opacity) = preset.marker_opacity {
        let percent = (opacity * 100.0).round() as i32;
        extra_details.push(format!("opacity:{}%", percent));
    }
    if let Some(kind) = preset.eraser_kind {
        extra_details.push(format!("eraser:{}", eraser_kind_label(kind)));
    }
    if let Some(mode) = preset.eraser_mode {
        extra_details.push(format!("mode:{}", eraser_mode_label(mode)));
    }
    if let Some(font_size) = preset.font_size {
        extra_details.push(format!("font:{}", px_label(font_size)));
    }
    if let Some(text_bg) = preset.text_background_enabled {
        extra_details.push(format!("text bg:{}", on_off(text_bg)));
    }
    let mut arrow_bits = Vec::new();
    if let Some(length) = preset.arrow_length {
        arrow_bits.push(format!("len {}", px_label(length)));
    }
    if let Some(angle) = preset.arrow_angle {
        arrow_bits.push(format!("ang {}", angle_label(angle)));
    }
    if let Some(head_at_end) = preset.arrow_head_at_end {
        let head = if head_at_end { "end" } else { "start" };
        arrow_bits.push(format!("head {}", head));
    }
    if !arrow_bits.is_empty() {
        extra_details.push(format!("arrow:{}", arrow_bits.join(", ")));
    }
    if let Some(show_status_bar) = preset.show_status_bar {
        extra_details.push(format!("status:{}", on_off(show_status_bar)));
    }

    let base_summary = format!(
        "{}, {}, {}",
        tool_label(preset.tool),
        color_to_name(&preset.color),
        px_label(preset.size)
    );
    let summary = if extra_details.is_empty() {
        base_summary
    } else {
        format!("{}; {}", base_summary, extra_details.join("; "))
    };
    let label = if let Some(name) = preset_name {
        format!("Apply preset {}: {} ({})", slot, name, summary)
    } else {
        format!("Apply preset {} ({})", slot, summary)
    };
    match binding {
        Some(binding) => format!("{label} (key: {binding})"),
        None => label,
    }
}

fn px_label(value: f64) -> String {
    if (value - value.round()).abs() < 0.05 {
        format!("{:.0}px", value)
    } else {
        format!("{:.1}px", value)
    }
}

fn angle_label(value: f64) -> String {
    if (value - value.round()).abs() < 0.05 {
        format!("{:.0}deg", value)
    } else {
        format!("{:.1}deg", value)
    }
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

fn eraser_kind_label(kind: EraserKind) -> &'static str {
    match kind {
        EraserKind::Circle => "circle",
        EraserKind::Rect => "rect",
    }
}

fn eraser_mode_label(mode: EraserMode) -> &'static str {
    match mode {
        EraserMode::Brush => "brush",
        EraserMode::Stroke => "stroke",
    }
}
