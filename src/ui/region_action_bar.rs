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
const EDIT_ROW_HEIGHT: f64 = 28.0;
const TOGGLE_ROW_HEIGHT: f64 = 26.0;
const STATUS_ROW_HEIGHT: f64 = 16.0;
const BAR_HEIGHT: f64 = BAR_PADDING * 2.0
    + ACTION_ROW_HEIGHT
    + ROW_GAP
    + EDIT_ROW_HEIGHT
    + ROW_GAP
    + TOGGLE_ROW_HEIGHT
    + ROW_GAP
    + STATUS_ROW_HEIGHT;
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
    CutBand,
    UndoCut,
    RedoCut,
    ResetCuts,
    ToggleIncludeDrawings,
}

impl RegionAction {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Save => "Save",
            Self::Both => "Both",
            Self::Board => "Board",
            Self::CutBand => "Cut",
            Self::UndoCut => "Undo",
            Self::RedoCut => "Redo",
            Self::ResetCuts => "Reset",
            Self::ToggleIncludeDrawings => "Include drawings in exports",
        }
    }

    pub(crate) const fn shortcut(self) -> &'static str {
        match self {
            Self::Copy => "Ctrl+C",
            Self::Save => "Ctrl+S",
            Self::Both => "Enter",
            Self::Board => "B",
            Self::CutBand => "X",
            Self::UndoCut => "Ctrl+Z",
            Self::RedoCut => "Ctrl+Y",
            Self::ResetCuts => "",
            Self::ToggleIncludeDrawings => "D",
        }
    }

    /// Destinations that leave Review. Edit controls stay in the picker.
    pub(crate) const fn is_terminal(self) -> bool {
        matches!(self, Self::Copy | Self::Save | Self::Both | Self::Board)
    }

    /// The accented default action: the one `Enter` submits.
    const fn is_primary(self) -> bool {
        matches!(self, Self::Both)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegionActionAvailability {
    pub terminal: bool,
    pub cut: bool,
    pub undo: bool,
    pub redo: bool,
    pub reset: bool,
}

impl RegionActionAvailability {
    /// Resting Review bar: terminals and Cut enabled, history empty.
    pub(crate) const DEFAULT: Self = Self {
        terminal: true,
        cut: true,
        undo: false,
        redo: false,
        reset: false,
    };

    pub(crate) const fn allows(self, action: RegionAction) -> bool {
        match action {
            RegionAction::Copy | RegionAction::Save | RegionAction::Both | RegionAction::Board => {
                self.terminal
            }
            RegionAction::CutBand => self.cut,
            RegionAction::UndoCut => self.undo,
            RegionAction::RedoCut => self.redo,
            RegionAction::ResetCuts => self.reset,
            RegionAction::ToggleIncludeDrawings => true,
        }
    }
}

impl Default for RegionActionAvailability {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegionCutStatus {
    Updating,
    Failed,
}

impl RegionCutStatus {
    const fn message(self) -> &'static str {
        match self {
            Self::Updating => "Updating cut preview…",
            Self::Failed => "Cut preview failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegionActionBarVisual {
    pub hovered: Option<RegionAction>,
    pub include_drawings: bool,
    pub availability: RegionActionAvailability,
    pub cut_armed: bool,
    pub status: Option<RegionCutStatus>,
}

#[cfg(test)]
impl RegionActionBarVisual {
    pub(crate) const fn simple(hovered: Option<RegionAction>, include_drawings: bool) -> Self {
        Self {
            hovered,
            include_drawings,
            availability: RegionActionAvailability::DEFAULT,
            cut_armed: false,
            status: None,
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
    edit: [RegionActionItem; 4],
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
        let width_scale = if BAR_WIDTH > 0.0 {
            (width / BAR_WIDTH).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let height_scale = if BAR_HEIGHT > 0.0 {
            (height / BAR_HEIGHT).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let pad_x = BAR_PADDING * width_scale;
        let pad_y = BAR_PADDING * height_scale;
        let item_gap = ITEM_GAP * width_scale;
        let row_gap = ROW_GAP * height_scale;
        let action_height = ACTION_ROW_HEIGHT * height_scale;
        let edit_height = EDIT_ROW_HEIGHT * height_scale;
        let toggle_height = TOGGLE_ROW_HEIGHT * height_scale;
        let item_count = 4.0;
        let item_width =
            ((width - pad_x * 2.0 - item_gap * (item_count - 1.0)) / item_count).max(0.0);
        let row_item = |index: usize, action, row_y, row_height| RegionActionItem {
            action,
            bounds: RegionActionRect::new(
                x + pad_x + index as f64 * (item_width + item_gap),
                row_y,
                item_width,
                row_height,
            ),
        };
        let action_y = y + pad_y;
        let edit_y = action_y + action_height + row_gap;
        let toggle_y = edit_y + edit_height + row_gap;
        let toggle = RegionActionItem {
            action: RegionAction::ToggleIncludeDrawings,
            bounds: RegionActionRect::new(
                x + pad_x,
                toggle_y,
                (width - pad_x * 2.0).max(0.0),
                toggle_height,
            ),
        };
        Self {
            bounds,
            items: [
                row_item(0, RegionAction::Copy, action_y, action_height),
                row_item(1, RegionAction::Save, action_y, action_height),
                row_item(2, RegionAction::Both, action_y, action_height),
                row_item(3, RegionAction::Board, action_y, action_height),
            ],
            edit: [
                row_item(0, RegionAction::CutBand, edit_y, edit_height),
                row_item(1, RegionAction::UndoCut, edit_y, edit_height),
                row_item(2, RegionAction::RedoCut, edit_y, edit_height),
                row_item(3, RegionAction::ResetCuts, edit_y, edit_height),
            ],
            toggle,
        }
    }

    /// The painted frame, without its drop shadow. The picker uses it to keep
    /// the Review size badge out from under the bar.
    pub(crate) const fn bounds(&self) -> RegionActionRect {
        self.bounds
    }

    pub(crate) fn hit(&self, point: (f64, f64)) -> Option<RegionAction> {
        self.items
            .iter()
            .chain(self.edit.iter())
            .find(|item| item.bounds.contains(point))
            .map(|item| item.action)
            .or_else(|| {
                self.toggle
                    .bounds
                    .contains(point)
                    .then_some(self.toggle.action)
            })
    }

    pub(crate) fn enabled_hit(
        &self,
        point: (f64, f64),
        availability: RegionActionAvailability,
    ) -> Option<RegionAction> {
        self.hit(point)
            .filter(|&action| availability.allows(action))
    }

    pub(crate) fn contains(&self, point: (f64, f64)) -> bool {
        self.bounds.contains(point)
    }

    fn status_bounds(&self) -> Option<RegionActionRect> {
        let toggle = self.toggle.bounds;
        if toggle.width <= 0.0 {
            return None;
        }
        let pad_y = (self.items[0].bounds.y - self.bounds.y).max(0.0);
        let row_gap =
            (self.edit[0].bounds.y - self.items[0].bounds.y - self.items[0].bounds.height).max(0.0);
        let y = toggle.y + toggle.height + row_gap;
        let height = (self.bounds.y + self.bounds.height - pad_y - y).max(0.0);
        (height > 0.0).then(|| RegionActionRect::new(toggle.x, y, toggle.width, height))
    }
}

pub(crate) fn render_region_action_bar(
    ctx: &cairo::Context,
    bar: &RegionActionBar,
    visual: RegionActionBarVisual,
) {
    let _ = ctx.save();
    draw_bar_frame(ctx, bar.bounds);

    for &item in &bar.items {
        draw_action(
            ctx,
            item,
            visual.hovered == Some(item.action),
            visual.availability.allows(item.action),
            false,
        );
    }
    draw_row_divider(
        ctx,
        bar.items[0].bounds,
        bar.edit[0].bounds.y,
        bar.toggle.bounds.width,
    );
    for &item in &bar.edit {
        draw_action(
            ctx,
            item,
            visual.hovered == Some(item.action),
            visual.availability.allows(item.action),
            visual.cut_armed && item.action == RegionAction::CutBand,
        );
    }
    draw_row_divider(
        ctx,
        bar.edit[0].bounds,
        bar.toggle.bounds.y,
        bar.toggle.bounds.width,
    );
    draw_toggle(ctx, bar.toggle, visual.hovered, visual.include_drawings);
    draw_status(ctx, bar, visual.status);
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

/// Hairline between stacked rows, inset from the bar's padding so it reads as
/// a grouping rule rather than a border.
fn draw_row_divider(ctx: &cairo::Context, row: RegionActionRect, next_y: f64, width: f64) {
    if width <= 0.0 || row.height <= 0.0 {
        return;
    }
    let gap = (next_y - row.y - row.height).max(0.0);
    if gap <= 0.0 {
        return;
    }
    let y = (row.y + row.height + gap / 2.0).floor() + 0.5;
    theme::set_color(ctx, overlay::DIVIDER_LIGHT);
    ctx.set_line_width(1.0);
    ctx.move_to(row.x, y);
    ctx.line_to(row.x + width, y);
    let _ = ctx.stroke();
}

fn draw_action(
    ctx: &cairo::Context,
    item: RegionActionItem,
    hovered: bool,
    enabled: bool,
    selected: bool,
) {
    if item.bounds.width <= 0.0 || item.bounds.height <= 0.0 {
        return;
    }
    let primary = item.action.is_primary();
    let (mut fill, mut border) = match (primary, hovered || selected) {
        (true, false) => (PRIMARY_BG, PRIMARY_BORDER),
        (true, true) => (PRIMARY_BG_HOVER, ITEM_BORDER_HOVER),
        (false, false) => (ITEM_BG, ITEM_BORDER),
        (false, true) => (ITEM_BG_HOVER, ITEM_BORDER_HOVER),
    };
    if selected && !primary {
        fill = theme::rgba(theme::ACCENT_RGB, 0.35);
        border = ITEM_BORDER_HOVER;
    }
    if !enabled {
        fill.3 *= 0.45;
        border.3 *= 0.45;
    }

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

    draw_action_content(ctx, item, primary, enabled);
}

/// Label over keycap, the pair centred as one block so every control's text
/// sits on the same optical line regardless of ascenders or descenders.
fn draw_action_content(ctx: &cairo::Context, item: RegionActionItem, primary: bool, enabled: bool) {
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
    let shortcut = item.action.shortcut();
    let (keycap_width, keycap_height) = if shortcut.is_empty() {
        (0.0, 0.0)
    } else {
        keycap_size(ctx, shortcut, KEYCAP_FONT_SIZE)
    };

    let stack_height = if shortcut.is_empty() {
        label_extents.height()
    } else {
        label_extents.height() + LABEL_KEYCAP_GAP + keycap_height
    };
    let stack_top = item.bounds.y + (item.bounds.height - stack_height) / 2.0;
    let text_color = if !enabled {
        overlay::TEXT_TERTIARY
    } else if primary {
        overlay::TEXT_WHITE
    } else {
        overlay::TEXT_PRIMARY
    };

    theme::set_color(ctx, text_color);
    layout.show_at_baseline(
        ctx,
        center_x - label_extents.width() / 2.0 - label_extents.x_bearing(),
        stack_top - label_extents.y_bearing(),
    );

    if !shortcut.is_empty() {
        draw_keycap(
            ctx,
            center_x - keycap_width / 2.0,
            stack_top + label_extents.height() + LABEL_KEYCAP_GAP,
            shortcut,
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
    }
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

fn draw_status(ctx: &cairo::Context, bar: &RegionActionBar, status: Option<RegionCutStatus>) {
    let Some(status) = status else {
        return;
    };
    let Some(row) = bar.status_bounds() else {
        return;
    };
    let font_size = (TOGGLE_FONT_SIZE * (row.height / STATUS_ROW_HEIGHT).min(1.0)).max(0.0);
    if font_size < 1.0 {
        return;
    }
    let _ = ctx.save();
    ctx.rectangle(row.x, row.y, row.width, row.height);
    ctx.clip();
    let layout = text_layout(ctx, status_label_style(font_size), status.message(), None);
    let extents = layout.ink_extents();
    theme::set_color(
        ctx,
        match status {
            RegionCutStatus::Updating => overlay::TEXT_HINT,
            RegionCutStatus::Failed => overlay::TEXT_PRIMARY,
        },
    );
    layout.show_at_baseline(
        ctx,
        row.x + (row.width - extents.width()) / 2.0 - extents.x_bearing(),
        row.y + (row.height - extents.height()) / 2.0 - extents.y_bearing(),
    );
    let _ = ctx.restore();
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
    status_label_style(TOGGLE_FONT_SIZE)
}

fn status_label_style(size: f64) -> UiTextStyle<'static> {
    UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bar() -> RegionActionBar {
        RegionActionBar::place(
            RegionSelection {
                start: (100.0, 100.0),
                end: (300.0, 200.0),
            },
            (800, 600),
        )
    }

    fn rect_inside(inner: RegionActionRect, outer: RegionActionRect) -> bool {
        inner.x + f64::EPSILON >= outer.x
            && inner.y + f64::EPSILON >= outer.y
            && inner.x + inner.width <= outer.x + outer.width + f64::EPSILON
            && inner.y + inner.height <= outer.y + outer.height + f64::EPSILON
    }

    fn assert_controls_stay_inside_bar(bar: &RegionActionBar) {
        let bounds = bar.bounds();
        for item in bar.items.iter().chain(bar.edit.iter()) {
            assert!(
                rect_inside(item.bounds, bounds),
                "{:?} at ({}, {}) {}x{} leaves bar {bounds:?}",
                item.action,
                item.bounds.x,
                item.bounds.y,
                item.bounds.width,
                item.bounds.height
            );
        }
        assert!(
            rect_inside(bar.toggle.bounds, bounds),
            "toggle leaves bar {bounds:?}"
        );
        for row in [&bar.items[..], &bar.edit[..]] {
            for pair in row.windows(2) {
                assert!(
                    pair[0].bounds.x + pair[0].bounds.width <= pair[1].bounds.x + f64::EPSILON,
                    "{:?} overlaps {:?}",
                    pair[0].action,
                    pair[1].action
                );
            }
        }
        assert!(
            bar.items[0].bounds.y + bar.items[0].bounds.height
                <= bar.edit[0].bounds.y + f64::EPSILON
        );
        assert!(
            bar.edit[0].bounds.y + bar.edit[0].bounds.height <= bar.toggle.bounds.y + f64::EPSILON
        );
    }

    #[test]
    fn action_bar_prefers_below_then_flips_above_and_clamps_to_the_surface() {
        let centered = sample_bar();
        assert_eq!(
            centered.bounds(),
            RegionActionRect::new(35.0, 212.0, 330.0, BAR_HEIGHT)
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
            RegionActionRect::new(462.0, 560.0 - SELECTION_GAP - BAR_HEIGHT, 330.0, BAR_HEIGHT)
        );
    }

    #[test]
    fn action_bar_hit_returns_typed_controls_and_rejects_gaps() {
        let bar = sample_bar();
        let action_y = bar.items[0].bounds.y + bar.items[0].bounds.height / 2.0;
        let edit_y = bar.edit[0].bounds.y + bar.edit[0].bounds.height / 2.0;
        let toggle_y = bar.toggle.bounds.y + bar.toggle.bounds.height / 2.0;

        assert_eq!(bar.hit((80.0, action_y)), Some(RegionAction::Copy));
        assert_eq!(bar.hit((160.0, action_y)), Some(RegionAction::Save));
        assert_eq!(bar.hit((240.0, action_y)), Some(RegionAction::Both));
        assert_eq!(bar.hit((320.0, action_y)), Some(RegionAction::Board));
        assert_eq!(bar.hit((80.0, edit_y)), Some(RegionAction::CutBand));
        assert_eq!(bar.hit((160.0, edit_y)), Some(RegionAction::UndoCut));
        assert_eq!(bar.hit((240.0, edit_y)), Some(RegionAction::RedoCut));
        assert_eq!(bar.hit((320.0, edit_y)), Some(RegionAction::ResetCuts));
        assert_eq!(
            bar.hit((200.0, toggle_y)),
            Some(RegionAction::ToggleIncludeDrawings)
        );
        assert_eq!(bar.hit((119.0, action_y)), None, "inter-item gap");
        assert!(bar.contains((119.0, action_y)), "bar gaps stay modal-owned");
        assert_eq!(bar.hit((20.0, 20.0)), None, "outside the bar");
        assert!(!bar.contains((20.0, 20.0)));
    }

    #[test]
    fn disabled_controls_still_consume_the_bar_but_return_no_enabled_action() {
        let bar = sample_bar();
        let availability = RegionActionAvailability {
            terminal: false,
            cut: true,
            undo: false,
            redo: false,
            reset: false,
        };
        let action_y = bar.items[0].bounds.y + bar.items[0].bounds.height / 2.0;
        assert_eq!(bar.hit((80.0, action_y)), Some(RegionAction::Copy));
        assert_eq!(bar.enabled_hit((80.0, action_y), availability), None);
        assert!(bar.contains((80.0, action_y)));
        assert_eq!(
            bar.enabled_hit(
                (
                    bar.edit[0].bounds.x + bar.edit[0].bounds.width / 2.0,
                    bar.edit[0].bounds.y + bar.edit[0].bounds.height / 2.0
                ),
                availability
            ),
            Some(RegionAction::CutBand)
        );
    }

    #[test]
    fn action_bar_rows_never_overlap_and_stay_inside_the_padded_frame() {
        let bar = sample_bar();
        let bounds = bar.bounds();
        let toggle = bar.toggle.bounds;

        for item in bar.items {
            assert!(item.bounds.y >= bounds.y + BAR_PADDING);
            assert!(
                item.bounds.y + item.bounds.height <= bar.edit[0].bounds.y - ROW_GAP + f64::EPSILON
            );
            assert!(item.bounds.x >= bounds.x + BAR_PADDING);
            assert!(item.bounds.x + item.bounds.width <= bounds.x + bounds.width - BAR_PADDING);
            assert_eq!(item.bounds.width, ACTION_ITEM_WIDTH);
        }
        for item in bar.edit {
            assert!(item.bounds.y >= bar.items[0].bounds.y + bar.items[0].bounds.height);
            assert!(item.bounds.y + item.bounds.height <= toggle.y - ROW_GAP + f64::EPSILON);
            assert_eq!(item.bounds.width, ACTION_ITEM_WIDTH);
        }
        assert!(toggle.y + toggle.height <= bounds.y + bounds.height - BAR_PADDING);
    }

    #[test]
    fn narrow_and_short_surfaces_keep_controls_inside_the_bar() {
        let selection = RegionSelection {
            start: (10.0, 10.0),
            end: (40.0, 30.0),
        };
        for surface in [(200, 80), (80, 40), (40, 600), (800, 36)] {
            let bar = RegionActionBar::place(selection, surface);
            assert_controls_stay_inside_bar(&bar);
            let action = bar.items[0].bounds;
            if action.width > 1.0 && action.height > 1.0 {
                assert_eq!(
                    bar.hit((
                        action.x + action.width / 2.0,
                        action.y + action.height / 2.0
                    )),
                    Some(RegionAction::Copy),
                    "typed hit on {surface:?}"
                );
            }
        }
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
        assert_eq!(RegionAction::CutBand.label(), "Cut");
        assert_eq!(RegionAction::CutBand.shortcut(), "X");
        assert_eq!(RegionAction::UndoCut.shortcut(), "Ctrl+Z");
        assert_eq!(RegionAction::RedoCut.shortcut(), "Ctrl+Y");
        assert_eq!(
            RegionAction::ToggleIncludeDrawings.label(),
            "Include drawings in exports"
        );
        assert_eq!(RegionAction::ToggleIncludeDrawings.shortcut(), "D");
        assert!(RegionAction::Copy.is_terminal());
        assert!(!RegionAction::CutBand.is_terminal());
        assert!(!RegionAction::ToggleIncludeDrawings.is_terminal());
    }

    #[test]
    fn enter_is_the_only_accented_default_action() {
        assert!(RegionAction::Both.is_primary());
        for action in [
            RegionAction::Copy,
            RegionAction::Save,
            RegionAction::Board,
            RegionAction::CutBand,
            RegionAction::UndoCut,
            RegionAction::RedoCut,
            RegionAction::ResetCuts,
            RegionAction::ToggleIncludeDrawings,
        ] {
            assert!(!action.is_primary(), "{action:?} must stay neutral");
        }
    }

    #[test]
    fn rendering_paints_the_bar_and_each_control() {
        let bar = sample_bar();
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_action_bar(
            &ctx,
            &bar,
            RegionActionBarVisual::simple(Some(RegionAction::Both), true),
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        let alpha = |x: usize, y: usize| data[y * stride + x * 4 + 3];
        let action_y = (bar.items[0].bounds.y + bar.items[0].bounds.height / 2.0) as usize;
        let toggle_y = (bar.toggle.bounds.y + bar.toggle.bounds.height / 2.0) as usize;

        assert!(alpha(40, action_y) > 0, "bar surface");
        for x in [80, 160, 240, 320] {
            assert!(alpha(x, action_y) > 0, "control at x={x}");
        }
        assert!(alpha(56, toggle_y) > 0, "checked drawings checkbox");
        assert_eq!(alpha(20, 20), 0, "outside remains untouched");
    }

    #[test]
    fn the_drawings_checkbox_carries_the_state_instead_of_a_full_width_slab() {
        let bar = sample_bar();
        let toggle_y = (bar.toggle.bounds.y + bar.toggle.bounds.height / 2.0) as usize;
        let row_alpha = |checked: bool| {
            let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
            let ctx = cairo::Context::new(&surface).unwrap();
            render_region_action_bar(&ctx, &bar, RegionActionBarVisual::simple(None, checked));
            drop(ctx);
            surface.flush();
            let stride = surface.stride() as usize;
            let data = surface.data().unwrap();
            (
                u32::from(data[toggle_y * stride + 300 * 4 + 3]),
                u32::from(data[toggle_y * stride + 56 * 4 + 3]),
            )
        };

        let (off_row, off_box) = row_alpha(false);
        let (on_row, on_box) = row_alpha(true);
        assert_eq!(off_row, on_row, "the row background must not change");
        assert!(on_box > 0 && off_box > 0, "the box is drawn either way");
    }

    #[test]
    fn updating_and_failed_preview_states_paint_status_text() {
        let bar = sample_bar();
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 600).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_action_bar(
            &ctx,
            &bar,
            RegionActionBarVisual {
                hovered: None,
                include_drawings: false,
                availability: RegionActionAvailability {
                    terminal: false,
                    cut: true,
                    undo: true,
                    redo: false,
                    reset: true,
                },
                cut_armed: true,
                status: Some(RegionCutStatus::Updating),
            },
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        let status = bar.status_bounds().unwrap();
        let status_y = (status.y + status.height / 2.0) as usize;
        let alpha = data[status_y * stride + 200 * 4 + 3];
        assert!(alpha > 0, "status caption is visible");
    }

    fn paint_bar(
        width: i32,
        height: i32,
        bar: RegionActionBar,
        status: Option<RegionCutStatus>,
    ) -> (usize, Vec<u8>) {
        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_action_bar(
            &ctx,
            &bar,
            RegionActionBarVisual {
                hovered: None,
                include_drawings: false,
                availability: RegionActionAvailability {
                    terminal: false,
                    cut: true,
                    undo: true,
                    redo: false,
                    reset: true,
                },
                cut_armed: false,
                status,
            },
        );
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let pixels = surface.data().unwrap().to_vec();
        (stride, pixels)
    }

    #[test]
    fn short_surface_status_paint_stays_inside_its_row() {
        let selection = RegionSelection {
            start: (10.0, 10.0),
            end: (40.0, 30.0),
        };
        for surface in [(800, 36), (800, 100), (200, 80)] {
            let bar = RegionActionBar::place(selection, surface);
            let width = i32::try_from(surface.0).unwrap();
            let height = i32::try_from(surface.1).unwrap();
            let (stride, without) = paint_bar(width, height, bar, None);
            let (_, with_status) = paint_bar(width, height, bar, Some(RegionCutStatus::Failed));
            let row = bar.status_bounds();
            for y in 0..surface.1 as usize {
                for x in 0..surface.0 as usize {
                    let offset = y * stride + x * 4;
                    if without[offset..offset + 4] == with_status[offset..offset + 4] {
                        continue;
                    }
                    let Some(row) = row else {
                        panic!("status painted with no status row on {surface:?}");
                    };
                    assert!(
                        row.contains((x as f64 + 0.5, y as f64 + 0.5)),
                        "status paint at ({x}, {y}) left the {row:?} row on {surface:?}"
                    );
                    assert!(
                        bar.bounds.contains((x as f64 + 0.5, y as f64 + 0.5)),
                        "status paint at ({x}, {y}) left the bar on {surface:?}"
                    );
                }
            }
        }
    }
}
