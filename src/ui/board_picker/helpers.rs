use std::f64::consts::PI;

use crate::config::Action;
use crate::draw::{BLACK, BLUE, Color, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};
use crate::input::InputState;
use crate::ui::constants::{self, ICON_DRAG_HANDLE};

pub(super) const BOARD_PALETTE: [Color; 11] = [
    RED,
    GREEN,
    BLUE,
    YELLOW,
    WHITE,
    BLACK,
    ORANGE,
    PINK,
    Color {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    },
    Color {
        r: 0.6,
        g: 0.4,
        b: 0.8,
        a: 1.0,
    },
    Color {
        r: 0.4,
        g: 0.4,
        b: 0.4,
        a: 1.0,
    },
];

pub(super) fn board_slot_hint(state: &InputState, index: usize) -> Option<String> {
    let action = match index {
        0 => Action::Board1,
        1 => Action::Board2,
        2 => Action::Board3,
        3 => Action::Board4,
        4 => Action::Board5,
        5 => Action::Board6,
        6 => Action::Board7,
        7 => Action::Board8,
        8 => Action::Board9,
        _ => return None,
    };
    let label = state.action_binding_label(action);
    if label == "Not bound" {
        None
    } else {
        Some(label)
    }
}

pub(super) fn draw_pin_icon(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    color: Color,
    filled: bool,
) {
    let head_radius = (size * 0.22).clamp(2.0, 3.2);
    let stem_length = size * 0.6;
    let head_y = y - stem_length * 0.35;
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.arc(x, head_y, head_radius, 0.0, PI * 2.0);
    if filled {
        let _ = ctx.fill();
    } else {
        ctx.set_line_width(1.2);
        let _ = ctx.stroke();
    }
    ctx.set_line_width(1.2);
    ctx.move_to(x, head_y + head_radius);
    ctx.line_to(x, head_y + head_radius + stem_length);
    let _ = ctx.stroke();
}

pub(super) fn draw_drag_handle(ctx: &cairo::Context, x: f64, y: f64, width: f64) {
    let dot_radius = (width * 0.18).clamp(1.2, 2.2);
    let gap = dot_radius * 2.2;
    let col_gap = dot_radius * 2.6;
    let start_x = x + width * 0.5 - col_gap * 0.5;
    let start_y = y - gap;
    constants::set_color(ctx, ICON_DRAG_HANDLE);
    for row in 0..3 {
        for col in 0..2 {
            let cx = start_x + col as f64 * col_gap;
            let cy = start_y + row as f64 * gap;
            ctx.arc(cx, cy, dot_radius, 0.0, PI * 2.0);
            let _ = ctx.fill();
        }
    }
}
