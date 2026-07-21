//! Interactive bottom-right zoom chip (M8 Part 2).
//!
//! A persistent, interactive `⊖  NN%  ⊕  Fit` control anchored to the
//! bottom-right corner. In the default pill layout zoom is otherwise
//! keyboard-only; this chip surfaces the live zoom percentage and lets the
//! user step the zoom by clicking. It is a builtin Cairo overlay modelled on
//! the status bar (`bar.rs`): the layout is computed headlessly once per
//! frame and cached on `InputState`, so rendering, damage geometry, and
//! pointer hit-testing all read the same cache and can never disagree.
//!
//! Buttons dispatch through the shared zoom action path:
//! `⊖` = ZoomOut, `⊕` = ZoomIn, `Fit` = ResetZoom (back to 100%), and — only
//! while zoom is active — a compact `Lock` toggle = ToggleZoomLock. The `NN%`
//! readout is a passive display (it consumes clicks but triggers nothing), so
//! no drag ever starts on the canvas beneath the chip.

use super::super::primitives::draw_pill;
use super::super::theme::{self, overlay};
use crate::config::StatusBarStyle;
use crate::input::{BoardBackground, InputState};
use crate::ui_text::{UiTextExtents, UiTextStyle, measure_text, text_layout};

// ============================================================================
// UI Layout Constants (not configurable) — mirror the status bar pill so the
// chip reads as the same chrome family.
// ============================================================================

/// Inset between the chip pill and the screen edges.
const ZOOM_CHIP_EDGE_INSET: f64 = overlay::SPACING_MD;
/// Corner radius of the chip pill (shares the status bar pill radius token so
/// the two bottom-anchored status pills can never drift apart).
const ZOOM_CHIP_CORNER_RADIUS: f64 = overlay::STATUS_PILL_RADIUS;
/// Minimum pill height so every button hit target is at least this tall.
const ZOOM_CHIP_MIN_HEIGHT: f64 = 28.0;
/// Minimum width of an interactive button hit target; narrower natural rects
/// (single glyphs at small user font sizes) are widened to this.
const ZOOM_CHIP_MIN_BUTTON_WIDTH: f64 = 28.0;
/// Horizontal gap between adjacent chip pieces.
const ZOOM_CHIP_PIECE_GAP: f64 = overlay::SPACING_MD;
/// Vertical gap the chip leaves above a bottom-right status bar it stacks over,
/// so the two bottom-anchored pills read as a tidy stack rather than touching.
const ZOOM_CHIP_STATUS_GAP: f64 = overlay::SPACING_MD;

/// Zoom-out button glyph (circled minus).
const ZOOM_OUT_GLYPH: &str = "\u{2296}";
/// Zoom-in button glyph (circled plus).
const ZOOM_IN_GLYPH: &str = "\u{2295}";
/// Reset-to-100% button label.
const ZOOM_FIT_LABEL: &str = "Fit";
/// Zoom-lock toggle label (shown only while zoom is active).
const ZOOM_LOCK_LABEL: &str = "Lock";

// ============================================================================
// Layout types (cached on `InputState`, consumed by rendering, hit-testing,
// and damage geometry)
// ============================================================================

/// Interactive surface a zoom chip button activates on click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomChipButtonKind {
    /// `⊖` — dispatches ZoomOut.
    Out,
    /// `⊕` — dispatches ZoomIn.
    In,
    /// `Fit` — dispatches ResetZoom (back to 100%).
    Fit,
    /// `Lock` — dispatches ToggleZoomLock (present only while zoomed).
    Lock,
}

