use super::super::base::InputState;
use crate::config::KeybindingsConfig;

pub const COMMAND_PALETTE_MAX_VISIBLE: usize = 10;

pub(crate) const COMMAND_PALETTE_MIN_WIDTH: f64 = 400.0;
pub(crate) const COMMAND_PALETTE_MAX_WIDTH: f64 = 820.0;
pub(crate) const COMMAND_PALETTE_HORIZONTAL_MARGIN: f64 = 12.0;
pub(crate) const COMMAND_PALETTE_ITEM_HEIGHT: f64 = 32.0;
pub(crate) const COMMAND_PALETTE_PADDING: f64 = 12.0;
pub(crate) const COMMAND_PALETTE_PADDING_BOTTOM: f64 = 48.0;
pub(crate) const COMMAND_PALETTE_INPUT_HEIGHT: f64 = 36.0;
pub(crate) const COMMAND_PALETTE_LIST_GAP: f64 = 8.0;
pub(crate) const COMMAND_PALETTE_ROW_ACTION_SIZE: f64 = 22.0;
pub(crate) const COMMAND_PALETTE_ROW_ACTION_GAP: f64 = 4.0;
pub(crate) const COMMAND_PALETTE_ROW_ACTION_COUNT: usize = 3;
pub(crate) const COMMAND_PALETTE_MAX_HEIGHT: f64 = 420.0;
pub(crate) const COMMAND_PALETTE_TOP_RATIO: f64 = 0.2;
pub(crate) const COMMAND_PALETTE_QUERY_PLACEHOLDER: &str = "Type to search commands...";

const LABEL_LEFT_PAD: f64 = 10.0;
const LABEL_DESC_GAP: f64 = 12.0;
const DESC_BADGE_GAP: f64 = 12.0;
const BADGE_RIGHT_PAD: f64 = 8.0;
const BADGE_TEXT_PADDING_X: f64 = 5.0;
const BADGE_BASE_PADDING: f64 = BADGE_TEXT_PADDING_X * 2.0;
const APPROX_LABEL_CHAR_WIDTH: f64 = 8.4;
const APPROX_DESC_CHAR_WIDTH: f64 = 6.7;
const APPROX_SHORTCUT_CHAR_WIDTH: f64 = 6.8;
const DESC_ESTIMATE_CHAR_CAP: usize = 42;
const QUERY_CHAR_WIDTH: f64 = 8.0;
const QUERY_INPUT_EXTRA_PADDING: f64 = 20.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct CommandPaletteGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub inner_x: f64,
    pub inner_width: f64,
    pub input_top: f64,
    pub input_bottom: f64,
    pub items_top: f64,
    pub visible_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandPaletteRowAction {
    Edit,
    Delete,
    Reset,
}

impl CommandPaletteRowAction {
    pub(crate) fn tooltip(self) -> &'static str {
        match self {
            Self::Edit => "Edit shortcut",
            Self::Delete => "Unbind shortcut",
            Self::Reset => "Reset shortcut to default",
        }
    }
}

impl CommandPaletteGeometry {
    pub(crate) fn local_point(self, x: i32, y: i32) -> (f64, f64) {
        (x as f64 - self.x, y as f64 - self.y)
    }

    pub(crate) fn contains_local(self, local_x: f64, local_y: f64) -> bool {
        (0.0..=self.width).contains(&local_x) && (0.0..=self.height).contains(&local_y)
    }

    pub(crate) fn local_in_input(self, local_x: f64, local_y: f64) -> bool {
        local_y >= self.input_top
            && local_y <= self.input_bottom
            && local_x >= self.inner_x
            && local_x <= self.inner_x + self.inner_width
    }

    pub(crate) fn visible_item_at(self, local_x: f64, local_y: f64) -> Option<usize> {
        if local_x < self.inner_x || local_x > self.inner_x + self.inner_width {
            return None;
        }
        if local_y < self.items_top {
            return None;
        }

        let row = ((local_y - self.items_top) / COMMAND_PALETTE_ITEM_HEIGHT).floor() as usize;
        if row >= self.visible_count {
            return None;
        }

        let item_top = self.items_top + row as f64 * COMMAND_PALETTE_ITEM_HEIGHT;
        let item_bottom = item_top + COMMAND_PALETTE_ITEM_HEIGHT;
        if local_y > item_bottom {
            return None;
        }

        Some(row)
    }

