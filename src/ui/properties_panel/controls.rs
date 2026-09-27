//! Painters for the properties panel's controls. Each takes the rectangles
//! the panel geometry laid out, so drawing never re-derives a position.

use std::f64::consts::{PI, TAU};

use cairo::FontWeight;

use crate::draw::{ArrowStyle, Color};
use crate::input::state::properties_panel_metrics::{
    CAPTION_FONT, SEGMENT_ICON_GAP, SEGMENT_ICON_WIDTH, SLIDER_THUMB_RADIUS, SLIDER_TRACK_HEIGHT,
    VALUE_FONT, text_style,
};
use crate::input::state::{
    LevelRange, PanelRect, PropertiesPanelHit, PropertiesPanelLock, PropertiesPanelSwatch,
};
use crate::toolbar_icons::draw_arrow_style_preview;
use crate::ui::primitives::{checkerboard_behind, draw_rounded_rect};
use crate::ui::theme::overlay::{
    ACCENT_BRIGHT, ACCENT_PRIMARY, BG_HOVER, BG_HOVER_WASH, PANEL_BG_PROPERTIES, RADIUS_MD,
    RADIUS_SM, RADIUS_STD, TEXT_DISABLED, TEXT_HINT, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY,
    TEXT_WHITE,
};
use crate::ui::theme::swatch::{chrome_rgb, swatch_edge_stroke};
use crate::ui::theme::{Rgba, set_color, with_alpha};
use crate::ui_text::UiTextEngine;

/// A switch's track while off: a faint well on the panel.
const SWITCH_TRACK_OFF: Rgba = (1.0, 1.0, 1.0, 0.14);
/// Hairline around a swatch that already contrasts with the panel.
const SWATCH_HAIRLINE: Rgba = (1.0, 1.0, 1.0, 0.18);
/// Outline of the "more colors" button and the style buttons at rest.
const QUIET_OUTLINE: Rgba = (1.0, 1.0, 1.0, 0.16);
/// A locked selection's padlock: the same amber the toolbar uses for "held".
const LOCK_ACTIVE: Rgba = (0.965, 0.827, 0.176, 1.0);
/// The stroke across the "no fill" swatch: the destructive red, as "none".
const NO_FILL_STRIKE: Rgba = crate::ui::theme::rgba(crate::ui::theme::DESTRUCTIVE_RGB, 0.9);
/// How much of its color a control keeps while its row is locked.
const DISABLED_ALPHA: f64 = 0.35;
const GLYPH_WIDTH: f64 = 1.6;

/// What a control needs to know about its row this frame.
#[derive(Clone, Copy)]
pub(super) struct ControlState {
    pub(super) enabled: bool,
    /// The row's part under the pointer, if any.
    pub(super) hover: Option<PropertiesPanelHit>,
}

impl ControlState {
    fn hovers(&self, matches: impl Fn(PropertiesPanelHit) -> bool) -> bool {
        self.enabled && self.hover.is_some_and(matches)
    }

    fn fade(&self, color: Rgba) -> Rgba {
        if self.enabled {
            color
        } else {
            with_alpha(color, color.3 * DISABLED_ALPHA)
        }
    }
}

fn rgba(color: Color) -> Rgba {
    (color.r, color.g, color.b, color.a)
}

fn round_line(ctx: &cairo::Context, from: (f64, f64), to: (f64, f64)) {
    ctx.set_line_width(GLYPH_WIDTH);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.move_to(from.0, from.1);
    ctx.line_to(to.0, to.1);
    let _ = ctx.stroke();
}

fn minus_glyph(ctx: &cairo::Context, (cx, cy): (f64, f64)) {
    round_line(ctx, (cx - 4.0, cy), (cx + 4.0, cy));
}

fn plus_glyph(ctx: &cairo::Context, (cx, cy): (f64, f64)) {
    minus_glyph(ctx, (cx, cy));
    round_line(ctx, (cx, cy - 4.0), (cx, cy + 4.0));
}