/// Three-state record of a left press that fell somewhere inside the zoom chip
/// pill, held between press and release so the chip owns its whole gesture.
///
/// This replaces a two-state `Option<ZoomChipButtonKind>`, which could not tell
/// a press on the passive `NN%` readout (or an inter-piece gap) apart from "no
/// chip press at all" — both mapped to `None`. A passive press therefore
/// swallowed the press but leaked the *release* into normal overlay/canvas
/// routing, where it could finish an unrelated interaction. The distinct
/// `Passive` variant keeps such a release consumed (firing nothing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZoomChipPress {
    /// No chip press is pending; the release is NOT the chip's to consume.
    #[default]
    None,
    /// The press landed inside the pill but not on an actionable button (the
    /// passive `NN%` readout or an inter-piece gap). The matching release IS
    /// consumed by the chip and fires no action.
    Passive,
    /// The press landed on an actionable button. The matching release IS
    /// consumed by the chip and fires the button's action only when it lands on
    /// the SAME button.
    Button(ZoomChipButtonKind),
}

impl ZoomChipPress {
    /// True for a press that landed inside the chip (`Passive` or `Button`) and
    /// whose release the chip must therefore consume; false for `None`.
    pub fn is_pending(self) -> bool {
        !matches!(self, ZoomChipPress::None)
    }
}

/// Clickable rect (absolute screen coordinates) mapped to a button.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ZoomChipButton {
    pub(crate) kind: ZoomChipButtonKind,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl ZoomChipButton {
    pub(crate) fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

/// One laid-out text run on the chip's shared baseline. `button` is `Some`
/// for the interactive glyph/label runs and `None` for the passive `NN%`
/// readout.
#[derive(Debug, Clone)]
pub(crate) struct ZoomChipRun {
    pub(crate) text: String,
    pub(crate) x: f64,
    pub(crate) button: Option<ZoomChipButtonKind>,
}

/// Cached zoom chip geometry for one frame: the pill, its text runs, and the
/// interactive button rects.
#[derive(Debug, Clone)]
pub struct ZoomChipLayout {
    pub(crate) pill_x: f64,
    pub(crate) pill_y: f64,
    pub(crate) pill_width: f64,
    pub(crate) pill_height: f64,
    pub(crate) runs: Vec<ZoomChipRun>,
    /// Absolute baseline y shared by every text run.
    pub(crate) line_baseline: f64,
    pub(crate) buttons: Vec<ZoomChipButton>,
    /// Whether zoom is currently locked (tints the `Lock` button on-state).
    pub(crate) lock_active: bool,
    /// (x, y, w, h) footprint for damage tracking.
    pub(crate) bounds: (f64, f64, f64, f64),
    /// Screen size this layout was computed for.
    pub(crate) screen_width: u32,
    pub(crate) screen_height: u32,
}

impl ZoomChipLayout {
    pub(crate) fn chip_contains(&self, x: f64, y: f64) -> bool {
        x >= self.pill_x
            && x <= self.pill_x + self.pill_width
            && y >= self.pill_y
            && y <= self.pill_y + self.pill_height
    }

    pub(crate) fn button_at(&self, x: f64, y: f64) -> Option<ZoomChipButtonKind> {
        self.buttons
            .iter()
            .find(|button| button.contains(x, y))
            .map(|button| button.kind)
    }
}

/// On-screen bounds (x, y, width, height) the chip occupies, without
/// rendering it. Used for damage tracking; the bounds come from the same
/// cached layout rendering consumes, so the two always agree. Returns `None`
/// when no chip layout exists for this screen size (chip hidden).
pub fn zoom_chip_geometry(
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) -> Option<(f64, f64, f64, f64)> {
    let layout = input_state.zoom_chip_layout()?;
    (layout.screen_width == screen_width && layout.screen_height == screen_height)
        .then_some(layout.bounds)
}

// ============================================================================
// Layout computation
// ============================================================================

fn chip_text_style(font_size: f64) -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: font_size,
    }
}

/// One chip piece before positioning: a glyph/label button or the `NN%`
/// passive readout (`kind == None`).
struct ChipPiece {
    text: String,
    kind: Option<ZoomChipButtonKind>,
    extents: UiTextExtents,
}

