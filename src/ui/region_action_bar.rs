use crate::input::state::RegionSelection;

use super::primitives::{draw_rounded_rect, text_extents_for};

const SURFACE_MARGIN: f64 = 8.0;
const SELECTION_GAP: f64 = 10.0;
const BAR_WIDTH: f64 = 296.0;
const BAR_HEIGHT: f64 = 44.0;
const BAR_PADDING: f64 = 6.0;
const ITEM_GAP: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegionAction {
    Copy,
    Save,
    Both,
    Board,
}

impl RegionAction {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Save => "Save",
            Self::Both => "Both",
            Self::Board => "Board",
        }
    }

    pub(crate) const fn shortcut(self) -> &'static str {
        match self {
            Self::Copy => "Ctrl+C",
            Self::Save => "Ctrl+S",
            Self::Both => "Enter",
            Self::Board => "B",
        }
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
        let item_height = (height - BAR_PADDING * 2.0).max(0.0);
        let item_width = ((width - BAR_PADDING * 2.0 - ITEM_GAP * 3.0) / 4.0).max(0.0);
        let item = |index: usize, action| RegionActionItem {
            action,
            bounds: RegionActionRect::new(
                x + BAR_PADDING + index as f64 * (item_width + ITEM_GAP),
                y + BAR_PADDING,
                item_width,
                item_height,
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
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn bounds(self) -> RegionActionRect {
        self.bounds
    }

    pub(crate) fn hit(self, point: (f64, f64)) -> Option<RegionAction> {
        self.items
            .iter()
            .find(|item| item.bounds.contains(point))
            .map(|item| item.action)
    }

    pub(crate) fn contains(self, point: (f64, f64)) -> bool {
        self.bounds.contains(point)
    }
}

pub(crate) fn render_region_action_bar(
    ctx: &cairo::Context,
    bar: RegionActionBar,
    hovered: Option<RegionAction>,
) {
    let _ = ctx.save();
    ctx.set_source_rgba(0.045, 0.05, 0.065, 0.96);
    draw_rounded_rect(
        ctx,
        bar.bounds.x,
        bar.bounds.y,
        bar.bounds.width,
        bar.bounds.height,
        9.0,
    );
    let _ = ctx.fill();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.18);
    ctx.set_line_width(1.0);
    draw_rounded_rect(
        ctx,
        bar.bounds.x + 0.5,
        bar.bounds.y + 0.5,
        bar.bounds.width - 1.0,
        bar.bounds.height - 1.0,
        8.5,
    );
    let _ = ctx.stroke();

    for item in bar.items {
        if item.bounds.width <= 0.0 || item.bounds.height <= 0.0 {
            continue;
        }
        let is_hovered = hovered == Some(item.action);
        if is_hovered {
            ctx.set_source_rgba(0.24, 0.48, 1.0, 0.34);
        } else {
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.07);
        }
        draw_rounded_rect(
            ctx,
            item.bounds.x,
            item.bounds.y,
            item.bounds.width,
            item.bounds.height,
            6.0,
        );
        let _ = ctx.fill();
        draw_action_text(ctx, item);
    }
    let _ = ctx.restore();
}

fn draw_action_text(ctx: &cairo::Context, item: RegionActionItem) {
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
    let label_extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
        10.5,
        label,
    );
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.94);
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(10.5);
    ctx.move_to(
        center_x - label_extents.width() / 2.0 - label_extents.x_bearing(),
        item.bounds.y + 13.0 - label_extents.y_bearing(),
    );
    let _ = ctx.show_text(label);

    let shortcut = item.action.shortcut();
    let shortcut_extents = text_extents_for(
        ctx,
        "Sans",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        8.0,
        shortcut,
    );
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.58);
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(8.0);
    ctx.move_to(
        center_x - shortcut_extents.width() / 2.0 - shortcut_extents.x_bearing(),
        item.bounds.y + 25.0 - shortcut_extents.y_bearing(),
    );
    let _ = ctx.show_text(shortcut);
    let _ = ctx.restore();
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
            RegionActionRect::new(52.0, 210.0, 296.0, 44.0)
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
            RegionActionRect::new(496.0, 506.0, 296.0, 44.0)
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

        assert_eq!(bar.hit((92.0, 232.0)), Some(RegionAction::Copy));
        assert_eq!(bar.hit((164.0, 232.0)), Some(RegionAction::Save));
        assert_eq!(bar.hit((236.0, 232.0)), Some(RegionAction::Both));
        assert_eq!(bar.hit((308.0, 232.0)), Some(RegionAction::Board));
        assert_eq!(bar.hit((128.0, 232.0)), None, "inter-item gap");
        assert!(bar.contains((128.0, 232.0)), "bar gaps stay modal-owned");
        assert_eq!(bar.hit((20.0, 20.0)), None, "outside the bar");
        assert!(!bar.contains((20.0, 20.0)));
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
        render_region_action_bar(&ctx, bar, Some(RegionAction::Both));
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        let alpha = |x: usize, y: usize| data[y * stride + x * 4 + 3];

        assert!(alpha(54, 212) > 0, "bar surface");
        for x in [92, 164, 236, 308] {
            assert!(alpha(x, 232) > 0, "control at x={x}");
        }
        assert_eq!(alpha(20, 20), 0, "outside remains untouched");
    }
}
