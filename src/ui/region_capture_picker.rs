use crate::input::state::RegionSelection;
use crate::util::Rect;

use super::primitives::{draw_rounded_rect, text_extents_for};
use super::region_action_bar::{RegionAction, RegionActionBar, render_region_action_bar};

const SCRIM: (f64, f64, f64, f64) = (0.02, 0.03, 0.05, 0.48);
const PANEL_FILL: (f64, f64, f64, f64) = (12.0 / 255.0, 12.0 / 255.0, 15.0 / 255.0, 0.92);
const PANEL_BORDER: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.16);
const POINTER_GAP: f64 = 15.0;
const PANEL_MARGIN: f64 = 6.0;
const PANEL_PADDING_X: f64 = 8.0;
const PANEL_HEIGHT: f64 = 22.0;
const PANEL_RADIUS: f64 = 6.0;
const READOUT_FONT_SIZE: f64 = 12.0;
const LEGEND_FONT_SIZE: f64 = 12.0;
const AREA_LEGEND_TEXT: &str = "Drag to select   Shift: square   Ctrl+A: all   Esc: cancel";
const AREA_WITH_WINDOWS_LEGEND_TEXT: &str =
    "Drag to select   Shift: square   Ctrl+A: all   Space: window   Esc: cancel";
const WINDOW_LEGEND_TEXT: &str =
    "Click: select   Super+Arrows: choose   Enter: select   Space: area   Esc: cancel";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionCaptureWindowVisual<'a> {
    pub available: bool,
    pub active: bool,
    pub targets: &'a [RegionSelection],
    /// The pointer-hovered or keyboard-focused window candidate.
    pub highlighted_target: Option<usize>,
}

impl RegionCaptureWindowVisual<'_> {
    #[cfg(test)]
    pub(crate) const fn disabled() -> Self {
        Self {
            available: false,
            active: false,
            targets: &[],
            highlighted_target: None,
        }
    }

    fn highlighted_selection(self) -> Option<RegionSelection> {
        self.active
            .then(|| {
                self.highlighted_target
                    .and_then(|index| self.targets.get(index).copied())
            })
            .flatten()
    }
}