/// Compute the zoom chip layout headlessly (no rendering context; text goes
/// through the shared measurement cache, so rendering agrees exactly).
/// Callers gate on `show_zoom_actions`; this always lays out the visible chip.
pub fn compute_zoom_chip_layout(
    input_state: &InputState,
    style: &StatusBarStyle,
    screen_width: u32,
    screen_height: u32,
) -> Option<ZoomChipLayout> {
    let text_style = chip_text_style(style.font_size);

    // Live percentage from the current scale (1.0 → "100%" when not zoomed):
    // the chip is a persistent control, not an only-when-zoomed indicator.
    let pct = (input_state.zoom_scale() * 100.0).round() as i32;
    let lock_active = input_state.zoom_locked();

    // Piece order: ⊖  NN%  ⊕  Fit  [Lock]. The Lock toggle only makes sense
    // while zoomed, so it is appended only then.
    let mut specs: Vec<(String, Option<ZoomChipButtonKind>)> = vec![
        (ZOOM_OUT_GLYPH.to_string(), Some(ZoomChipButtonKind::Out)),
        (format!("{pct}%"), None),
        (ZOOM_IN_GLYPH.to_string(), Some(ZoomChipButtonKind::In)),
        (ZOOM_FIT_LABEL.to_string(), Some(ZoomChipButtonKind::Fit)),
    ];
    if input_state.zoom_active() {
        specs.push((ZOOM_LOCK_LABEL.to_string(), Some(ZoomChipButtonKind::Lock)));
    }

    let mut pieces: Vec<ChipPiece> = Vec::with_capacity(specs.len());
    for (text, kind) in specs {
        let extents = measure_text(text_style, &text, None)?;
        pieces.push(ChipPiece {
            text,
            kind,
            extents,
        });
    }

    // Shared ascent/descent so every run sits on one baseline.
    let mut ascent = 0.0_f64;
    let mut descent = 0.0_f64;
    for piece in &pieces {
        ascent = ascent.max(-piece.extents.y_bearing());
        descent = descent.max(piece.extents.height() + piece.extents.y_bearing());
    }
    let line_height = ascent + descent;

    let content_width: f64 = pieces
        .iter()
        .map(|piece| piece.extents.x_advance())
        .sum::<f64>()
        + ZOOM_CHIP_PIECE_GAP * pieces.len().saturating_sub(1) as f64;

    let v_pad = style.padding * 0.5;
    let pill_width = content_width + style.padding * 2.0;
    let pill_height = (line_height + v_pad * 2.0).max(ZOOM_CHIP_MIN_HEIGHT);

    let status_bar_avoid_top =
        bottom_right_status_bar_avoid_top(input_state, screen_width, screen_height);
    let (pill_x, pill_y) = chip_origin(
        screen_width as f64,
        screen_height as f64,
        pill_width,
        pill_height,
        status_bar_avoid_top,
    );
    let line_baseline = pill_y + (pill_height - line_height) / 2.0 + ascent;
    let pill_right = pill_x + pill_width;

    let mut runs = Vec::with_capacity(pieces.len());
    let mut buttons = Vec::new();
    let mut cursor = pill_x + style.padding;
    for (index, piece) in pieces.iter().enumerate() {
        if index > 0 {
            cursor += ZOOM_CHIP_PIECE_GAP;
        }
        let advance = piece.extents.x_advance();
        if let Some(kind) = piece.kind {
            // Hit target: the piece plus half a gap on each side, at full pill
            // height (>= ZOOM_CHIP_MIN_HEIGHT by construction), clamped inside
            // the pill.
            let hit_x = (cursor - ZOOM_CHIP_PIECE_GAP * 0.5).clamp(pill_x, pill_right);
            let hit_right =
                (cursor + advance + ZOOM_CHIP_PIECE_GAP * 0.5).clamp(pill_x, pill_right);
            buttons.push(ZoomChipButton {
                kind,
                x: hit_x,
                y: pill_y,
                width: (hit_right - hit_x).max(0.0),
                height: pill_height,
            });
        }
        runs.push(ZoomChipRun {
            text: piece.text.clone(),
            x: cursor,
            button: piece.kind,
        });
        cursor += advance;
    }

    widen_narrow_buttons(&mut buttons, pill_x, pill_width);

    Some(ZoomChipLayout {
        pill_x,
        pill_y,
        pill_width,
        pill_height,
        runs,
        line_baseline,
        buttons,
        lock_active,
        bounds: (pill_x, pill_y, pill_width, pill_height),
        screen_width,
        screen_height,
    })
}

