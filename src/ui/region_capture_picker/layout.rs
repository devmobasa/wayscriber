use crate::input::state::RegionSelection;
use crate::ui::region_action_bar::RegionActionRect;
use crate::util::Rect;

const POINTER_GAP: f64 = 15.0;
/// Gap between the reviewed selection and its size badge. The badge is parked
/// on the selection during Review instead of trailing the pointer, so a
/// finished rectangle stops behaving like one that is still being dragged.
const SELECTION_BADGE_GAP: f64 = 6.0;
const PANEL_MARGIN: f64 = 6.0;
pub(super) const PANEL_PADDING_X: f64 = 8.0;
const PANEL_HEIGHT: f64 = 22.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PointerPanelLayout {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
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

pub(super) fn normalized_rect(selection: RegionSelection) -> (f64, f64, f64, f64) {
    let x = selection.start.0.min(selection.end.0);
    let y = selection.start.1.min(selection.end.1);
    (
        x,
        y,
        (selection.end.0 - selection.start.0).abs(),
        (selection.end.1 - selection.start.1).abs(),
    )
}

pub(super) fn pointer_panel_layout(
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

/// Park the size badge on the selection's top-left corner. The action bar is
/// painted after the badge, and it flips above the selection when it does not
/// fit below, so each placement is checked against the bar and skipped rather
/// than drawn under it. Candidates run outside-above, inside-top-left,
/// inside-top-right; inside-top-left is the fallback when a clamped bar covers
/// all three.
pub(super) fn selection_badge_layout(
    rect: (f64, f64, f64, f64),
    text_width: f64,
    action_bar: Option<RegionActionRect>,
    screen: (u32, u32),
) -> PointerPanelLayout {
    let screen_width = f64::from(screen.0);
    let screen_height = f64::from(screen.1);
    let width =
        (text_width + PANEL_PADDING_X * 2.0).min((screen_width - PANEL_MARGIN * 2.0).max(0.0));
    let height = PANEL_HEIGHT.min((screen_height - PANEL_MARGIN * 2.0).max(0.0));
    let (rect_x, rect_y, rect_width, ..) = rect;
    let clamp_x = |x: f64| {
        x.clamp(
            PANEL_MARGIN,
            (screen_width - width - PANEL_MARGIN).max(PANEL_MARGIN),
        )
    };
    let clamp_y = |y: f64| {
        y.clamp(
            PANEL_MARGIN,
            (screen_height - height - PANEL_MARGIN).max(PANEL_MARGIN),
        )
    };
    let left = clamp_x(rect_x);
    let right = clamp_x(rect_x + rect_width - width);
    let inside = clamp_y(rect_y + SELECTION_BADGE_GAP);
    let above = rect_y - SELECTION_BADGE_GAP - height;
    let fallback = (left, inside);
    let (x, y) = [
        (above >= PANEL_MARGIN).then(|| (left, clamp_y(above))),
        Some(fallback),
        Some((right, inside)),
    ]
    .into_iter()
    .flatten()
    .find(|&(x, y)| !covered_by_action_bar(x, y, width, height, action_bar))
    .unwrap_or(fallback);
    PointerPanelLayout {
        x,
        y,
        width,
        height,
    }
}

pub(super) fn covered_by_action_bar(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    action_bar: Option<RegionActionRect>,
) -> bool {
    action_bar.is_some_and(|bar| {
        x < bar.x + bar.width && bar.x < x + width && y < bar.y + bar.height && bar.y < y + height
    })
}
