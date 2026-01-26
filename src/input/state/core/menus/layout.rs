use super::super::base::InputState;
use super::types::{ContextMenuCursorHint, ContextMenuLayout, ContextMenuState};
use crate::ui_text::{UiTextStyle, text_layout};
use crate::util::Rect;
use cairo::Context as CairoContext;

impl InputState {
    /// Returns cached context menu layout, if available.
    pub fn context_menu_layout(&self) -> Option<&ContextMenuLayout> {
        self.context_menu_layout.as_ref()
    }

    /// Clears cached layout data (used when menu closes).
    pub fn clear_context_menu_layout(&mut self) {
        self.context_menu_layout = None;
        self.pending_menu_hover_recalc = false;
    }

    /// Recomputes context menu layout for rendering and hit-testing.
    pub fn update_context_menu_layout(
        &mut self,
        ctx: &CairoContext,
        screen_width: u32,
        screen_height: u32,
    ) {
        if !self.is_context_menu_open() {
            self.context_menu_layout = None;
            return;
        }

        let entries = self.context_menu_entries();
        if entries.is_empty() {
            self.context_menu_layout = None;
            return;
        }

        const FONT_SIZE: f64 = 14.0;
        const ROW_HEIGHT: f64 = 24.0;
        const PADDING_X: f64 = 12.0;
        const PADDING_Y: f64 = 8.0;
        const GAP_BETWEEN_COLUMNS: f64 = 20.0;
        const ARROW_WIDTH: f64 = 10.0;

        let _ = ctx.save();
        let text_style = UiTextStyle {
            family: "Sans",
            slant: cairo::FontSlant::Normal,
            weight: cairo::FontWeight::Normal,
            size: FONT_SIZE,
        };

        let mut max_label_width: f64 = 0.0;
        let mut max_shortcut_width: f64 = 0.0;
        for entry in &entries {
            let extents = text_layout(ctx, text_style, &entry.label, None).ink_extents();
            max_label_width = max_label_width.max(extents.width());
            if let Some(shortcut) = &entry.shortcut {
                let extents = text_layout(ctx, text_style, shortcut, None).ink_extents();
                max_shortcut_width = max_shortcut_width.max(extents.width());
            }
        }

        let _ = ctx.restore();

        let menu_width = PADDING_X * 2.0
            + max_label_width
            + GAP_BETWEEN_COLUMNS
            + max_shortcut_width
            + ARROW_WIDTH;
        let menu_height = PADDING_Y * 2.0 + ROW_HEIGHT * entries.len() as f64;

        let mut origin_x = match &self.context_menu_state {
            ContextMenuState::Open { anchor, .. } => anchor.0 as f64,
            ContextMenuState::Hidden => 0.0,
        };
        let mut origin_y = match &self.context_menu_state {
            ContextMenuState::Open { anchor, .. } => anchor.1 as f64,
            ContextMenuState::Hidden => 0.0,
        };

        let screen_w = screen_width as f64;
        let screen_h = screen_height as f64;
        if origin_x + menu_width > screen_w - 6.0 {
            origin_x = (screen_w - menu_width - 6.0).max(6.0);
        }
        if origin_y + menu_height > screen_h - 6.0 {
            origin_y = (screen_h - menu_height - 6.0).max(6.0);
        }

        self.context_menu_layout = Some(ContextMenuLayout {
            origin_x,
            origin_y,
            width: menu_width,
            height: menu_height,
            row_height: ROW_HEIGHT,
            font_size: FONT_SIZE,
            padding_x: PADDING_X,
            padding_y: PADDING_Y,
            shortcut_width: max_shortcut_width,
            arrow_width: ARROW_WIDTH,
        });

        if self.pending_menu_hover_recalc {
            let focus_set = matches!(
                self.context_menu_state,
                ContextMenuState::Open {
                    keyboard_focus: Some(_),
                    ..
                }
            );
            if !focus_set {
                let (px, py) = self.last_pointer_position;
                self.update_context_menu_hover_from_pointer_internal(px, py, false);
            }
            self.pending_menu_hover_recalc = false;
        }

        if let Some(layout) = self.context_menu_layout {
            self.mark_context_menu_region(layout);
        }
    }

    /// Maps pointer coordinates to a context menu entry index, if applicable.
    pub fn context_menu_index_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.context_menu_layout()?;
        let entries = self.context_menu_entries();
        if entries.is_empty() {
            return None;
        }

        let local_x = x as f64 - layout.origin_x;
        let local_y = y as f64 - layout.origin_y;

        if local_x < 0.0 || local_y < 0.0 || local_x > layout.width || local_y > layout.height {
            return None;
        }

        let row = ((local_y - layout.padding_y) / layout.row_height).floor() as isize;
        if row < 0 {
            return None;
        }

        let index = row as usize;
        if index >= entries.len() {
            None
        } else {
            Some(index)
        }
    }

    /// Determine the cursor type for a given point within the context menu.
    /// Returns `None` if the context menu is not open or the point is outside.
    pub fn context_menu_cursor_hint_at(&self, x: i32, y: i32) -> Option<ContextMenuCursorHint> {
        if !self.is_context_menu_open() {
            return None;
        }
        let layout = self.context_menu_layout()?;

        let local_x = x as f64 - layout.origin_x;
        let local_y = y as f64 - layout.origin_y;

        // Check if within panel bounds
        if local_x < 0.0 || local_y < 0.0 || local_x > layout.width || local_y > layout.height {
            return None;
        }

        // Check if hovering over a menu item (not disabled)
        if let Some(index) = self.context_menu_index_at(x, y) {
            let entries = self.context_menu_entries();
            if let Some(entry) = entries.get(index)
                && !entry.disabled
            {
                return Some(ContextMenuCursorHint::Pointer);
            }
        }

        Some(ContextMenuCursorHint::Default)
    }

    pub(super) fn mark_context_menu_region(&mut self, layout: ContextMenuLayout) {
        // Add margin for border stroke and anti-aliasing
        let margin = 4;
        let x = layout.origin_x.floor() as i32 - margin;
        let y = layout.origin_y.floor() as i32 - margin;
        let width = layout.width.ceil() as i32 + margin * 2;
        // Include navigation hint area below the menu:
        // hint_y = layout.height + 4.0, hint_height = font_size * 0.8 + 6.0 * 2.0
        let hint_extra = 4.0 + layout.font_size * 0.8 + 12.0 + 4.0; // gap + hint + padding
        let height = (layout.height + hint_extra).ceil() as i32 + margin * 2;
        let width = width.max(1);
        let height = height.max(1);

        if let Some(rect) = Rect::new(x, y, width, height) {
            self.dirty_tracker.mark_rect(rect);
        } else {
            self.dirty_tracker.mark_full();
        }
    }
}