/// Bottom-right corner of the pill, clamped so it never leaves the screen even
/// when it is wider than the available space. When a bottom-right status bar
/// shares the corner (`status_bar_avoid_top` = the top of its footprint) the
/// chip is lifted to sit directly above it instead of overdrawing it.
fn chip_origin(
    screen_width: f64,
    screen_height: f64,
    pill_width: f64,
    pill_height: f64,
    status_bar_avoid_top: Option<f64>,
) -> (f64, f64) {
    let inset = ZOOM_CHIP_EDGE_INSET;
    let bx = screen_width - inset - pill_width;
    let mut by = screen_height - inset - pill_height;
    if let Some(avoid_top) = status_bar_avoid_top {
        // Stack directly above the status bar's whole footprint (pill plus any
        // mode badges), so the two never overdraw and the status HUD hit-test
        // can't eclipse the chip's buttons.
        by = by.min(avoid_top - ZOOM_CHIP_STATUS_GAP - pill_height);
    }
    (
        bx.clamp(inset, (screen_width - inset - pill_width).max(inset)),
        by.clamp(inset, (screen_height - inset - pill_height).max(inset)),
    )
}

/// The top edge of a cached status bar's footprint when it is anchored to the
/// bottom-right corner — the only position that collides with the corner-anchored
/// chip. `None` for every other position, when the sizes don't match this
/// frame, or when the bar is hidden, leaving the chip flush in the corner.
///
/// Detection is geometric (right-aligned and bottom-anchored, within a
/// sub-pixel tolerance) rather than reading the config position, so it uses the
/// status bar's actual cached layout — the same layout rendering and hit-testing
/// consume — and the two can never disagree. The damage collector refreshes the
/// status HUD layout before the chip each frame, so this reads a fresh cache.
fn bottom_right_status_bar_avoid_top(
    input_state: &InputState,
    screen_width: u32,
    screen_height: u32,
) -> Option<f64> {
    let layout = input_state.status_hud_layout()?;
    if layout.screen_width != screen_width || layout.screen_height != screen_height {
        return None;
    }
    const EDGE_EPS: f64 = 1.0;
    let inset = ZOOM_CHIP_EDGE_INSET;
    let sw = screen_width as f64;
    let sh = screen_height as f64;
    let right_aligned = ((layout.pill_x + layout.pill_width) - (sw - inset)).abs() <= EDGE_EPS;
    let bottom_anchored = ((layout.pill_y + layout.pill_height) - (sh - inset)).abs() <= EDGE_EPS;
    // `bounds.1` is the top of the union of the pill and any badges stacked
    // above it, so stacking over it clears the badges too.
    (right_aligned && bottom_anchored).then_some(layout.bounds.1)
}

