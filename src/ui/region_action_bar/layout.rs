use super::model::{RegionAction, RegionActionAvailability};
use crate::input::state::RegionSelection;

const SURFACE_MARGIN: f64 = 8.0;
pub(super) const SELECTION_GAP: f64 = 12.0;
pub(super) const BAR_PADDING: f64 = 8.0;
const ITEM_GAP: f64 = 6.0;
/// Vertical gap between the action row and the drawings toggle. The hairline
/// divider is centred in it.
pub(super) const ROW_GAP: f64 = 8.0;
const ACTION_ROW_HEIGHT: f64 = 38.0;
const EDIT_ROW_HEIGHT: f64 = 28.0;
const TOGGLE_ROW_HEIGHT: f64 = 26.0;
pub(super) const STATUS_ROW_HEIGHT: f64 = 16.0;
pub(super) const BAR_HEIGHT: f64 = BAR_PADDING * 2.0
    + ACTION_ROW_HEIGHT
    + ROW_GAP
    + EDIT_ROW_HEIGHT
    + ROW_GAP
    + TOGGLE_ROW_HEIGHT
    + ROW_GAP
    + STATUS_ROW_HEIGHT;
/// Resting width of one action control. The bar sizes itself from this instead
/// of stretching controls across an arbitrary fixed width.
pub(super) const ACTION_ITEM_WIDTH: f64 = 74.0;
const BAR_WIDTH: f64 = BAR_PADDING * 2.0 + ACTION_ITEM_WIDTH * 4.0 + ITEM_GAP * 3.0;

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

    pub(super) fn contains(self, point: (f64, f64)) -> bool {
        point.0 >= self.x
            && point.0 < self.x + self.width
            && point.1 >= self.y
            && point.1 < self.y + self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RegionActionItem {
    pub(super) action: RegionAction,
    pub(super) bounds: RegionActionRect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionActionBar {
    pub(super) bounds: RegionActionRect,
    pub(super) items: [RegionActionItem; 4],
    pub(super) edit: [RegionActionItem; 4],
    pub(super) toggle: RegionActionItem,
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

    pub(super) fn status_bounds(&self) -> Option<RegionActionRect> {
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
