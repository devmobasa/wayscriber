//! Input HUD: a row of keycap/pill chips showing recent keystrokes and clicks.
//!
//! The row is laid out headlessly (text goes through the shared measurement
//! cache) so damage geometry and rendering read the same numbers and can never
//! disagree — the same split `toasts.rs` uses. Chips are drawn in the shared
//! keycap language and tinted from `status_bar_style`, so the HUD reads as the
//! same chrome family as the status bar and zoom chip.

use crate::config::{InputHudPosition, StatusBarStyle};
use crate::input::state::{InputHudEntryKind, InputState};

use super::primitives::{
    draw_keycap_in_box, draw_rounded_rect, keycap_box_size, keycap_text_style,
};
use super::theme::{self, overlay};
use crate::ui_text::text_layout;

/// Inset between the chip row and the screen edges, matching the other
/// corner-anchored chrome (status HUD, zoom chip).
const INPUT_HUD_EDGE_INSET: f64 = overlay::SPACING_MD;
/// Horizontal gap between adjacent chips.
const INPUT_HUD_CHIP_GAP: f64 = overlay::SPACING_MD;
/// Corner radius of the mouse/scroll pills (the keycaps keep `RADIUS_SM`).
const INPUT_HUD_PILL_RADIUS: f64 = overlay::STATUS_PILL_RADIUS;
/// Repeat-counter separator, e.g. `Backspace ×7`.
const INPUT_HUD_REPEAT_SEPARATOR: &str = " \u{00d7}";

/// One laid-out chip: its full text (label plus any repeat counter), chrome
/// kind, and box within the row.
pub(crate) struct InputHudChip {
    pub(crate) text: String,
    pub(crate) kind: InputHudEntryKind,
    pub(crate) x: f64,
    pub(crate) width: f64,
    /// Per-chip fade, resolved at layout time from the entry's clock.
    pub(crate) alpha: f64,
}

/// The whole chip row for one frame.
pub(crate) struct InputHudLayout {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) chips: Vec<InputHudChip>,
}

/// Full chip text: the chord label plus a repeat counter once the chip has
/// coalesced more than one press.
fn chip_text(label: &str, count: u32) -> String {
    if count > 1 {
        format!("{label}{INPUT_HUD_REPEAT_SEPARATOR}{count}")
    } else {
        label.to_string()
    }
}

/// Lay the chip row out for the current screen size, or `None` when nothing
/// would be drawn. Chips are measured newest-first so an overlong row drops
/// its oldest chips rather than running off screen.
pub(crate) fn compute_input_hud_layout(
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) -> Option<InputHudLayout> {
    if !input_state.input_hud_visible() {
        return None;
    }
    let font_size = input_state.input_hud_font_size();
    let now = std::time::Instant::now();

    // Newest chip first: the row is right-aligned in spirit (new chips arrive
    // on the right and push older ones left), so overflow drops the oldest.
    let mut measured: Vec<(InputHudChip, f64)> = Vec::new();
    let mut row_height = 0.0_f64;
    let mut content_width = 0.0_f64;
    let available = (screen_width as f64 - INPUT_HUD_EDGE_INSET * 2.0).max(0.0);
    if available <= 0.0 {
        return None;
    }

    for entry in input_state.input_hud_entries().rev() {
        let text = chip_text(entry.label(), entry.count());
        let Some((width, height)) = keycap_box_size(&text, font_size) else {
            continue;
        };
        // The newest chip always renders, but never wider than the available
        // span: a large font on a narrow output would otherwise produce a row
        // wider than the surface. Rendering clips each chip to its box, so a
        // clamped label truncates instead of bleeding past the row.
        let width = if measured.is_empty() {
            width.min(available)
        } else {
            width
        };
        let next_width = if measured.is_empty() {
            width
        } else {
            content_width + INPUT_HUD_CHIP_GAP + width
        };
        if !measured.is_empty() && next_width > available {
            break;
        }
        content_width = next_width;
        row_height = row_height.max(height);
        measured.push((
            InputHudChip {
                text,
                kind: entry.kind(),
                x: 0.0,
                width,
                alpha: input_state.input_hud_alpha(entry, now),
            },
            height,
        ));
    }

    if measured.is_empty() || row_height <= 0.0 {
        return None;
    }
    measured.reverse();

    let position = input_state.input_hud_position();
    let x = row_origin_x(position, screen_width as f64, content_width);
    let y = if position.is_top() {
        INPUT_HUD_EDGE_INSET
    } else if position.is_middle() {
        ((screen_height as f64 - row_height) / 2.0).max(0.0)
    } else {
        (screen_height as f64 - INPUT_HUD_EDGE_INSET - row_height).max(0.0)
    };

    let mut cursor = x;
    let mut chips = Vec::with_capacity(measured.len());
    for (mut chip, _) in measured {
        chip.x = cursor;
        cursor += chip.width + INPUT_HUD_CHIP_GAP;
        chips.push(chip);
    }

    Some(InputHudLayout {
        x,
        y,
        width: content_width,
        height: row_height,
        chips,
    })
}