    pub(crate) fn row_action_at(
        self,
        local_x: f64,
        local_y: f64,
    ) -> Option<(usize, CommandPaletteRowAction)> {
        let row = self.visible_item_at(local_x, local_y)?;
        let right = self.inner_x + self.inner_width;
        let stride = COMMAND_PALETTE_ROW_ACTION_SIZE + COMMAND_PALETTE_ROW_ACTION_GAP;
        let actions_left = right - stride * COMMAND_PALETTE_ROW_ACTION_COUNT as f64;
        if local_x < actions_left || local_x > right {
            return None;
        }
        let slot = ((local_x - actions_left) / stride).floor() as usize;
        let within = (local_x - actions_left) % stride;
        if slot >= COMMAND_PALETTE_ROW_ACTION_COUNT || within > COMMAND_PALETTE_ROW_ACTION_SIZE {
            return None;
        }
        let action = match slot {
            0 => CommandPaletteRowAction::Edit,
            1 => CommandPaletteRowAction::Delete,
            2 => CommandPaletteRowAction::Reset,
            _ => return None,
        };
        Some((row, action))
    }
}

pub(crate) fn command_palette_visible_count(total_items: usize) -> usize {
    total_items.min(COMMAND_PALETTE_MAX_VISIBLE)
}

pub(crate) fn command_palette_height(visible_count: usize) -> f64 {
    let items_top =
        COMMAND_PALETTE_PADDING + COMMAND_PALETTE_INPUT_HEIGHT + COMMAND_PALETTE_LIST_GAP;
    let items_height = visible_count as f64 * COMMAND_PALETTE_ITEM_HEIGHT;
    (items_top + items_height + COMMAND_PALETTE_PADDING_BOTTOM).min(COMMAND_PALETTE_MAX_HEIGHT)
}

impl InputState {
    pub fn command_palette_width(&self, screen_width: u32) -> f64 {
        let commands = self.filtered_commands();
        let mut required_inner_width: f64 = 0.0;
        let default_bindings = KeybindingsConfig::default();

        for command in &commands {
            let label_chars = command.label.chars().count() as f64;
            let desc_chars = command
                .description
                .chars()
                .count()
                .min(DESC_ESTIMATE_CHAR_CAP) as f64;
            let shortcut_chars = self
                .action_binding_primary_label(command.action)
                .map_or(0.0, |label| label.chars().count() as f64);

            let mut row_inner = LABEL_LEFT_PAD + label_chars * APPROX_LABEL_CHAR_WIDTH;
            if desc_chars > 0.0 {
                row_inner += LABEL_DESC_GAP + desc_chars * APPROX_DESC_CHAR_WIDTH;
            }
            if shortcut_chars > 0.0 {
                let badge_width = shortcut_chars * APPROX_SHORTCUT_CHAR_WIDTH + BADGE_BASE_PADDING;
                row_inner += DESC_BADGE_GAP + badge_width;
            }
            row_inner += BADGE_RIGHT_PAD;
            if default_bindings
                .bindings_for_action(command.action)
                .is_some()
            {
                row_inner += (COMMAND_PALETTE_ROW_ACTION_SIZE + COMMAND_PALETTE_ROW_ACTION_GAP)
                    * COMMAND_PALETTE_ROW_ACTION_COUNT as f64;
            }
            required_inner_width = required_inner_width.max(row_inner);
        }

        let query_chars =
            self.command_palette_query
                .chars()
                .count()
                .max(COMMAND_PALETTE_QUERY_PLACEHOLDER.chars().count()) as f64;
        let query_inner_width =
            LABEL_LEFT_PAD + query_chars * QUERY_CHAR_WIDTH + QUERY_INPUT_EXTRA_PADDING;
        required_inner_width = required_inner_width.max(query_inner_width);

        let requested_width = required_inner_width + COMMAND_PALETTE_PADDING * 2.0;
        let max_available =
            (screen_width as f64 - COMMAND_PALETTE_HORIZONTAL_MARGIN * 2.0).max(240.0);
        requested_width.clamp(
            COMMAND_PALETTE_MIN_WIDTH.min(max_available),
            COMMAND_PALETTE_MAX_WIDTH.min(max_available),
        )
    }