/// Widen button hit rects narrower than [`ZOOM_CHIP_MIN_BUTTON_WIDTH`] to that
/// floor (single glyphs at small user font sizes can drop below it), centered
/// on the natural rect and clamped inside the pill. Neighboring buttons cede
/// the overlapped span — never dropping below the floor themselves — so button
/// rects stay disjoint. Layout only; rendering is unaffected.
fn widen_narrow_buttons(buttons: &mut [ZoomChipButton], pill_x: f64, pill_width: f64) {
    let pill_right = pill_x + pill_width;
    for index in 0..buttons.len() {
        if buttons[index].width >= ZOOM_CHIP_MIN_BUTTON_WIDTH {
            continue;
        }
        let center = buttons[index].x + buttons[index].width / 2.0;
        let half = ZOOM_CHIP_MIN_BUTTON_WIDTH / 2.0;
        let mut left = center - half;
        let mut right = center + half;
        if left < pill_x {
            right += pill_x - left;
            left = pill_x;
        }
        if right > pill_right {
            left -= right - pill_right;
            right = pill_right;
        }
        left = left.max(pill_x);
        if index > 0 {
            let prev = &mut buttons[index - 1];
            let prev_right = prev.x + prev.width;
            if prev_right > left {
                let boundary = left.max((prev.x + ZOOM_CHIP_MIN_BUTTON_WIDTH).min(prev_right));
                prev.width = (boundary - prev.x).max(0.0);
                left = boundary;
            }
        }
        if index + 1 < buttons.len() {
            let next = &mut buttons[index + 1];
            let next_right = next.x + next.width;
            if next.x < right {
                let boundary = right
                    .min((next_right - ZOOM_CHIP_MIN_BUTTON_WIDTH).max(next.x))
                    .max(left);
                next.width = (next_right - boundary).max(0.0);
                next.x = boundary;
                right = boundary;
            }
        }
        let button = &mut buttons[index];
        button.x = left;
        button.width = (right - left).max(0.0);
    }
}

// ============================================================================
// Rendering
// ============================================================================