/// Left edge of the row for an anchor, clamped so it never leaves the screen
/// even when the row is wider than the inset-reduced width.
fn row_origin_x(position: InputHudPosition, screen_width: f64, content_width: f64) -> f64 {
    let raw = if position.is_center() {
        (screen_width - content_width) / 2.0
    } else if position.is_right() {
        screen_width - INPUT_HUD_EDGE_INSET - content_width
    } else {
        INPUT_HUD_EDGE_INSET
    };
    raw.clamp(0.0, (screen_width - content_width).max(0.0))
}

/// On-screen bounds (x, y, width, height) the chip row occupies, without
/// rendering it. Used for damage tracking; the bounds come from the same
/// headless layout rendering consumes, so the two always agree.
pub fn input_hud_geometry(
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) -> Option<(f64, f64, f64, f64)> {
    let layout = compute_input_hud_layout(input_state, screen_width, screen_height)?;
    Some((layout.x, layout.y, layout.width, layout.height))
}

/// Render the chip row using the status-bar style tokens, so it reads as the
/// same chrome family as the status HUD and zoom chip.
pub fn render_input_hud(
    ctx: &cairo::Context,
    input_state: &InputState,
    style: &StatusBarStyle,
    screen_width: u32,
    screen_height: u32,
) {
    let Some(layout) = compute_input_hud_layout(input_state, screen_width, screen_height) else {
        return;
    };
    let font_size = input_state.input_hud_font_size();
    let [br, bg, bb, ba] = style.bg_color;
    let [tr, tg, tb, ta] = style.text_color;

    for chip in &layout.chips {
        let alpha = chip.alpha.clamp(0.0, 1.0);
        if alpha <= 0.0 {
            continue;
        }
        let fill = (br, bg, bb, ba * alpha);
        let text_color = (tr, tg, tb, ta * alpha);
        // Clip to the chip's box: the layout may have clamped the newest
        // chip's width to the available span, and the label must truncate at
        // the box edge rather than paint outside the damaged footprint.
        let _ = ctx.save();
        ctx.rectangle(chip.x, layout.y, chip.width, layout.height);
        ctx.clip();
        match chip.kind {
            InputHudEntryKind::Key => {
                draw_keycap_in_box(
                    ctx,
                    chip.x,
                    layout.y,
                    chip.width,
                    layout.height,
                    &chip.text,
                    font_size,
                    fill,
                    text_color,
                );
            }
            InputHudEntryKind::Mouse | InputHudEntryKind::Scroll => {
                draw_input_hud_pill(
                    ctx,
                    chip.x,
                    layout.y,
                    chip.width,
                    layout.height,
                    &chip.text,
                    font_size,
                    fill,
                    text_color,
                );
            }
        }
        let _ = ctx.restore();
    }
}