/// A short arrow for the Start/End segments, pointing right when `right`.
fn arrow_glyph(ctx: &cairo::Context, x: f64, cy: f64, right: bool) {
    let (tail, tip) = if right {
        (x, x + SEGMENT_ICON_WIDTH)
    } else {
        (x + SEGMENT_ICON_WIDTH, x)
    };
    let back = if right { tip - 3.5 } else { tip + 3.5 };
    round_line(ctx, (tail, cy), (tip, cy));
    ctx.move_to(back, cy - 3.5);
    ctx.line_to(tip, cy);
    ctx.line_to(back, cy + 3.5);
    let _ = ctx.stroke();
}

pub(super) fn draw_lock(
    ctx: &cairo::Context,
    rect: PanelRect,
    lock: PropertiesPanelLock,
    hovered: bool,
) {
    if hovered {
        set_color(ctx, BG_HOVER);
        draw_rounded_rect(ctx, rect.x, rect.y, rect.width, rect.height, RADIUS_STD);
        let _ = ctx.fill();
    }
    let color = match lock {
        PropertiesPanelLock::Locked => LOCK_ACTIVE,
        PropertiesPanelLock::Partial => TEXT_HINT,
        PropertiesPanelLock::Unlocked if hovered => TEXT_PRIMARY,
        PropertiesPanelLock::Unlocked => TEXT_TERTIARY,
    };
    let (cx, cy) = rect.center();
    set_color(ctx, color);
    ctx.set_line_width(GLYPH_WIDTH);
    ctx.set_line_cap(cairo::LineCap::Round);
    draw_rounded_rect(ctx, cx - 5.0, cy - 0.5, 10.0, 7.0, 1.5);
    let _ = ctx.stroke();
    ctx.new_path();
    ctx.move_to(cx - 2.6, cy - 0.5);
    ctx.line_to(cx - 2.6, cy - 3.0);
    if lock == PropertiesPanelLock::Unlocked {
        // Open: the shackle swings up and stops short of the body.
        ctx.arc(cx, cy - 3.0, 2.6, PI, PI * 1.85);
    } else {
        ctx.arc(cx, cy - 3.0, 2.6, PI, TAU);
        ctx.line_to(cx + 2.6, cy - 0.5);
    }
    let _ = ctx.stroke();
}

/// Where a swatch row's extra cells sit, and whether "no fill" is the
/// current choice.
#[derive(Clone, Copy)]
pub(super) struct SwatchExtras {
    pub(super) none: Option<PanelRect>,
    pub(super) none_selected: bool,
    pub(super) more: Option<PanelRect>,
}

fn selection_ring(ctx: &cairo::Context, rect: PanelRect, state: ControlState) {
    let (cx, cy) = rect.center();
    ctx.new_path();
    ctx.arc(cx, cy, rect.width / 2.0 + 3.5, 0.0, TAU);
    set_color(ctx, state.fade(ACCENT_BRIGHT));
    ctx.set_line_width(2.0);
    let _ = ctx.stroke();
}

pub(super) fn draw_swatches(
    ctx: &cairo::Context,
    swatches: &[PropertiesPanelSwatch],
    rects: &[PanelRect],
    extras: SwatchExtras,
    current: Option<usize>,
    state: ControlState,
) {
    let background = chrome_rgb(PANEL_BG_PROPERTIES);
    for (index, (swatch, rect)) in swatches.iter().zip(rects).enumerate() {
        let (cx, cy) = rect.center();
        let hovered = state.hovers(|hit| {
            matches!(hit, PropertiesPanelHit::Swatch { index: hovered, .. } if hovered == index)
        });
        let radius = rect.width / 2.0 + if hovered { 1.0 } else { 0.0 };
        let fill = state.fade(rgba(swatch.color));
        let circle = |ctx: &cairo::Context| {
            ctx.new_path();
            ctx.arc(cx, cy, radius, 0.0, TAU);
        };

        // Translucency is the swatch's own: a locked row fades its swatches
        // without turning them into checkerboards.
        checkerboard_behind(ctx, swatch.color.a, circle);
        circle(ctx);
        set_color(ctx, fill);
        let _ = ctx.fill_preserve();
        let (edge, width) = swatch_edge_stroke(fill, background, SWATCH_HAIRLINE, 1.0);
        set_color(ctx, edge);
        ctx.set_line_width(width);
        let _ = ctx.stroke();

        if current == Some(index) {
            selection_ring(ctx, *rect, state);
        }
    }

    if let Some(none) = extras.none {
        draw_no_fill(ctx, none, extras.none_selected, state);
    }
    if let Some(more) = extras.more {
        draw_more_colors(ctx, more, state);
    }
}