fn picker_legend_text(window: RegionCaptureWindowVisual<'_>) -> &'static str {
    if window.active {
        WINDOW_LEGEND_TEXT
    } else if window.available {
        AREA_WITH_WINDOWS_LEGEND_TEXT
    } else {
        AREA_LEGEND_TEXT
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionCapturePickerVisual<'a> {
    pub selection: Option<RegionSelection>,
    pub pointer: (f64, f64),
    /// Authoritative pixel coordinates or size supplied by the picker owner.
    pub measurement: Option<&'a str>,
    pub show_scrim: bool,
    pub show_legend: bool,
    pub loupe: Option<RegionCaptureLoupeVisual>,
    pub action_bar: Option<RegionActionBar>,
    pub hovered_action: Option<RegionAction>,
    pub include_drawings: bool,
    pub window: RegionCaptureWindowVisual<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionCaptureLoupeVisual {
    pub pointer: (f64, f64),
    pub image_center: (f64, f64),
}

impl RegionCaptureLoupeVisual {
    pub(crate) fn when_enabled(
        show_loupe: bool,
        pointer: (f64, f64),
        image_center: (f64, f64),
    ) -> Option<Self> {
        show_loupe.then_some(Self {
            pointer,
            image_center,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PointerPanelLayout {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

pub(crate) fn capture_size_text(size: (u32, u32)) -> String {
    format!("{} × {}", size.0, size.1)
}

/// Conservative targeted damage for Measure Mode's chrome. The crosshair is
/// represented as two thin strips; the selection as four edge strips; and the
/// pointer readout by a bounded box covering every flip direction. This avoids
/// the capture picker's full-surface scrim damage without leaving trails.
pub(crate) fn measure_picker_damage(
    selection: Option<RegionSelection>,
    pointer: (f64, f64),
    screen: (u32, u32),
) -> Vec<Rect> {
    let width = screen.0.min(i32::MAX as u32) as i32;
    let height = screen.1.min(i32::MAX as u32) as i32;
    if width <= 0 || height <= 0 {
        return Vec::new();
    }
    let x = pointer.0.round().clamp(0.0, f64::from(width - 1)) as i32;
    let y = pointer.1.round().clamp(0.0, f64::from(height - 1)) as i32;
    let mut damage = Vec::with_capacity(7);
    push_clipped_damage(&mut damage, x - 2, 0, 5, height, width, height);
    push_clipped_damage(&mut damage, 0, y - 2, width, 5, width, height);
    // The monospace readout is short, but cover both horizontal and vertical
    // flip choices so the damage remains correct without a Cairo text pass.
    push_clipped_damage(&mut damage, x - 240, y - 48, 480, 96, width, height);

    if let Some(selection) = selection {
        let min_x = selection.start.0.min(selection.end.0).floor() as i32;
        let min_y = selection.start.1.min(selection.end.1).floor() as i32;
        let max_x = selection.start.0.max(selection.end.0).ceil() as i32;
        let max_y = selection.start.1.max(selection.end.1).ceil() as i32;
        let rect_width = max_x.saturating_sub(min_x);
        let rect_height = max_y.saturating_sub(min_y);
        push_clipped_damage(
            &mut damage,
            min_x - 4,
            min_y - 4,
            rect_width + 8,
            8,
            width,
            height,
        );
        push_clipped_damage(
            &mut damage,
            min_x - 4,
            max_y - 4,
            rect_width + 8,
            8,
            width,
            height,
        );
        push_clipped_damage(
            &mut damage,
            min_x - 4,
            min_y - 4,
            8,
            rect_height + 8,
            width,
            height,
        );
        push_clipped_damage(
            &mut damage,
            max_x - 4,
            min_y - 4,
            8,
            rect_height + 8,
            width,
            height,
        );
    }
    damage
}

fn push_clipped_damage(
    damage: &mut Vec<Rect>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    screen_width: i32,
    screen_height: i32,
) {
    let min_x = x.clamp(0, screen_width);
    let min_y = y.clamp(0, screen_height);
    let max_x = x.saturating_add(width).clamp(0, screen_width);
    let max_y = y.saturating_add(height).clamp(0, screen_height);
    if let Some(rect) = Rect::from_min_max(min_x, min_y, max_x, max_y) {
        damage.push(rect);
    }
}

pub(crate) fn render_region_capture_loupe(
    ctx: &cairo::Context,
    screen: (u32, u32),
    visual: RegionCaptureLoupeVisual,
    sample: impl FnMut(f64, f64) -> Option<crate::draw::Color>,
) {
    let layout = crate::ui::compute_eyedropper_loupe_layout(visual.pointer, screen);
    crate::ui::render_eyedropper_loupe(ctx, layout, visual.image_center, sample);
}

pub(crate) fn render_region_capture_picker(
    ctx: &cairo::Context,
    screen_width: u32,
    screen_height: u32,
    visual: RegionCapturePickerVisual<'_>,
    mut sample_loupe: impl FnMut(f64, f64) -> Option<crate::draw::Color>,
) {
    let width = f64::from(screen_width);
    let height = f64::from(screen_height);

    let highlighted_window = visual.window.highlighted_selection();
    let effective_selection = if visual.window.active {
        highlighted_window
    } else {
        visual.selection
    };

    let _ = ctx.save();
    if visual.show_scrim {
        ctx.set_source_rgba(SCRIM.0, SCRIM.1, SCRIM.2, SCRIM.3);
        ctx.rectangle(0.0, 0.0, width, height);
        if let Some(selection) = effective_selection {
            let (x, y, w, h) = normalized_rect(selection);
            ctx.rectangle(x, y, w, h);
            ctx.set_fill_rule(cairo::FillRule::EvenOdd);
        }
        let _ = ctx.fill();
        ctx.set_fill_rule(cairo::FillRule::Winding);
    }

    if visual.window.active {
        draw_window_target_frames(ctx, visual.window);
    }
    if let Some(selection) = effective_selection {
        let (x, y, w, h) = normalized_rect(selection);
        draw_selection_frame(ctx, x, y, w, h);
    }
    if !visual.window.active {
        draw_crosshair(ctx, visual.pointer, (width, height));
    }

    if let Some(measurement) = visual.measurement {
        draw_pointer_panel(
            ctx,
            measurement,
            READOUT_FONT_SIZE,
            visual.pointer,
            (screen_width, screen_height),
            cairo::FontWeight::Bold,
        );
    }
    if visual.show_legend && (visual.window.active || visual.selection.is_none()) {
        draw_legend(
            ctx,
            (screen_width, screen_height),
            picker_legend_text(visual.window),
        );
    }
    if let Some(loupe) = visual.loupe {
        render_region_capture_loupe(ctx, (screen_width, screen_height), loupe, &mut sample_loupe);
    }
    if let Some(action_bar) = visual.action_bar {
        render_region_action_bar(
            ctx,
            action_bar,
            visual.hovered_action,
            visual.include_drawings,
        );
    }
    let _ = ctx.restore();
}

fn normalized_rect(selection: RegionSelection) -> (f64, f64, f64, f64) {
    let x = selection.start.0.min(selection.end.0);
    let y = selection.start.1.min(selection.end.1);
    (
        x,
        y,
        (selection.end.0 - selection.start.0).abs(),
        (selection.end.1 - selection.start.1).abs(),
    )
}

fn draw_selection_frame(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    ctx.set_line_width(1.0);
    ctx.rectangle(x + 0.5, y + 0.5, (w - 1.0).max(0.0), (h - 1.0).max(0.0));
    let _ = ctx.stroke();

    let arm = (w.min(h) / 4.0).clamp(4.0, 20.0);
    ctx.set_line_width(2.0);
    ctx.set_line_cap(cairo::LineCap::Square);
    for (corner_x, corner_y, dx, dy) in [
        (x, y, 1.0, 1.0),
        (x + w, y, -1.0, 1.0),
        (x, y + h, 1.0, -1.0),
        (x + w, y + h, -1.0, -1.0),
    ] {
        ctx.move_to(corner_x + dx * arm, corner_y);
        ctx.line_to(corner_x, corner_y);
        ctx.line_to(corner_x, corner_y + dy * arm);
        let _ = ctx.stroke();
    }
}

fn draw_window_target_frames(ctx: &cairo::Context, window: RegionCaptureWindowVisual<'_>) {
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.34);
    ctx.set_line_width(1.0);
    for target in window.targets {
        let (x, y, width, height) = normalized_rect(*target);
        ctx.rectangle(
            x + 0.5,
            y + 0.5,
            (width - 1.0).max(0.0),
            (height - 1.0).max(0.0),
        );
        let _ = ctx.stroke();
    }
}

fn draw_crosshair(ctx: &cairo::Context, pointer: (f64, f64), screen: (f64, f64)) {
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.22);
    ctx.set_line_width(1.0);
    ctx.move_to(0.0, pointer.1 + 0.5);
    ctx.line_to(screen.0, pointer.1 + 0.5);
    ctx.move_to(pointer.0 + 0.5, 0.0);
    ctx.line_to(pointer.0 + 0.5, screen.1);
    let _ = ctx.stroke();
}

fn pointer_panel_layout(
    pointer: (f64, f64),
    text_width: f64,
    screen: (u32, u32),
) -> PointerPanelLayout {
    let screen_width = f64::from(screen.0);
    let screen_height = f64::from(screen.1);
    let width =
        (text_width + PANEL_PADDING_X * 2.0).min((screen_width - PANEL_MARGIN * 2.0).max(0.0));
    let height = PANEL_HEIGHT.min((screen_height - PANEL_MARGIN * 2.0).max(0.0));
    let mut x = pointer.0 + POINTER_GAP;
    let mut y = pointer.1 + POINTER_GAP;
    if x + width + PANEL_MARGIN > screen_width {
        x = pointer.0 - POINTER_GAP - width;
    }
    if y + height + PANEL_MARGIN > screen_height {
        y = pointer.1 - POINTER_GAP - height;
    }
    PointerPanelLayout {
        x: x.clamp(
            PANEL_MARGIN,
            (screen_width - width - PANEL_MARGIN).max(PANEL_MARGIN),
        ),
        y: y.clamp(
            PANEL_MARGIN,
            (screen_height - height - PANEL_MARGIN).max(PANEL_MARGIN),
        ),
        width,
        height,
    }
}

fn draw_pointer_panel(
    ctx: &cairo::Context,
    text: &str,
    font_size: f64,
    pointer: (f64, f64),
    screen: (u32, u32),
    weight: cairo::FontWeight,
) {
    let extents = text_extents_for(
        ctx,
        "monospace",
        cairo::FontSlant::Normal,
        weight,
        font_size,
        text,
    );
    let layout = pointer_panel_layout(pointer, extents.width(), screen);
    if layout.width <= 0.0 || layout.height <= 0.0 {
        return;
    }
    ctx.set_source_rgba(PANEL_FILL.0, PANEL_FILL.1, PANEL_FILL.2, PANEL_FILL.3);
    let radius = PANEL_RADIUS
        .min(layout.width / 2.0)
        .min(layout.height / 2.0);
    draw_rounded_rect(ctx, layout.x, layout.y, layout.width, layout.height, radius);
    let _ = ctx.fill();
    ctx.set_source_rgba(
        PANEL_BORDER.0,
        PANEL_BORDER.1,
        PANEL_BORDER.2,
        PANEL_BORDER.3,
    );
    ctx.set_line_width(1.0);
    draw_rounded_rect(
        ctx,
        layout.x + 0.5,
        layout.y + 0.5,
        layout.width - 1.0,
        layout.height - 1.0,
        (radius - 0.5).max(0.0),
    );
    let _ = ctx.stroke();

    ctx.set_source_rgb(1.0, 1.0, 1.0);
    ctx.select_font_face(
        "monospace",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    ctx.set_font_size(font_size);
    let baseline = layout.y + (layout.height - extents.height()) / 2.0 - extents.y_bearing();
    let _ = ctx.save();
    ctx.rectangle(layout.x, layout.y, layout.width, layout.height);
    ctx.clip();
    ctx.move_to(layout.x + PANEL_PADDING_X - extents.x_bearing(), baseline);
    let _ = ctx.show_text(text);
    let _ = ctx.restore();
}

fn draw_legend(ctx: &cairo::Context, screen: (u32, u32), text: &str) {
    let extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        LEGEND_FONT_SIZE,
        text,
    );
    let screen_width = f64::from(screen.0);
    let screen_height = f64::from(screen.1);
    let width = (extents.width() + 24.0).min((screen_width - 12.0).max(0.0));
    let height = 28.0_f64.min((screen_height - 12.0).max(0.0));
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let x = ((screen_width - width) / 2.0).max(6.0);
    let y = 12.0_f64.min((screen_height - height).max(0.0));
    let radius = PANEL_RADIUS.min(width / 2.0).min(height / 2.0);
    ctx.set_source_rgba(PANEL_FILL.0, PANEL_FILL.1, PANEL_FILL.2, PANEL_FILL.3);
    draw_rounded_rect(ctx, x, y, width, height, radius);
    let _ = ctx.fill();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.88);
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(LEGEND_FONT_SIZE);
    let text_x = x + ((width - extents.width()) / 2.0).max(6.0) - extents.x_bearing();
    let baseline = y + (height - extents.height()) / 2.0 - extents.y_bearing();
    let _ = ctx.save();
    ctx.rectangle(x, y, width, height);
    ctx.clip();
    ctx.move_to(text_x, baseline);
    let _ = ctx.show_text(text);
    let _ = ctx.restore();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_readout_uses_export_pixel_units_and_multiplication_sign() {
        assert_eq!(capture_size_text((0, 0)), "0 × 0");
        assert_eq!(capture_size_text((900, 620)), "900 × 620");
    }

    #[test]
    fn legend_only_advertises_supported_window_controls() {
        assert_eq!(
            picker_legend_text(RegionCaptureWindowVisual {
                available: false,
                active: false,
                targets: &[],
                highlighted_target: None,
            }),
            "Drag to select   Shift: square   Ctrl+A: all   Esc: cancel"
        );
        assert_eq!(
            picker_legend_text(RegionCaptureWindowVisual {
                available: true,
                active: false,
                targets: &[],
                highlighted_target: None,
            }),
            "Drag to select   Shift: square   Ctrl+A: all   Space: window   Esc: cancel"
        );
        assert_eq!(
            picker_legend_text(RegionCaptureWindowVisual {
                available: true,
                active: true,
                targets: &[],
                highlighted_target: None,
            }),
            "Click: select   Super+Arrows: choose   Enter: select   Space: area   Esc: cancel"
        );
    }

    #[test]
    fn pointer_panel_prefers_below_right_then_flips_and_clamps() {
        assert_eq!(
            pointer_panel_layout((20.0, 30.0), 60.0, (400, 300)),
            PointerPanelLayout {
                x: 35.0,
                y: 45.0,
                width: 76.0,
                height: 22.0,
            }
        );
        assert_eq!(
            pointer_panel_layout((390.0, 290.0), 60.0, (400, 300)),
            PointerPanelLayout {
                x: 299.0,
                y: 253.0,
                width: 76.0,
                height: 22.0,
            }
        );
        assert_eq!(
            pointer_panel_layout((2.0, 2.0), 120.0, (80, 20)),
            PointerPanelLayout {
                x: 6.0,
                y: 6.0,
                width: 68.0,
                height: 8.0,
            }
        );
    }

    #[test]
    fn selected_area_is_cut_out_of_the_scrim() {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_capture_picker(
            &ctx,
            40,
            40,
            RegionCapturePickerVisual {
                selection: Some(RegionSelection {
                    start: (10.0, 10.0),
                    end: (30.0, 30.0),
                }),
                pointer: (30.0, 30.0),
                measurement: None,
                show_scrim: true,
                show_legend: false,
                loupe: None,
                action_bar: None,
                hovered_action: None,
                include_drawings: false,
                window: RegionCaptureWindowVisual::disabled(),
            },
            |_x, _y| None,
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        let alpha = |x: usize, y: usize| data[y * stride + x * 4 + 3];
        assert!(alpha(2, 2) > 0, "outside the selection must be scrimmed");
        assert_eq!(alpha(20, 20), 0, "the selected pixels must remain clear");
    }

    #[test]
    fn highlighted_window_is_cut_out_in_window_mode() {
        let targets = [
            RegionSelection {
                start: (4.0, 4.0),
                end: (14.0, 14.0),
            },
            RegionSelection {
                start: (20.0, 20.0),
                end: (36.0, 36.0),
            },
        ];
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_capture_picker(
            &ctx,
            40,
            40,
            RegionCapturePickerVisual {
                selection: None,
                pointer: (28.0, 28.0),
                measurement: None,
                show_scrim: true,
                show_legend: false,
                loupe: None,
                action_bar: None,
                hovered_action: None,
                include_drawings: false,
                window: RegionCaptureWindowVisual {
                    available: true,
                    active: true,
                    targets: &targets,
                    highlighted_target: Some(1),
                },
            },
            |_x, _y| None,
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        let alpha = |x: usize, y: usize| data[y * stride + x * 4 + 3];
        assert!(alpha(1, 1) > 0, "outside windows remains scrimmed");
        assert!(alpha(9, 9) > 0, "an unhighlighted window remains scrimmed");
        assert_eq!(alpha(28, 28), 0, "highlighted window is the clear target");
    }

    #[test]
    fn window_mode_omits_the_area_crosshair() {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_capture_picker(
            &ctx,
            40,
            40,
            RegionCapturePickerVisual {
                selection: None,
                pointer: (20.0, 20.0),
                measurement: None,
                show_scrim: false,
                show_legend: false,
                loupe: None,
                action_bar: None,
                hovered_action: None,
                include_drawings: false,
                window: RegionCaptureWindowVisual {
                    available: true,
                    active: true,
                    targets: &[],
                    highlighted_target: None,
                },
            },
            |_x, _y| None,
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        assert_eq!(data[20 * stride + 20 * 4 + 3], 0);
    }

    #[test]
    fn highlighted_window_outline_is_stronger_than_other_targets() {
        let targets = [
            RegionSelection {
                start: (4.0, 4.0),
                end: (16.0, 16.0),
            },
            RegionSelection {
                start: (24.0, 4.0),
                end: (36.0, 16.0),
            },
        ];
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 20).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_capture_picker(
            &ctx,
            40,
            20,
            RegionCapturePickerVisual {
                selection: None,
                pointer: (20.0, 18.0),
                measurement: None,
                show_scrim: false,
                show_legend: false,
                loupe: None,
                action_bar: None,
                hovered_action: None,
                include_drawings: false,
                window: RegionCaptureWindowVisual {
                    available: true,
                    active: true,
                    targets: &targets,
                    highlighted_target: Some(1),
                },
            },
            |_x, _y| None,
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        let edge_alpha = |x: usize| {
            (3..=6)
                .flat_map(|y| ((x - 1)..=(x + 1)).map(move |sample_x| (sample_x, y)))
                .map(|(sample_x, y)| u32::from(data[y * stride + sample_x * 4 + 3]))
                .sum::<u32>()
        };
        assert!(
            edge_alpha(24) > edge_alpha(4),
            "the highlighted candidate must be visually stronger"
        );
    }

    #[test]
    fn measure_visual_leaves_the_screen_unscrimmed() {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_capture_picker(
            &ctx,
            40,
            40,
            RegionCapturePickerVisual {
                selection: None,
                pointer: (20.0, 20.0),
                measurement: Some("20, 20"),
                show_scrim: false,
                show_legend: false,
                loupe: None,
                action_bar: None,
                hovered_action: None,
                include_drawings: false,
                window: RegionCaptureWindowVisual::disabled(),
            },
            |_x, _y| None,
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        assert_eq!(data[2 * stride + 2 * 4 + 3], 0, "no measure scrim");
        assert!(data[20 * stride + 20 * 4 + 3] > 0, "measure crosshair");
    }

    #[test]
    fn measure_damage_uses_thin_chrome_regions_instead_of_the_full_surface() {
        let damage = measure_picker_damage(
            Some(RegionSelection {
                start: (100.0, 80.0),
                end: (700.0, 500.0),
            }),
            (400.0, 300.0),
            (800, 600),
        );

        assert!(damage.len() >= 7);
        assert!(damage.iter().all(|rect| {
            rect.x >= 0
                && rect.y >= 0
                && rect.x + rect.width <= 800
                && rect.y + rect.height <= 600
                && (rect.width < 800 || rect.height < 600)
        }));
        assert!(
            damage
                .iter()
                .any(|rect| rect.width == 800 && rect.height <= 5)
        );
        assert!(
            damage
                .iter()
                .any(|rect| rect.height == 600 && rect.width <= 5)
        );
    }

    #[test]
    fn crosshair_remains_visible_while_selecting() {
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_capture_picker(
            &ctx,
            40,
            40,
            RegionCapturePickerVisual {
                selection: Some(RegionSelection {
                    start: (10.0, 10.0),
                    end: (30.0, 30.0),
                }),
                pointer: (20.0, 20.0),
                measurement: None,
                show_scrim: true,
                show_legend: false,
                loupe: None,
                action_bar: None,
                hovered_action: None,
                include_drawings: false,
                window: RegionCaptureWindowVisual::disabled(),
            },
            |_x, _y| None,
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        assert!(
            data[20 * stride + 20 * 4 + 3] > 0,
            "crosshair must be painted inside the clear selection"
        );
    }

    #[test]
    fn review_visual_composes_the_action_bar_after_the_scrim() {
        let selection = RegionSelection {
            start: (100.0, 100.0),
            end: (300.0, 200.0),
        };
        let bar = crate::ui::RegionActionBar::place(selection, (800, 600));
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_capture_picker(
            &ctx,
            800,
            600,
            RegionCapturePickerVisual {
                selection: Some(selection),
                pointer: (200.0, 150.0),
                measurement: Some("200 × 100"),
                show_scrim: true,
                show_legend: false,
                loupe: None,
                action_bar: Some(bar),
                hovered_action: Some(crate::ui::RegionAction::Both),
                include_drawings: true,
                window: RegionCaptureWindowVisual::disabled(),
            },
            |_x, _y| None,
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        assert!(data[212 * stride + 54 * 4 + 3] > 0, "action bar surface");
        assert!(
            data[150 * stride + 200 * 4 + 3] > 0,
            "review keeps the pointer crosshair with its size readout"
        );
    }

    #[test]
    fn capture_loupe_reuses_the_pixel_loupe_renderer_when_enabled() {
        let visual = RegionCaptureLoupeVisual::when_enabled(true, (20.0, 30.0), (50.0, 50.0))
            .expect("enabled immutable option");
        assert!(
            RegionCaptureLoupeVisual::when_enabled(false, (20.0, 30.0), (50.0, 50.0),).is_none()
        );

        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 200).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_capture_loupe(&ctx, (200, 200), visual, |_x, _y| {
            Some(crate::draw::Color::new(1.0, 0.0, 0.0, 1.0))
        });
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        assert!(data[98 * stride + 88 * 4 + 3] > 0, "loupe center pixel");
        assert_eq!(data[5 * stride + 5 * 4 + 3], 0, "outside untouched");
    }
}