/// Mouse/scroll chips: the same box and label metrics as a keycap, drawn with
/// the status pill's rounder corners so pointer events read differently from
/// keystrokes at a glance.
#[allow(clippy::too_many_arguments)]
fn draw_input_hud_pill(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    text: &str,
    font_size: f64,
    fill: theme::Rgba,
    text_color: theme::Rgba,
) {
    theme::set_color(ctx, fill);
    draw_rounded_rect(ctx, x, y, width, height, INPUT_HUD_PILL_RADIUS);
    let _ = ctx.fill();

    let layout = text_layout(ctx, keycap_text_style(font_size), text, None);
    let extents = layout.ink_extents();
    theme::set_color(ctx, text_color);
    layout.show_at_baseline(
        ctx,
        x + (width - extents.width()) / 2.0 - extents.x_bearing(),
        y + (height - extents.height()) / 2.0 - extents.y_bearing(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InputHudConfig;
    use crate::input::state::{InputHudSettings, test_support::make_test_input_state};
    use crate::input::{Key, Modifiers};

    fn state_with(config: InputHudConfig) -> InputState {
        let mut state = make_test_input_state();
        state.init_input_hud_from_config(InputHudSettings::from(&config));
        state
    }

    fn enabled_state() -> InputState {
        state_with(InputHudConfig {
            enabled: true,
            ..InputHudConfig::default()
        })
    }

    #[test]
    fn hidden_hud_has_no_geometry() {
        let state = state_with(InputHudConfig::default());
        assert!(input_hud_geometry(&state, 1920, 1080).is_none());

        let empty = enabled_state();
        assert!(
            input_hud_geometry(&empty, 1920, 1080).is_none(),
            "an enabled but empty HUD draws nothing"
        );
    }

    #[test]
    fn bottom_center_row_is_centered_and_bottom_anchored() {
        let mut state = enabled_state();
        state.note_input_hud_key(Key::Char('a'), Modifiers::new());
        let (x, y, width, height) = input_hud_geometry(&state, 1920, 1080).expect("row geometry");

        assert!((x + width / 2.0 - 960.0).abs() < 1e-6, "row is centered");
        assert!(
            (y + height - (1080.0 - INPUT_HUD_EDGE_INSET)).abs() < 1e-6,
            "row sits one inset above the bottom edge"
        );
    }

    #[test]
    fn anchors_place_the_row_on_the_requested_edges() {
        for position in [
            InputHudPosition::TopLeft,
            InputHudPosition::TopCenter,
            InputHudPosition::TopRight,
            InputHudPosition::CenterLeft,
            InputHudPosition::Center,
            InputHudPosition::CenterRight,
            InputHudPosition::BottomLeft,
            InputHudPosition::BottomRight,
        ] {
            let mut state = state_with(InputHudConfig {
                enabled: true,
                position,
                ..InputHudConfig::default()
            });
            state.note_input_hud_key(Key::Char('a'), Modifiers::new());
            let (x, y, width, height) =
                input_hud_geometry(&state, 1920, 1080).expect("row geometry");

            if position.is_right() {
                assert!((x + width - (1920.0 - INPUT_HUD_EDGE_INSET)).abs() < 1e-6);
            } else if position.is_center() {
                assert!((x + width / 2.0 - 960.0).abs() < 1e-6, "row is centered");
            } else {
                assert!((x - INPUT_HUD_EDGE_INSET).abs() < 1e-6);
            }
            if position.is_top() {
                assert!((y - INPUT_HUD_EDGE_INSET).abs() < 1e-6);
            } else if position.is_middle() {
                assert!(
                    (y + height / 2.0 - 540.0).abs() < 1e-6,
                    "middle anchors sit on the vertical center line"
                );
            } else {
                assert!(y > 540.0, "bottom anchors stay in the lower half");
            }
        }
    }

    /// A valid-but-large font on a narrow output can make a single chip wider
    /// than the inset span. The newest chip must clamp to it (rendering clips
    /// the label at the box edge) so the row never leaves the surface.
    #[test]
    fn the_newest_chip_clamps_to_the_available_width() {
        let mut state = state_with(InputHudConfig {
            enabled: true,
            font_size: 72.0,
            ..InputHudConfig::default()
        });
        let mut modifiers = Modifiers::new();
        modifiers.ctrl = true;
        modifiers.shift = true;
        state.note_input_hud_key(Key::Backspace, modifiers);

        let screen_width = 160_u32;
        let available = screen_width as f64 - INPUT_HUD_EDGE_INSET * 2.0;
        let layout = compute_input_hud_layout(&state, screen_width, 1080).expect("layout");
        assert_eq!(layout.chips.len(), 1);
        assert!(
            layout.width <= available,
            "row width {} must not exceed the available span {available}",
            layout.width
        );
        assert!(layout.x >= 0.0);
        assert!(layout.x + layout.width <= screen_width as f64);

        // A surface no wider than its insets has no drawable span at all.
        let no_span = (INPUT_HUD_EDGE_INSET * 2.0) as u32;
        assert!(compute_input_hud_layout(&state, no_span, 1080).is_none());
    }

    #[test]
    fn repeat_counter_is_appended_to_the_chip_text() {
        let mut state = enabled_state();
        for _ in 0..7 {
            state.note_input_hud_key(Key::Backspace, Modifiers::new());
        }
        let layout = compute_input_hud_layout(&state, 1920, 1080).expect("layout");
        assert_eq!(layout.chips.len(), 1);
        assert_eq!(layout.chips[0].text, "Backspace \u{00d7}7");
    }

    #[test]
    fn mouse_chips_keep_the_pill_chrome() {
        let mut state = enabled_state();
        state.note_input_hud_mouse("Click", Modifiers::new());
        state.note_input_hud_scroll(true, Modifiers::new());
        let layout = compute_input_hud_layout(&state, 1920, 1080).expect("layout");
        assert_eq!(layout.chips.len(), 2);
        assert_eq!(layout.chips[0].kind, InputHudEntryKind::Mouse);
        assert_eq!(layout.chips[1].kind, InputHudEntryKind::Scroll);
    }

    /// The row never runs off screen: a narrow surface keeps only the newest
    /// chips that fit inside the inset-reduced width.
    #[test]
    fn overlong_rows_drop_their_oldest_chips() {
        let mut state = enabled_state();
        for label in ['a', 'b', 'c', 'd', 'e', 'f'] {
            state.note_input_hud_key(Key::Char(label), Modifiers::new());
        }
        let layout = compute_input_hud_layout(&state, 120, 1080).expect("layout");

        assert!(layout.chips.len() < 6, "narrow screens shed older chips");
        assert!(layout.x >= 0.0);
        assert!(layout.x + layout.width <= 120.0 + 1e-6);
        assert_eq!(
            layout.chips.last().map(|chip| chip.text.as_str()),
            Some("F"),
            "the newest chip always survives"
        );
    }

    /// Chips share one row height so labels with different ascenders and
    /// descenders still align.
    #[test]
    fn chips_share_a_single_row_height() {
        let mut state = enabled_state();
        state.note_input_hud_key(Key::Backspace, Modifiers::new());
        state.note_input_hud_key(Key::Escape, Modifiers::new());
        let layout = compute_input_hud_layout(&state, 1920, 1080).expect("layout");

        assert_eq!(layout.chips.len(), 2);
        for chip in &layout.chips {
            let (_, natural) =
                keycap_box_size(&chip.text, state.input_hud_font_size()).expect("chip measurement");
            assert!(natural <= layout.height + 1e-6);
        }
    }
}
