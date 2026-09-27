use std::time::Instant;

use super::super::base::InputState;
use super::types::{
    ContextMenuCursorHint, ContextMenuEntry, ContextMenuLayout, ContextMenuLevel, ContextMenuState,
    SubmenuSide,
};
use crate::ui::theme::overlay::{NAV_HINT_MENU, NAV_HINT_MENU_SUBMENUS, NAV_HINT_SUBMENU};
use crate::ui_text::{UiTextEngine, UiTextStyle};

const FONT_SIZE: f64 = 14.0;
/// The key-hint footer sits inside the menu box, at a readable size.
const FOOTER_FONT_SIZE: f64 = 12.0;
const FOOTER_HEIGHT: f64 = 26.0;
const ROW_HEIGHT: f64 = 24.0;
const PADDING_X: f64 = 12.0;
const PADDING_Y: f64 = 8.0;
const GAP_BETWEEN_COLUMNS: f64 = 20.0;
const ARROW_WIDTH: f64 = 10.0;
/// Space between a menu and the submenu beside it.
const SUBMENU_GAP: f64 = 4.0;
/// Menus keep this distance from the output edges.
const SCREEN_MARGIN: f64 = 6.0;

impl InputState {
    /// Returns cached context menu layout, if available.
    pub fn context_menu_layout(&self) -> Option<&ContextMenuLayout> {
        self.context_menu.layout()
    }

    /// Returns the cached layout of the open submenu, if any.
    pub fn context_submenu_layout(&self) -> Option<&ContextMenuLayout> {
        self.context_menu.submenu_layout()
    }