/// "No fill": an empty circle struck through, as a crossed-out swatch.
fn draw_no_fill(ctx: &cairo::Context, rect: PanelRect, selected: bool, state: ControlState) {
    let hovered = state.hovers(|hit| matches!(hit, PropertiesPanelHit::NoFill(_)));
    let (cx, cy) = rect.center();
    let radius = rect.width / 2.0 - 0.5;
    ctx.new_path();
    ctx.arc(cx, cy, radius, 0.0, TAU);
    set_color(
        ctx,
        state.fade(if hovered {
            ACCENT_BRIGHT
        } else {
            QUIET_OUTLINE
        }),
    );
    ctx.set_line_width(1.5);
    let _ = ctx.stroke();
    let reach = radius * std::f64::consts::FRAC_1_SQRT_2;
    set_color(ctx, state.fade(NO_FILL_STRIKE));
    round_line(ctx, (cx - reach, cy + reach), (cx + reach, cy - reach));
    if selected {
        selection_ring(ctx, rect, state);
    }
}

fn draw_more_colors(ctx: &cairo::Context, more: PanelRect, state: ControlState) {
    let hovered = state.hovers(|hit| matches!(hit, PropertiesPanelHit::MoreColors(_)));
    let (cx, cy) = more.center();
    let outline = if hovered {
        ACCENT_BRIGHT
    } else {
        QUIET_OUTLINE
    };
    ctx.new_path();
    ctx.arc(cx, cy, more.width / 2.0 - 0.5, 0.0, TAU);
    set_color(ctx, state.fade(outline));
    ctx.set_line_width(1.0);
    ctx.set_dash(&[2.0, 2.0], 0.0);
    let _ = ctx.stroke();
    ctx.set_dash(&[], 0.0);
    set_color(
        ctx,
        state.fade(if hovered { TEXT_PRIMARY } else { TEXT_HINT }),
    );
    plus_glyph(ctx, (cx, cy));
}

pub(super) fn draw_stepper(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    (down, value, up): (PanelRect, PanelRect, PanelRect),
    text: &str,
    state: ControlState,
) {
    let well = PanelRect::new(down.x, down.y, up.right() - down.x, down.height);
    set_color(ctx, BG_HOVER_WASH);
    draw_rounded_rect(ctx, well.x, well.y, well.width, well.height, RADIUS_STD);
    let _ = ctx.fill();

    for (rect, is_up) in [(down, false), (up, true)] {
        let hovered = state.hovers(|hit| match hit {
            PropertiesPanelHit::StepUp(_) => is_up,
            PropertiesPanelHit::StepDown(_) => !is_up,
            _ => false,
        });
        if hovered {
            set_color(ctx, BG_HOVER);
            draw_rounded_rect(ctx, rect.x, rect.y, rect.width, rect.height, RADIUS_STD);
            let _ = ctx.fill();
        }
        set_color(
            ctx,
            if !state.enabled {
                TEXT_DISABLED
            } else if hovered {
                TEXT_WHITE
            } else {
                TEXT_SECONDARY
            },
        );
        if is_up {
            plus_glyph(ctx, rect.center());
        } else {
            minus_glyph(ctx, rect.center());
        }
    }

    let style = text_style(VALUE_FONT, FontWeight::Normal);
    let layout = engine.layout(ctx, style, text, None);
    let extents = layout.ink_extents();
    let (cx, cy) = value.center();
    set_color(
        ctx,
        if state.enabled {
            TEXT_PRIMARY
        } else {
            TEXT_DISABLED
        },
    );
    layout.show_at_baseline(ctx, cx - extents.x_advance() / 2.0, cy + VALUE_FONT * 0.35);
}

