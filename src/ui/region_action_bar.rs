use crate::input::state::RegionSelection;
use crate::ui::theme::{self, Rgba, overlay};
use crate::ui_text::{UiTextStyle, text_layout};

use super::primitives::{draw_keycap, draw_rounded_rect, keycap_size};

const SURFACE_MARGIN: f64 = 8.0;
const SELECTION_GAP: f64 = 12.0;
const BAR_PADDING: f64 = 8.0;
const ITEM_GAP: f64 = 6.0;
/// Vertical gap between the action row and the drawings toggle. The hairline
/// divider is centred in it.
const ROW_GAP: f64 = 8.0;
const ACTION_ROW_HEIGHT: f64 = 38.0;
const TOGGLE_ROW_HEIGHT: f64 = 26.0;
const BAR_HEIGHT: f64 = BAR_PADDING * 2.0 + ACTION_ROW_HEIGHT + ROW_GAP + TOGGLE_ROW_HEIGHT;
/// Resting width of one action control. The bar sizes itself from this instead
/// of stretching controls across an arbitrary fixed width.
const ACTION_ITEM_WIDTH: f64 = 74.0;
const BAR_WIDTH: f64 = BAR_PADDING * 2.0 + ACTION_ITEM_WIDTH * 4.0 + ITEM_GAP * 3.0;

const BAR_RADIUS: f64 = overlay::RADIUS_PANEL;
const ITEM_RADIUS: f64 = overlay::RADIUS_MD;
/// Downward-only two-layer drop shadow, matching the command palette frame, so
/// the bar reads as floating above the frozen screenshot rather than painted
/// into it.
const SHADOW_OFFSET: f64 = 8.0;
const SHADOW_SOFT: Rgba = (0.0, 0.0, 0.0, 0.20);

const LABEL_FONT_SIZE: f64 = 11.0;
const KEYCAP_FONT_SIZE: f64 = 8.5;
const TOGGLE_FONT_SIZE: f64 = 10.5;
/// Gap between an action's label and the keycap chip under it.
const LABEL_KEYCAP_GAP: f64 = 3.0;

const ITEM_BG: Rgba = (1.0, 1.0, 1.0, 0.06);
const ITEM_BORDER: Rgba = (1.0, 1.0, 1.0, 0.10);
const ITEM_BG_HOVER: Rgba = overlay::BG_HOVER;
const ITEM_BORDER_HOVER: Rgba = overlay::BORDER_FOCUS;
/// `Both` is what Enter does, so it carries the accent as the bar's default
/// action. Resting alpha stays below full so hover still reads as a change.
const PRIMARY_BG: Rgba = theme::rgba(theme::ACCENT_RGB, 0.80);
const PRIMARY_BG_HOVER: Rgba = theme::rgba(theme::ACCENT_RGB, 1.0);
const PRIMARY_BORDER: Rgba = theme::rgba(theme::ACCENT_BRIGHT_RGB, 0.45);

const KEYCAP_BG: Rgba = (1.0, 1.0, 1.0, 0.10);
const KEYCAP_BG_ON_ACCENT: Rgba = (1.0, 1.0, 1.0, 0.20);

const CHECKBOX_SIZE: f64 = 14.0;
const CHECKBOX_BORDER: Rgba = (1.0, 1.0, 1.0, 0.38);
const CHECKBOX_BG: Rgba = (1.0, 1.0, 1.0, 0.06);
const CHECKBOX_BG_CHECKED: Rgba = theme::rgba(theme::ACCENT_RGB, 0.95);
const TOGGLE_BG_HOVER: Rgba = (1.0, 1.0, 1.0, 0.07);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegionAction {
    Copy,
    Save,
    Both,
    Board,
    ToggleIncludeDrawings,
}