    /// The key hint the open menu's footer shows.
    pub fn context_menu_footer_hint(&self) -> &'static str {
        context_menu_footer_hint(
            &self.context_menu_entries(),
            self.context_submenu_is_active(),
        )
    }

    /// The side submenus open on for the current layout. Arrows on parent
    /// rows point this way.
    pub fn context_submenu_side(&self) -> SubmenuSide {
        self.context_menu.submenu_side()
    }

    /// Clears cached layout data (used when menu closes).
    pub fn clear_context_menu_layout(&mut self) {
        self.context_menu.clear_layout();
        self.pointer.clear_menu_hover_recalc();
    }

    /// Recomputes context menu layout for rendering and hit-testing.
    pub fn update_context_menu_layout(&mut self, screen_width: u32, screen_height: u32) {
        self.update_context_menu_layout_with_engine(
            &UiTextEngine::default(),
            screen_width,
            screen_height,
        );
    }

    /// Lays out the menu and any open submenu. The backend calls this once per
    /// frame before painting, so damage, painting, and hit-testing share it.
    pub(crate) fn update_context_menu_layout_with_engine(
        &mut self,
        engine: &UiTextEngine,
        screen_width: u32,
        screen_height: u32,
    ) {
        let screen = (f64::from(screen_width), f64::from(screen_height));
        let ContextMenuState::Open { anchor, .. } = &self.context_menu.state else {
            self.context_menu.clear_layout();
            return;
        };
        let anchor = *anchor;
        let entries = self.context_menu_entries();
        let footer_hints = context_menu_footer_hints(&entries);
        let Some(mut root) = measure_menu(engine, &entries, footer_hints) else {
            self.context_menu.clear_layout();
            return;
        };
        root.origin_x = fit_within(f64::from(anchor.0), root.width, screen.0);
        root.origin_y = fit_within(f64::from(anchor.1), root.height, screen.1);
        self.context_menu.layout = Some(root);

        // Hover can open a submenu, so settle it before laying one out.
        if self.pointer.take_menu_hover_recalc() {
            let focus_set = matches!(
                self.context_menu.state,
                ContextMenuState::Open {
                    keyboard_focus: Some(_),
                    ..
                }
            );
            if !focus_set {
                let (px, py) = self.pointer.screen();
                self.update_context_menu_hover_from_pointer_internal(px, py, Instant::now(), false);
            }
        }

        let pane = self.context_submenu().and_then(|submenu| {
            Some((
                submenu,
                measure_menu(engine, &self.context_submenu_entries(), &[])?,
            ))
        });
        // Without an open pane, predict the side from one as wide as the menu
        // so parent-row arrows point where a pane would open.
        let pane_width = pane.map_or(root.width, |(_, pane)| pane.width);
        let side = submenu_side(&root, pane_width, screen.0);
        self.context_menu.submenu_side = side;
        self.context_menu.submenu_layout = pane.map(|(submenu, mut pane)| {
            pane.origin_x = match side {
                SubmenuSide::Right => fit_within(
                    root.origin_x + root.width + SUBMENU_GAP,
                    pane.width,
                    screen.0,
                ),
                SubmenuSide::Left => root.origin_x - SUBMENU_GAP - pane.width,
            };
            // Line the first entry up with the parent row.
            let row_top =
                root.origin_y + root.padding_y + root.row_height * submenu.parent_index as f64;
            pane.origin_y = fit_within(row_top - pane.padding_y, pane.height, screen.1);
            pane
        });
    }

    /// Maps pointer coordinates to a context menu entry index, if applicable.
    pub fn context_menu_index_at(&self, x: i32, y: i32) -> Option<usize> {
        self.context_menu_row_at(ContextMenuLevel::Root, x, y)
    }

    /// Maps pointer coordinates to an entry of the open submenu.
    pub fn context_submenu_index_at(&self, x: i32, y: i32) -> Option<usize> {
        self.context_menu_row_at(ContextMenuLevel::Submenu, x, y)
    }

    /// The row of one menu under a point. The layout already encodes the row
    /// count, so this needs no entry list.
    pub(super) fn context_menu_row_at(
        &self,
        level: ContextMenuLevel,
        x: i32,
        y: i32,
    ) -> Option<usize> {
        let layout = self.context_menu_level_layout(level)?;
        let (x, y) = (f64::from(x), f64::from(y));
        if !layout_contains(layout, x, y) {
            return None;
        }
        let row = ((y - layout.origin_y - layout.padding_y) / layout.row_height).floor();
        (row >= 0.0 && row < row_count(layout)).then_some(row as usize)
    }

    fn context_menu_level_layout(&self, level: ContextMenuLevel) -> Option<&ContextMenuLayout> {
        match level {
            ContextMenuLevel::Root => self.context_menu.layout(),
            ContextMenuLevel::Submenu => self.context_menu.submenu_layout(),
        }
    }

    /// The menu under the pointer. The submenu is checked first because it
    /// paints above its parent.
    pub(crate) fn context_menu_level_at(&self, x: i32, y: i32) -> Option<ContextMenuLevel> {
        let (x, y) = (f64::from(x), f64::from(y));
        [ContextMenuLevel::Submenu, ContextMenuLevel::Root]
            .into_iter()
            .find(|level| {
                self.context_menu_level_layout(*level)
                    .is_some_and(|layout| layout_contains(layout, x, y))
            })
    }

    /// Determine the cursor type for a given point within the context menu.
    /// Returns `None` if the context menu is not open or the point is outside.
    pub fn context_menu_cursor_hint_at(&self, x: i32, y: i32) -> Option<ContextMenuCursorHint> {
        if !self.is_context_menu_open() {
            return None;
        }
        let level = self.context_menu_level_at(x, y)?;
        let on_enabled_row = self.context_menu_row_at(level, x, y).is_some_and(|index| {
            self.context_menu_level_entries(level)
                .get(index)
                .is_some_and(|entry| !entry.disabled)
        });
        Some(if on_enabled_row {
            ContextMenuCursorHint::Pointer
        } else {
            ContextMenuCursorHint::Default
        })
    }
}