/// A slider: a track filled up to the thumb in the selection's color, and
/// the readout to its right. A mixed value draws no fill and no thumb; the
/// readout says "Mixed", and a drag sets every shape alike.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_slider(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    track: PanelRect,
    readout: PanelRect,
    text: &str,
    value: Option<f64>,
    range: LevelRange,
    fill: Option<Color>,
    state: ControlState,
) {
    let (_, cy) = track.center();
    let left = track.x + SLIDER_THUMB_RADIUS;
    let right = track.right() - SLIDER_THUMB_RADIUS;
    let half = SLIDER_TRACK_HEIGHT / 2.0;
    set_color(ctx, state.fade(SWITCH_TRACK_OFF));
    draw_rounded_rect(
        ctx,
        left - half,
        cy - half,
        right - left + SLIDER_TRACK_HEIGHT,
        SLIDER_TRACK_HEIGHT,
        half,
    );
    let _ = ctx.fill();

    if let Some(value) = value {
        let thumb = range.thumb_x(track, value);
        let fill = fill.map_or(ACCENT_PRIMARY, |color| rgba(Color { a: 1.0, ..color }));
        set_color(ctx, state.fade(fill));
        draw_rounded_rect(
            ctx,
            left - half,
            cy - half,
            thumb - left + SLIDER_TRACK_HEIGHT,
            SLIDER_TRACK_HEIGHT,
            half,
        );
        let _ = ctx.fill();

        let hovered = state.hovers(|hit| matches!(hit, PropertiesPanelHit::Slider(_)));
        ctx.new_path();
        ctx.arc(thumb, cy, SLIDER_THUMB_RADIUS, 0.0, TAU);
        set_color(ctx, state.fade(TEXT_WHITE));
        let _ = ctx.fill_preserve();
        set_color(
            ctx,
            state.fade(if hovered {
                ACCENT_BRIGHT
            } else {
                with_alpha(ACCENT_PRIMARY, 0.6)
            }),
        );
        ctx.set_line_width(if hovered { 3.0 } else { 2.0 });
        let _ = ctx.stroke();
    }

    let style = text_style(VALUE_FONT, FontWeight::Normal);
    let width = engine
        .layout(ctx, style, text, None)
        .ink_extents()
        .x_advance();
    set_color(
        ctx,
        if state.enabled {
            TEXT_PRIMARY
        } else {
            TEXT_DISABLED
        },
    );
    engine.draw_baseline(
        ctx,
        style,
        text,
        readout.right() - width,
        readout.center().1 + VALUE_FONT * 0.35,
        None,
    );
}

pub(super) fn draw_switch(
    ctx: &cairo::Context,
    rect: PanelRect,
    on: Option<bool>,
    state: ControlState,
) {
    let hovered = state.hovers(|hit| {
        matches!(
            hit,
            PropertiesPanelHit::Toggle(_) | PropertiesPanelHit::Row(_)
        )
    });
    let radius = rect.height / 2.0;
    let track = match on {
        Some(true) => ACCENT_PRIMARY,
        Some(false) => SWITCH_TRACK_OFF,
        None => with_alpha(ACCENT_PRIMARY, 0.4),
    };
    set_color(ctx, state.fade(track));
    draw_rounded_rect(ctx, rect.x, rect.y, rect.width, rect.height, radius);
    let _ = ctx.fill_preserve();
    if hovered {
        set_color(ctx, with_alpha(ACCENT_BRIGHT, 0.6));
        ctx.set_line_width(1.0);
        let _ = ctx.stroke();
    }
    ctx.new_path();

    let knob = radius - 3.0;
    let (_, cy) = rect.center();
    match on {
        Some(on) => {
            let cx = if on {
                rect.right() - 3.0 - knob
            } else {
                rect.x + 3.0 + knob
            };
            ctx.arc(cx, cy, knob, 0.0, TAU);
            set_color(
                ctx,
                state.fade(if on { TEXT_WHITE } else { TEXT_SECONDARY }),
            );
            let _ = ctx.fill();
        }
        // Mixed: a bar in the middle, neither on nor off.
        None => {
            let (cx, _) = rect.center();
            set_color(ctx, state.fade(TEXT_WHITE));
            draw_rounded_rect(ctx, cx - 7.0, cy - 2.0, 14.0, 4.0, 2.0);
            let _ = ctx.fill();
        }
    }
}