impl RegionAction {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Save => "Save",
            Self::Both => "Both",
            Self::Board => "Board",
            Self::ToggleIncludeDrawings => "Include drawings in exports",
        }
    }

    pub(crate) const fn shortcut(self) -> &'static str {
        match self {
            Self::Copy => "Ctrl+C",
            Self::Save => "Ctrl+S",
            Self::Both => "Enter",
            Self::Board => "B",
            Self::ToggleIncludeDrawings => "D",
        }
    }

    /// The accented default action: the one `Enter` submits.
    const fn is_primary(self) -> bool {
        matches!(self, Self::Both)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionActionRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl RegionActionRect {
    pub(crate) const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn contains(self, point: (f64, f64)) -> bool {
        point.0 >= self.x
            && point.0 < self.x + self.width
            && point.1 >= self.y
            && point.1 < self.y + self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RegionActionItem {
    action: RegionAction,
    bounds: RegionActionRect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionActionBar {
    bounds: RegionActionRect,
    items: [RegionActionItem; 4],
    toggle: RegionActionItem,
}

impl RegionActionBar {
    pub(crate) fn place(selection: RegionSelection, surface: (u32, u32)) -> Self {
        let left = selection.start.0.min(selection.end.0);
        let right = selection.start.0.max(selection.end.0);
        let top = selection.start.1.min(selection.end.1);
        let bottom = selection.start.1.max(selection.end.1);
        let surface_width = f64::from(surface.0);
        let surface_height = f64::from(surface.1);
        let width = BAR_WIDTH.min((surface_width - SURFACE_MARGIN * 2.0).max(0.0));
        let height = BAR_HEIGHT.min((surface_height - SURFACE_MARGIN * 2.0).max(0.0));
        let x = ((left + right - width) / 2.0).clamp(
            SURFACE_MARGIN,
            (surface_width - width - SURFACE_MARGIN).max(SURFACE_MARGIN),
        );
        let below = bottom + SELECTION_GAP;
        let preferred_y = if below + height + SURFACE_MARGIN <= surface_height {
            below
        } else {
            top - SELECTION_GAP - height
        };
        let y = preferred_y.clamp(
            SURFACE_MARGIN,
            (surface_height - height - SURFACE_MARGIN).max(SURFACE_MARGIN),
        );
        let bounds = RegionActionRect::new(x, y, width, height);
        let item_height = ACTION_ROW_HEIGHT.min((height - BAR_PADDING * 2.0).max(0.0));
        let item_count = 4.0;
        let item_width =
            ((width - BAR_PADDING * 2.0 - ITEM_GAP * (item_count - 1.0)) / item_count).max(0.0);
        let item = |index: usize, action| RegionActionItem {
            action,
            bounds: RegionActionRect::new(
                x + BAR_PADDING + index as f64 * (item_width + ITEM_GAP),
                y + BAR_PADDING,
                item_width,
                item_height,
            ),
        };
        let toggle_y = y + BAR_PADDING + item_height + ROW_GAP;
        let toggle = RegionActionItem {
            action: RegionAction::ToggleIncludeDrawings,
            bounds: RegionActionRect::new(
                x + BAR_PADDING,
                toggle_y,
                (width - BAR_PADDING * 2.0).max(0.0),
                (y + height - BAR_PADDING - toggle_y).max(0.0),
            ),
        };
        Self {
            bounds,
            items: [
                item(0, RegionAction::Copy),
                item(1, RegionAction::Save),
                item(2, RegionAction::Both),
                item(3, RegionAction::Board),
            ],
            toggle,
        }
    }

    /// The painted frame, without its drop shadow. The picker uses it to keep
    /// the Review size badge out from under the bar.
    pub(crate) const fn bounds(self) -> RegionActionRect {
        self.bounds
    }

    pub(crate) fn hit(self, point: (f64, f64)) -> Option<RegionAction> {
        self.items
            .iter()
            .find(|item| item.bounds.contains(point))
            .map(|item| item.action)
            .or_else(|| {
                self.toggle
                    .bounds
                    .contains(point)
                    .then_some(self.toggle.action)
            })
    }

    pub(crate) fn contains(self, point: (f64, f64)) -> bool {
        self.bounds.contains(point)
    }
}

pub(crate) fn render_region_action_bar(
    ctx: &cairo::Context,
    bar: RegionActionBar,
    hovered: Option<RegionAction>,
    include_drawings: bool,
) {
    let _ = ctx.save();
    draw_bar_frame(ctx, bar.bounds);

    for item in bar.items {
        draw_action(ctx, item, hovered == Some(item.action));
    }
    draw_row_divider(ctx, bar);
    draw_toggle(ctx, bar.toggle, hovered, include_drawings);
    let _ = ctx.restore();
}

fn draw_bar_frame(ctx: &cairo::Context, bounds: RegionActionRect) {
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }
    for (offset, color) in [
        (SHADOW_OFFSET, SHADOW_SOFT),
        (SHADOW_OFFSET * 0.5, overlay::SHADOW),
    ] {
        theme::set_color(ctx, color);
        draw_rounded_rect(
            ctx,
            bounds.x,
            bounds.y + offset,
            bounds.width,
            bounds.height,
            BAR_RADIUS,
        );
        let _ = ctx.fill();
    }

    theme::set_color(ctx, crate::ui::theme::popup::bg_context_menu());
    draw_rounded_rect(
        ctx,
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        BAR_RADIUS,
    );
    let _ = ctx.fill();

    theme::set_color(ctx, crate::ui::theme::popup::border_context_menu());
    ctx.set_line_width(1.0);
    draw_rounded_rect(
        ctx,
        bounds.x + 0.5,
        bounds.y + 0.5,
        bounds.width - 1.0,
        bounds.height - 1.0,
        BAR_RADIUS - 0.5,
    );
    let _ = ctx.stroke();
}

/// Hairline between the action row and the toggle row, inset from the bar's
/// padding so it reads as a grouping rule rather than a border.
fn draw_row_divider(ctx: &cairo::Context, bar: RegionActionBar) {
    let toggle = bar.toggle.bounds;
    if toggle.width <= 0.0 || toggle.height <= 0.0 {
        return;
    }
    let y = (toggle.y - ROW_GAP / 2.0).floor() + 0.5;
    theme::set_color(ctx, overlay::DIVIDER_LIGHT);
    ctx.set_line_width(1.0);
    ctx.move_to(toggle.x, y);
    ctx.line_to(toggle.x + toggle.width, y);
    let _ = ctx.stroke();
}

fn draw_action(ctx: &cairo::Context, item: RegionActionItem, hovered: bool) {
    if item.bounds.width <= 0.0 || item.bounds.height <= 0.0 {
        return;
    }
    let primary = item.action.is_primary();
    let (fill, border) = match (primary, hovered) {
        (true, false) => (PRIMARY_BG, PRIMARY_BORDER),
        (true, true) => (PRIMARY_BG_HOVER, ITEM_BORDER_HOVER),
        (false, false) => (ITEM_BG, ITEM_BORDER),
        (false, true) => (ITEM_BG_HOVER, ITEM_BORDER_HOVER),
    };

    theme::set_color(ctx, fill);
    draw_rounded_rect(
        ctx,
        item.bounds.x,
        item.bounds.y,
        item.bounds.width,
        item.bounds.height,
        ITEM_RADIUS,
    );
    let _ = ctx.fill();

    theme::set_color(ctx, border);
    ctx.set_line_width(1.0);
    draw_rounded_rect(
        ctx,
        item.bounds.x + 0.5,
        item.bounds.y + 0.5,
        (item.bounds.width - 1.0).max(0.0),
        (item.bounds.height - 1.0).max(0.0),
        (ITEM_RADIUS - 0.5).max(0.0),
    );
    let _ = ctx.stroke();

    draw_action_content(ctx, item, primary);
}

/// Label over keycap, the pair centred as one block so every control's text
/// sits on the same optical line regardless of ascenders or descenders.
fn draw_action_content(ctx: &cairo::Context, item: RegionActionItem, primary: bool) {
    let _ = ctx.save();
    ctx.rectangle(
        item.bounds.x,
        item.bounds.y,
        item.bounds.width,
        item.bounds.height,
    );
    ctx.clip();

    let center_x = item.bounds.x + item.bounds.width / 2.0;
    let label = item.action.label();
    let layout = text_layout(ctx, label_style(), label, None);
    let label_extents = layout.ink_extents();
    let (keycap_width, keycap_height) = keycap_size(ctx, item.action.shortcut(), KEYCAP_FONT_SIZE);

    let stack_height = label_extents.height() + LABEL_KEYCAP_GAP + keycap_height;
    let stack_top = item.bounds.y + (item.bounds.height - stack_height) / 2.0;

    theme::set_color(ctx, overlay::TEXT_PRIMARY);
    layout.show_at_baseline(
        ctx,
        center_x - label_extents.width() / 2.0 - label_extents.x_bearing(),
        stack_top - label_extents.y_bearing(),
    );

    draw_keycap(
        ctx,
        center_x - keycap_width / 2.0,
        stack_top + label_extents.height() + LABEL_KEYCAP_GAP,
        item.action.shortcut(),
        KEYCAP_FONT_SIZE,
        if primary {
            KEYCAP_BG_ON_ACCENT
        } else {
            KEYCAP_BG
        },
        if primary {
            overlay::TEXT_WHITE
        } else {
            overlay::TEXT_HINT
        },
    );
    let _ = ctx.restore();
}

fn draw_toggle(
    ctx: &cairo::Context,
    item: RegionActionItem,
    hovered: Option<RegionAction>,
    checked: bool,
) {
    if item.bounds.width <= 0.0 || item.bounds.height <= 0.0 {
        return;
    }
    let _ = ctx.save();
    ctx.rectangle(
        item.bounds.x,
        item.bounds.y,
        item.bounds.width,
        item.bounds.height,
    );
    ctx.clip();

    // The row itself stays quiet: the checkbox carries the on/off state, so an
    // enabled toggle no longer paints a full-width slab across the bar.
    if hovered == Some(item.action) {
        theme::set_color(ctx, TOGGLE_BG_HOVER);
        draw_rounded_rect(
            ctx,
            item.bounds.x,
            item.bounds.y,
            item.bounds.width,
            item.bounds.height,
            ITEM_RADIUS,
        );
        let _ = ctx.fill();
    }

    let box_size = CHECKBOX_SIZE.min(item.bounds.height - 2.0).max(0.0);
    let box_x = item.bounds.x + 6.0;
    let box_y = item.bounds.y + (item.bounds.height - box_size) / 2.0;
    draw_checkbox(ctx, box_x, box_y, box_size, checked);

    let label = item.action.label();
    let layout = text_layout(ctx, toggle_label_style(), label, None);
    let extents = layout.ink_extents();
    theme::set_color(
        ctx,
        if checked {
            overlay::TEXT_PRIMARY
        } else {
            overlay::TEXT_TERTIARY
        },
    );
    layout.show_at_baseline(
        ctx,
        box_x + box_size + 8.0 - extents.x_bearing(),
        item.bounds.y + (item.bounds.height - extents.height()) / 2.0 - extents.y_bearing(),
    );

    let (keycap_width, keycap_height) = keycap_size(ctx, item.action.shortcut(), KEYCAP_FONT_SIZE);
    draw_keycap(
        ctx,
        item.bounds.x + item.bounds.width - 6.0 - keycap_width,
        item.bounds.y + (item.bounds.height - keycap_height) / 2.0,
        item.action.shortcut(),
        KEYCAP_FONT_SIZE,
        KEYCAP_BG,
        overlay::TEXT_HINT,
    );
    let _ = ctx.restore();
}

fn draw_checkbox(ctx: &cairo::Context, x: f64, y: f64, size: f64, checked: bool) {
    if size <= 0.0 {
        return;
    }
    theme::set_color(
        ctx,
        if checked {
            CHECKBOX_BG_CHECKED
        } else {
            CHECKBOX_BG
        },
    );
    draw_rounded_rect(ctx, x, y, size, size, overlay::RADIUS_SM);
    let _ = ctx.fill();

    theme::set_color(
        ctx,
        if checked {
            theme::rgba(theme::ACCENT_BRIGHT_RGB, 0.8)
        } else {
            CHECKBOX_BORDER
        },
    );
    ctx.set_line_width(1.0);
    draw_rounded_rect(
        ctx,
        x + 0.5,
        y + 0.5,
        (size - 1.0).max(0.0),
        (size - 1.0).max(0.0),
        (overlay::RADIUS_SM - 0.5).max(0.0),
    );
    let _ = ctx.stroke();

    if !checked {
        return;
    }
    theme::set_color(ctx, overlay::TEXT_WHITE);
    ctx.set_line_width((size * 0.14).max(1.4));
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.move_to(x + size * 0.26, y + size * 0.52);
    ctx.line_to(x + size * 0.44, y + size * 0.70);
    ctx.line_to(x + size * 0.76, y + size * 0.32);
    let _ = ctx.stroke();
}

fn label_style() -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: LABEL_FONT_SIZE,
    }
}

fn toggle_label_style() -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: TOGGLE_FONT_SIZE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_bar_prefers_below_then_flips_above_and_clamps_to_the_surface() {
        let centered = RegionActionBar::place(
            RegionSelection {
                start: (100.0, 100.0),
                end: (300.0, 200.0),
            },
            (800, 600),
        );
        assert_eq!(
            centered.bounds(),
            RegionActionRect::new(35.0, 212.0, 330.0, 88.0)
        );

        let flipped = RegionActionBar::place(
            RegionSelection {
                start: (730.0, 560.0),
                end: (790.0, 590.0),
            },
            (800, 600),
        );
        assert_eq!(
            flipped.bounds(),
            RegionActionRect::new(462.0, 460.0, 330.0, 88.0)
        );
    }

    #[test]
    fn action_bar_hit_returns_typed_controls_and_rejects_gaps() {
        let bar = RegionActionBar::place(
            RegionSelection {
                start: (100.0, 100.0),
                end: (300.0, 200.0),
            },
            (800, 600),
        );

        assert_eq!(bar.hit((80.0, 239.0)), Some(RegionAction::Copy));
        assert_eq!(bar.hit((160.0, 239.0)), Some(RegionAction::Save));
        assert_eq!(bar.hit((240.0, 239.0)), Some(RegionAction::Both));
        assert_eq!(bar.hit((320.0, 239.0)), Some(RegionAction::Board));
        assert_eq!(
            bar.hit((200.0, 279.0)),
            Some(RegionAction::ToggleIncludeDrawings)
        );
        assert_eq!(bar.hit((119.0, 239.0)), None, "inter-item gap");
        assert!(bar.contains((119.0, 239.0)), "bar gaps stay modal-owned");
        assert_eq!(bar.hit((20.0, 20.0)), None, "outside the bar");
        assert!(!bar.contains((20.0, 20.0)));
    }

    #[test]
    fn action_bar_rows_never_overlap_and_stay_inside_the_padded_frame() {
        let bar = RegionActionBar::place(
            RegionSelection {
                start: (100.0, 100.0),
                end: (300.0, 200.0),
            },
            (800, 600),
        );
        let bounds = bar.bounds();
        let toggle = bar.toggle.bounds;

        for item in bar.items {
            assert!(item.bounds.y >= bounds.y + BAR_PADDING);
            assert!(item.bounds.y + item.bounds.height <= toggle.y - ROW_GAP + f64::EPSILON);
            assert!(item.bounds.x >= bounds.x + BAR_PADDING);
            assert!(item.bounds.x + item.bounds.width <= bounds.x + bounds.width - BAR_PADDING);
            assert_eq!(item.bounds.width, ACTION_ITEM_WIDTH);
        }
        assert!(toggle.y + toggle.height <= bounds.y + bounds.height - BAR_PADDING);
    }

    #[test]
    fn action_bar_exposes_the_requested_labels_and_shortcuts() {
        assert_eq!(RegionAction::Copy.label(), "Copy");
        assert_eq!(RegionAction::Copy.shortcut(), "Ctrl+C");
        assert_eq!(RegionAction::Save.label(), "Save");
        assert_eq!(RegionAction::Save.shortcut(), "Ctrl+S");
        assert_eq!(RegionAction::Both.label(), "Both");
        assert_eq!(RegionAction::Both.shortcut(), "Enter");
        assert_eq!(RegionAction::Board.label(), "Board");
        assert_eq!(RegionAction::Board.shortcut(), "B");
        assert_eq!(
            RegionAction::ToggleIncludeDrawings.label(),
            "Include drawings in exports"
        );
        assert_eq!(RegionAction::ToggleIncludeDrawings.shortcut(), "D");
    }

    #[test]
    fn enter_is_the_only_accented_default_action() {
        assert!(RegionAction::Both.is_primary());
        for action in [
            RegionAction::Copy,
            RegionAction::Save,
            RegionAction::Board,
            RegionAction::ToggleIncludeDrawings,
        ] {
            assert!(!action.is_primary(), "{action:?} must stay neutral");
        }
    }

    #[test]
    fn rendering_paints_the_bar_and_each_control() {
        let bar = RegionActionBar::place(
            RegionSelection {
                start: (100.0, 100.0),
                end: (300.0, 200.0),
            },
            (800, 600),
        );
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_action_bar(&ctx, bar, Some(RegionAction::Both), true);
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        let alpha = |x: usize, y: usize| data[y * stride + x * 4 + 3];

        assert!(alpha(40, 250) > 0, "bar surface");
        for x in [80, 160, 240, 320] {
            assert!(alpha(x, 239) > 0, "control at x={x}");
        }
        assert!(alpha(56, 279) > 0, "checked drawings checkbox");
        assert_eq!(alpha(20, 20), 0, "outside remains untouched");
    }

    #[test]
    fn the_drawings_checkbox_carries_the_state_instead_of_a_full_width_slab() {
        let bar = RegionActionBar::place(
            RegionSelection {
                start: (100.0, 100.0),
                end: (300.0, 200.0),
            },
            (800, 600),
        );
        let row_alpha = |checked: bool| {
            let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
            let ctx = cairo::Context::new(&surface).unwrap();
            render_region_action_bar(&ctx, bar, None, checked);
            drop(ctx);
            surface.flush();
            let stride = surface.stride() as usize;
            let data = surface.data().unwrap();
            // Far right of the toggle row, clear of the checkbox, the label and
            // the keycap chip: unfilled in both states.
            (
                u32::from(data[279 * stride + 300 * 4 + 3]),
                u32::from(data[279 * stride + 56 * 4 + 3]),
            )
        };

        let (off_row, off_box) = row_alpha(false);
        let (on_row, on_box) = row_alpha(true);
        assert_eq!(off_row, on_row, "the row background must not change");
        assert!(on_box > 0 && off_box > 0, "the box is drawn either way");
    }
}