/// Every key hint a root menu with these entries can show in its footer.
/// The box is sized for the widest, so opening a submenu never resizes it.
fn context_menu_footer_hints(entries: &[ContextMenuEntry]) -> &'static [&'static str] {
    if entries.iter().any(|entry| entry.submenu.is_some()) {
        &[NAV_HINT_MENU_SUBMENUS, NAV_HINT_SUBMENU]
    } else {
        &[NAV_HINT_MENU]
    }
}

/// The key hint a root menu's footer shows right now.
fn context_menu_footer_hint(entries: &[ContextMenuEntry], submenu_active: bool) -> &'static str {
    if submenu_active {
        NAV_HINT_SUBMENU
    } else if entries.iter().any(|entry| entry.submenu.is_some()) {
        NAV_HINT_MENU_SUBMENUS
    } else {
        NAV_HINT_MENU
    }
}

/// Sizes a menu for its entries, placed at the origin. A root menu passes the
/// footer hints it may show; a submenu passes none and gets no footer.
fn measure_menu(
    engine: &UiTextEngine,
    entries: &[ContextMenuEntry],
    footer_hints: &[&str],
) -> Option<ContextMenuLayout> {
    if entries.is_empty() {
        return None;
    }
    let style = |size| UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size,
    };
    let measured_width = |size, text: &str| {
        engine
            .measure(style(size), text, None)
            .map_or(0.0, |extents| extents.width())
    };
    let text_width = |text: &str| measured_width(FONT_SIZE, text);
    let label_width = entries
        .iter()
        .map(|entry| text_width(&entry.label))
        .fold(0.0, f64::max);
    let shortcut_width = entries
        .iter()
        .filter_map(|entry| entry.shortcut.as_deref())
        .map(text_width)
        .fold(0.0, f64::max);

    let footer_width = footer_hints
        .iter()
        .map(|hint| measured_width(FOOTER_FONT_SIZE, hint))
        .fold(0.0, f64::max);
    let footer_height = if footer_hints.is_empty() {
        0.0
    } else {
        FOOTER_HEIGHT
    };
    let rows_width = label_width + GAP_BETWEEN_COLUMNS + shortcut_width + ARROW_WIDTH;

    Some(ContextMenuLayout {
        origin_x: 0.0,
        origin_y: 0.0,
        width: PADDING_X * 2.0 + rows_width.max(footer_width),
        height: PADDING_Y * 2.0 + ROW_HEIGHT * entries.len() as f64 + footer_height,
        row_height: ROW_HEIGHT,
        font_size: FONT_SIZE,
        footer_height,
        footer_font_size: FOOTER_FONT_SIZE,
        padding_x: PADDING_X,
        padding_y: PADDING_Y,
        shortcut_width,
        arrow_width: ARROW_WIDTH,
    })
}

/// The number of rows a menu was measured for.
fn row_count(layout: &ContextMenuLayout) -> f64 {
    ((layout.height - layout.padding_y * 2.0 - layout.footer_height) / layout.row_height).round()
}

/// Submenus open to the right, or to the left when the output edge is in the
/// way and there is room on the left.
fn submenu_side(root: &ContextMenuLayout, pane_width: f64, screen_width: f64) -> SubmenuSide {
    let right = root.origin_x + root.width + SUBMENU_GAP;
    let left = root.origin_x - SUBMENU_GAP - pane_width;
    if right + pane_width <= screen_width - SCREEN_MARGIN || left < SCREEN_MARGIN {
        SubmenuSide::Right
    } else {
        SubmenuSide::Left
    }
}

/// Pulls a span that would cross the far output edge back inside the margin.
fn fit_within(start: f64, length: f64, limit: f64) -> f64 {
    if start + length > limit - SCREEN_MARGIN {
        (limit - length - SCREEN_MARGIN).max(SCREEN_MARGIN)
    } else {
        start
    }
}

fn layout_contains(layout: &ContextMenuLayout, x: f64, y: f64) -> bool {
    let (local_x, local_y) = (x - layout.origin_x, y - layout.origin_y);
    (0.0..=layout.width).contains(&local_x) && (0.0..=layout.height).contains(&local_y)
}