pub(super) fn draw_arrow_head_segments(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    well: PanelRect,
    start: PanelRect,
    end: PanelRect,
    at_end: Option<bool>,
    state: ControlState,
) {
    set_color(ctx, BG_HOVER_WASH);
    draw_rounded_rect(ctx, well.x, well.y, well.width, well.height, RADIUS_STD);
    let _ = ctx.fill();

    let style = text_style(VALUE_FONT, FontWeight::Normal);
    for (rect, is_end, label) in [(start, false, "Start"), (end, true, "End")] {
        let active = at_end == Some(is_end);
        let hovered = state.hovers(
            |hit| matches!(hit, PropertiesPanelHit::ArrowHead { at_end, .. } if at_end == is_end),
        );
        if active {
            set_color(ctx, state.fade(ACCENT_PRIMARY));
            draw_rounded_rect(ctx, rect.x, rect.y, rect.width, rect.height, RADIUS_SM);
            let _ = ctx.fill();
        } else if hovered {
            set_color(ctx, BG_HOVER_WASH);
            draw_rounded_rect(ctx, rect.x, rect.y, rect.width, rect.height, RADIUS_SM);
            let _ = ctx.fill();
        }

        let text_width = engine
            .layout(ctx, style, label, None)
            .ink_extents()
            .x_advance();
        let content = text_width + SEGMENT_ICON_GAP + SEGMENT_ICON_WIDTH;
        let (cx, cy) = rect.center();
        let left = cx - content / 2.0;
        let color = if !state.enabled {
            TEXT_DISABLED
        } else if active {
            TEXT_WHITE
        } else if hovered {
            TEXT_PRIMARY
        } else {
            TEXT_HINT
        };
        set_color(ctx, color);
        let (text_x, icon_x) = if is_end {
            (left, left + text_width + SEGMENT_ICON_GAP)
        } else {
            (left + SEGMENT_ICON_WIDTH + SEGMENT_ICON_GAP, left)
        };
        engine.draw_baseline(ctx, style, label, text_x, cy + VALUE_FONT * 0.35, None);
        arrow_glyph(ctx, icon_x, cy, is_end);
    }
}

pub(super) fn draw_arrow_styles(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    buttons: &[(ArrowStyle, PanelRect)],
    current: Option<ArrowStyle>,
    state: ControlState,
) {
    let caption = text_style(CAPTION_FONT, FontWeight::Normal);
    for (style, rect) in buttons {
        let active = current == Some(*style);
        let hovered = state.hovers(|hit| {
            matches!(hit, PropertiesPanelHit::ArrowStyle { style: hovered, .. } if hovered == *style)
        });

        draw_rounded_rect(
            ctx,
            rect.x + 0.5,
            rect.y + 0.5,
            rect.width - 1.0,
            rect.height - 1.0,
            RADIUS_MD,
        );
        set_color(
            ctx,
            if active {
                state.fade(with_alpha(ACCENT_PRIMARY, 0.3))
            } else {
                BG_HOVER_WASH
            },
        );
        let _ = ctx.fill_preserve();
        let outline = if active {
            ACCENT_BRIGHT
        } else if hovered {
            with_alpha(ACCENT_BRIGHT, 0.6)
        } else {
            QUIET_OUTLINE
        };
        set_color(ctx, state.fade(outline));
        ctx.set_line_width(1.0);
        let _ = ctx.stroke();

        let ink = state.fade(if active || hovered {
            TEXT_WHITE
        } else {
            TEXT_SECONDARY
        });
        draw_arrow_style_preview(
            ctx,
            (rect.x + 6.0, rect.y + 5.0, rect.width - 12.0, 18.0),
            *style,
            ink,
        );

        let label = style.label();
        let width = engine
            .layout(ctx, caption, label, None)
            .ink_extents()
            .x_advance();
        set_color(ctx, ink);
        engine.draw_baseline(
            ctx,
            caption,
            label,
            rect.x + (rect.width - width) / 2.0,
            rect.bottom() - 8.0,
            None,
        );
    }
}