/// Render the zoom chip from the layout cached on `InputState` by
/// `update_zoom_chip_layout`.
pub fn render_zoom_chip(
    ctx: &cairo::Context,
    input_state: &InputState,
    style: &StatusBarStyle,
    screen_width: u32,
    screen_height: u32,
) {
    let Some(layout) = input_state.zoom_chip_layout() else {
        return;
    };
    if layout.screen_width != screen_width || layout.screen_height != screen_height {
        return;
    }

    let (bg_color, text_color) = match input_state.boards.active_background() {
        BoardBackground::Transparent => (style.bg_color, style.text_color),
        BoardBackground::Solid(color) => {
            theme::Theme::status_palette_for_background(color.r, color.g, color.b)
        }
    };

    draw_pill(
        ctx,
        layout.pill_x,
        layout.pill_y,
        layout.pill_width,
        layout.pill_height,
        ZOOM_CHIP_CORNER_RADIUS,
        (bg_color[0], bg_color[1], bg_color[2], bg_color[3]),
        theme::current().border_hairline,
        None,
    );

    let text_style = chip_text_style(style.font_size);
    let [r, g, b, a] = text_color;

    let _ = ctx.save();
    ctx.rectangle(
        layout.pill_x,
        layout.pill_y,
        layout.pill_width,
        layout.pill_height,
    );
    ctx.clip();
    for run in &layout.runs {
        // The Lock toggle in its on-state gets the accent tint so the locked
        // state reads without a separate label; every other run uses the
        // shared chip text color.
        if run.button == Some(ZoomChipButtonKind::Lock) && layout.lock_active {
            let (ar, ag, ab, aa) = theme::current().accent;
            ctx.set_source_rgba(ar, ag, ab, aa);
        } else {
            ctx.set_source_rgba(r, g, b, a);
        }
        text_layout(ctx, text_style, &run.text, None).show_at_baseline(
            ctx,
            run.x,
            layout.line_baseline,
        );
    }
    let _ = ctx.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig, StatusBarStyle};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");
        InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        )
    }

    /// The chip anchors to the bottom-right corner: its right/bottom edges sit
    /// one inset in from the screen edges, and it stays fully on screen.
    #[test]
    fn chip_anchors_bottom_right() {
        let state = make_state();
        let style = StatusBarStyle::default();
        let (w, h) = (1920_u32, 1080_u32);
        let layout = compute_zoom_chip_layout(&state, &style, w, h).expect("layout");

        assert!(
            (layout.pill_x + layout.pill_width - (w as f64 - ZOOM_CHIP_EDGE_INSET)).abs() < 1e-6
        );
        assert!(
            (layout.pill_y + layout.pill_height - (h as f64 - ZOOM_CHIP_EDGE_INSET)).abs() < 1e-6
        );
        assert!(layout.pill_x >= 0.0 && layout.pill_y >= 0.0);
        assert!(layout.pill_x + layout.pill_width <= w as f64 + 1e-6);
        assert!(layout.pill_y + layout.pill_height <= h as f64 + 1e-6);
    }

    /// The percentage tracks the live scale even at rest (100%), and reflects
    /// an active zoom.
    #[test]
    fn percentage_run_reflects_live_scale() {
        let mut state = make_state();
        let style = StatusBarStyle::default();

        let at_rest = compute_zoom_chip_layout(&state, &style, 1920, 1080).expect("layout");
        assert!(
            at_rest
                .runs
                .iter()
                .any(|run| run.button.is_none() && run.text == "100%"),
            "chip shows 100% at rest"
        );

        state.set_zoom_status(true, false, 2.5, (0.0, 0.0));
        let zoomed = compute_zoom_chip_layout(&state, &style, 1920, 1080).expect("layout");
        assert!(
            zoomed
                .runs
                .iter()
                .any(|run| run.button.is_none() && run.text == "250%")
        );
    }

    /// The core buttons are always present; the Lock toggle appears only while
    /// zoom is active.
    #[test]
    fn lock_button_present_only_when_zoomed() {
        let mut state = make_state();
        let style = StatusBarStyle::default();

        let at_rest = compute_zoom_chip_layout(&state, &style, 1920, 1080).expect("layout");
        for kind in [
            ZoomChipButtonKind::Out,
            ZoomChipButtonKind::In,
            ZoomChipButtonKind::Fit,
        ] {
            assert!(at_rest.buttons.iter().any(|b| b.kind == kind));
        }
        assert!(
            !at_rest
                .buttons
                .iter()
                .any(|b| b.kind == ZoomChipButtonKind::Lock),
            "no Lock toggle when not zoomed"
        );

        state.set_zoom_status(true, true, 2.0, (0.0, 0.0));
        let zoomed = compute_zoom_chip_layout(&state, &style, 1920, 1080).expect("layout");
        assert!(
            zoomed
                .buttons
                .iter()
                .any(|b| b.kind == ZoomChipButtonKind::Lock),
            "Lock toggle present while zoomed"
        );
        assert!(zoomed.lock_active, "lock_active mirrors zoom_locked");
    }

    /// At realistic font sizes the widen step brings every button hit rect up
    /// to the minimum interactive width, and rects stay inside the pill and
    /// never overlap.
    #[test]
    fn buttons_meet_min_width_at_default_font() {
        let state = make_state();
        let style = StatusBarStyle::default();
        let layout = compute_zoom_chip_layout(&state, &style, 1920, 1080).expect("layout");

        for button in &layout.buttons {
            assert!(
                button.width >= ZOOM_CHIP_MIN_BUTTON_WIDTH - 1e-6,
                "{:?} narrower than floor: {}",
                button.kind,
                button.width
            );
            assert!(button.height >= ZOOM_CHIP_MIN_HEIGHT);
            assert!(button.x >= layout.pill_x - 1e-6);
            assert!(button.x + button.width <= layout.pill_x + layout.pill_width + 1e-6);
        }
    }

    /// Even at tiny user font sizes — where the pill is too narrow to grant
    /// every button the full floor (pill containment wins, mirroring the status
    /// bar) — button rects stay inside the pill and never overlap.
    #[test]
    fn buttons_stay_inside_pill_and_disjoint_at_tiny_font() {
        let state = make_state();
        let style = StatusBarStyle {
            font_size: 8.0,
            padding: 2.0,
            ..StatusBarStyle::default()
        };
        let layout = compute_zoom_chip_layout(&state, &style, 1920, 1080).expect("layout");

        for button in &layout.buttons {
            assert!(button.height >= ZOOM_CHIP_MIN_HEIGHT);
            assert!(button.x >= layout.pill_x - 1e-6);
            assert!(button.x + button.width <= layout.pill_x + layout.pill_width + 1e-6);
        }
        for pair in layout.buttons.windows(2) {
            assert!(
                pair[0].x + pair[0].width <= pair[1].x + 1e-6,
                "buttons {:?} and {:?} overlap",
                pair[0].kind,
                pair[1].kind
            );
        }
    }

    /// `button_at` maps hits to the right button and misses to `None`.
    #[test]
    fn button_at_maps_hits_and_misses() {
        let state = make_state();
        let style = StatusBarStyle::default();
        let layout = compute_zoom_chip_layout(&state, &style, 1920, 1080).expect("layout");

        let out = layout
            .buttons
            .iter()
            .find(|b| b.kind == ZoomChipButtonKind::Out)
            .expect("out button");
        assert_eq!(
            layout.button_at(out.x + out.width / 2.0, out.y + out.height / 2.0),
            Some(ZoomChipButtonKind::Out)
        );
        assert_eq!(
            layout.button_at(layout.pill_x - 10.0, layout.pill_y - 10.0),
            None
        );
    }

    /// Regression (M8): the chip is a persistent control gated only on
    /// `show_zoom_actions`, so in the default config it surfaces the zoom % no
    /// matter where the pointer is — including over the toolbar, the case that
    /// previously hid the chip while both fallback badges were suppressed,
    /// leaving no zoom % at all. The layout takes no pointer input, so its
    /// producing the % run is exactly that pointer-independence.
    #[test]
    fn chip_shows_percent_in_default_config_regardless_of_pointer() {
        let mut state = make_state();
        assert!(
            state.show_zoom_actions,
            "default config enables zoom actions"
        );
        let style = StatusBarStyle::default();
        state.set_zoom_status(true, false, 2.5, (0.0, 0.0));

        state.update_zoom_chip_layout(&style, 1920, 1080);
        let layout = state
            .zoom_chip_layout()
            .expect("chip present in default config");
        assert!(
            layout
                .runs
                .iter()
                .any(|run| run.button.is_none() && run.text == "250%"),
            "the persistent chip shows the live zoom %"
        );

        // And the status HUD suppresses its ZOOM badge in this config, so the
        // chip is the sole zoom % indicator (never two).
        let hud = crate::ui::compute_status_hud_layout(
            &state,
            crate::config::StatusPosition::BottomLeft,
            &style,
            1920,
            1080,
        )
        .expect("hud layout");
        assert!(
            !hud.badges.iter().any(|badge| badge.label.contains("ZOOM")),
            "the HUD ZOOM badge yields to the chip when zoom actions are on"
        );
    }

    /// Reconciliation invariant (M8): exactly one zoom-% indicator shows while
    /// zoomed in every {show_zoom_actions}×{show_status_bar} combination —
    /// never zero (the regression), never two. The chip and the status-HUD
    /// ZOOM badge come from their real layouts; the passive top-corner badge is
    /// the backend render gate (`zoom_active && !show_status_bar &&
    /// !show_zoom_actions`, see `backend/wayland/state/render/ui.rs`).
    #[test]
    fn exactly_one_zoom_percent_indicator_while_zoomed() {
        let style = StatusBarStyle::default();
        let (w, h) = (1920_u32, 1080_u32);

        for show_zoom_actions in [true, false] {
            for show_status_bar in [true, false] {
                let mut state = make_state();
                state.show_zoom_actions = show_zoom_actions;
                state.show_status_bar = show_status_bar;
                state.set_zoom_status(true, false, 2.5, (0.0, 0.0));

                // Chip: present and showing the % exactly when zoom actions on.
                state.update_zoom_chip_layout(&style, w, h);
                let chip_pct = state.zoom_chip_layout().is_some_and(|layout| {
                    layout
                        .runs
                        .iter()
                        .any(|run| run.button.is_none() && run.text.ends_with('%'))
                });

                // Status-HUD ZOOM badge: only rendered while the bar is shown,
                // and only present in the layout when the chip does not own it.
                let hud_badge = show_status_bar
                    && crate::ui::compute_status_hud_layout(
                        &state,
                        crate::config::StatusPosition::BottomLeft,
                        &style,
                        w,
                        h,
                    )
                    .expect("hud layout")
                    .badges
                    .iter()
                    .any(|badge| badge.label.contains("ZOOM"));

                // Passive top-corner badge: bar hidden and chip absent.
                let top_badge = state.zoom_active() && !show_status_bar && !show_zoom_actions;

                let count = [chip_pct, hud_badge, top_badge]
                    .into_iter()
                    .filter(|shown| *shown)
                    .count();
                assert_eq!(
                    count, 1,
                    "expected exactly one zoom % (chip={chip_pct}, hud={hud_badge}, \
                     top={top_badge}) for show_zoom_actions={show_zoom_actions}, \
                     show_status_bar={show_status_bar}"
                );
            }
        }
    }

    /// A bottom-right status bar shares the chip's corner: the chip must stack
    /// clear above the status footprint rather than overdraw it (the status HUD
    /// hit-test wins in an overlap and would kill the chip's buttons).
    #[test]
    fn chip_stacks_above_bottom_right_status_bar_without_overlap() {
        let mut state = make_state();
        let style = StatusBarStyle::default();
        let (w, h) = (1920_u32, 1080_u32);
        state.show_status_bar = true;
        // The damage collector refreshes the status HUD layout before the chip
        // each frame; cache it by hand here at the bottom-right corner.
        state.update_status_hud_layout(crate::config::StatusPosition::BottomRight, &style, w, h);
        let status = state.status_hud_layout().expect("status layout").clone();

        let chip = compute_zoom_chip_layout(&state, &style, w, h).expect("chip layout");

        // Chip stays right-anchored one inset in from the right edge...
        assert!(
            (chip.pill_x + chip.pill_width - (w as f64 - ZOOM_CHIP_EDGE_INSET)).abs() < 1e-6,
            "chip stays right-anchored"
        );
        // ...and its bottom edge clears the top of the whole status footprint.
        let (sx, sy, sw, sh) = status.bounds;
        assert!(
            chip.pill_y + chip.pill_height <= sy - ZOOM_CHIP_STATUS_GAP + 1e-6,
            "chip bottom {} must sit a gap above status top {}",
            chip.pill_y + chip.pill_height,
            sy
        );
        // No rectangle overlap between the chip pill and the status footprint.
        let overlap = chip.pill_x < sx + sw
            && sx < chip.pill_x + chip.pill_width
            && chip.pill_y < sy + sh
            && sy < chip.pill_y + chip.pill_height;
        assert!(!overlap, "chip overlaps the bottom-right status bar");
        assert!(chip.pill_y >= 0.0, "chip stays on screen");
    }

    /// Other status-bar positions (the default bottom-left) do not share the
    /// corner, so the chip stays flush in the bottom-right.
    #[test]
    fn chip_stays_flush_with_bottom_left_status_bar() {
        let mut state = make_state();
        let style = StatusBarStyle::default();
        let (w, h) = (1920_u32, 1080_u32);
        state.show_status_bar = true;
        state.update_status_hud_layout(crate::config::StatusPosition::BottomLeft, &style, w, h);

        let chip = compute_zoom_chip_layout(&state, &style, w, h).expect("chip layout");
        assert!(
            (chip.pill_y + chip.pill_height - (h as f64 - ZOOM_CHIP_EDGE_INSET)).abs() < 1e-6,
            "chip stays flush to the bottom with a bottom-left status bar"
        );
    }
}