    pub(crate) fn command_palette_geometry(
        &self,
        screen_width: u32,
        screen_height: u32,
        total_items: usize,
    ) -> CommandPaletteGeometry {
        let width = self.command_palette_width(screen_width);
        let x = (screen_width as f64 - width) / 2.0;
        let y = screen_height as f64 * COMMAND_PALETTE_TOP_RATIO;
        let visible_count = command_palette_visible_count(total_items);

        let input_top = COMMAND_PALETTE_PADDING;
        let input_bottom = input_top + COMMAND_PALETTE_INPUT_HEIGHT;
        let items_top = input_bottom + COMMAND_PALETTE_LIST_GAP;

        CommandPaletteGeometry {
            x,
            y,
            width,
            height: command_palette_height(visible_count),
            inner_x: COMMAND_PALETTE_PADDING,
            inner_width: width - COMMAND_PALETTE_PADDING * 2.0,
            input_top,
            input_bottom,
            items_top,
            visible_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_geometry() -> CommandPaletteGeometry {
        CommandPaletteGeometry {
            x: 100.0,
            y: 200.0,
            width: 400.0,
            height: 300.0,
            inner_x: COMMAND_PALETTE_PADDING,
            inner_width: 376.0,
            input_top: COMMAND_PALETTE_PADDING,
            input_bottom: COMMAND_PALETTE_PADDING + COMMAND_PALETTE_INPUT_HEIGHT,
            items_top: COMMAND_PALETTE_PADDING
                + COMMAND_PALETTE_INPUT_HEIGHT
                + COMMAND_PALETTE_LIST_GAP,
            visible_count: 3,
        }
    }

    #[test]
    fn visible_count_caps_at_max_visible() {
        assert_eq!(command_palette_visible_count(0), 0);
        assert_eq!(command_palette_visible_count(3), 3);
        assert_eq!(
            command_palette_visible_count(99),
            COMMAND_PALETTE_MAX_VISIBLE
        );
    }

    #[test]
    fn command_palette_height_clamps_to_maximum() {
        assert!(command_palette_height(1) < COMMAND_PALETTE_MAX_HEIGHT);
        assert_eq!(
            command_palette_height(COMMAND_PALETTE_MAX_VISIBLE),
            COMMAND_PALETTE_MAX_HEIGHT
        );
    }

    #[test]
    fn contains_local_includes_edges_and_rejects_outside_points() {
        let geometry = sample_geometry();
        assert!(geometry.contains_local(0.0, 0.0));
        assert!(geometry.contains_local(geometry.width, geometry.height));
        assert!(!geometry.contains_local(-0.1, 0.0));
        assert!(!geometry.contains_local(0.0, geometry.height + 0.1));
    }

    #[test]
    fn local_in_input_checks_inner_bounds() {
        let geometry = sample_geometry();
        assert!(geometry.local_in_input(geometry.inner_x + 1.0, geometry.input_top + 1.0));
        assert!(!geometry.local_in_input(geometry.inner_x - 1.0, geometry.input_top + 1.0));
        assert!(!geometry.local_in_input(geometry.inner_x + 1.0, geometry.input_bottom + 1.0));
    }

    #[test]
    fn visible_item_at_maps_rows_and_rejects_outside_items() {
        let geometry = sample_geometry();
        let x = geometry.inner_x + 10.0;

        assert_eq!(
            geometry.visible_item_at(x, geometry.items_top + 1.0),
            Some(0)
        );
        assert_eq!(
            geometry.visible_item_at(x, geometry.items_top + COMMAND_PALETTE_ITEM_HEIGHT + 1.0),
            Some(1)
        );
        assert_eq!(
            geometry.visible_item_at(geometry.inner_x - 1.0, geometry.items_top + 1.0),
            None
        );
        assert_eq!(
            geometry.visible_item_at(
                x,
                geometry.items_top
                    + geometry.visible_count as f64 * COMMAND_PALETTE_ITEM_HEIGHT
                    + 1.0,
            ),
            None
        );
    }

    #[test]
    fn row_action_at_maps_edit_delete_and_reset_slots() {
        let geometry = sample_geometry();
        let stride = COMMAND_PALETTE_ROW_ACTION_SIZE + COMMAND_PALETTE_ROW_ACTION_GAP;
        let left = geometry.inner_x + geometry.inner_width
            - stride * COMMAND_PALETTE_ROW_ACTION_COUNT as f64;
        let y = geometry.items_top + 5.0;

        assert_eq!(
            geometry.row_action_at(left + 2.0, y),
            Some((0, CommandPaletteRowAction::Edit))
        );
        assert_eq!(
            geometry.row_action_at(left + stride + 2.0, y),
            Some((0, CommandPaletteRowAction::Delete))
        );
        assert_eq!(
            geometry.row_action_at(left + stride * 2.0 + 2.0, y),
            Some((0, CommandPaletteRowAction::Reset))
        );
    }
}
