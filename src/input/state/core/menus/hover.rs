use std::time::{Duration, Instant};

use super::super::base::InputState;
use super::context_menu::{AimSample, PendingHover};
use super::types::{ContextMenuLevel, ContextMenuState, MenuCommand, SubmenuSide};

/// How long the pointer rests on a parent row before its submenu opens, and
/// on another row before the open submenu closes or switches. Sweeping the
/// pointer down the menu then passes parent rows without flashing their panes.
pub const SUBMENU_HOVER_DELAY: Duration = Duration::from_millis(120);
/// How long a move toward the open submenu keeps it open while the pointer
/// crosses other rows. After this the row under the resting pointer wins.
pub const SUBMENU_AIM_GRACE: Duration = Duration::from_millis(300);

impl InputState {
    pub(super) fn update_context_menu_hover_from_pointer_internal(
        &mut self,
        x: i32,
        y: i32,
        now: Instant,
        trigger_redraw: bool,
    ) {
        if !self.is_context_menu_open() {
            return;
        }
        let point = (f64::from(x), f64::from(y));
        let changed = if self.context_menu_level_at(x, y) == Some(ContextMenuLevel::Submenu) {
            let row = self.context_submenu_index_at(x, y);
            self.context_menu.pending_hover = None;
            let cleared = self.set_context_menu_hover(ContextMenuLevel::Root, None);
            self.set_context_menu_hover(ContextMenuLevel::Submenu, row) || cleared
        } else {
            let row = self.context_menu_index_at(x, y);
            self.update_parent_menu_hover(point, row, Some(now))
        };
        self.context_menu.aim = Some(AimSample { point, at: now });
        if changed && trigger_redraw {
            self.needs_redraw = true;
        }
    }

    /// Hover over the parent menu. Resting on a row with a submenu opens it and
    /// resting on another row closes it, except while the pointer crosses rows
    /// on its way into the open submenu. With `now` the open or close waits
    /// for the pointer to rest; without it the row under the pointer decides
    /// at once.
    fn update_parent_menu_hover(
        &mut self,
        point: (f64, f64),
        row: Option<usize>,
        now: Option<Instant>,
    ) -> bool {
        let mut changed = self.set_context_menu_hover(ContextMenuLevel::Submenu, None);
        let open_parent = self.context_submenu().map(|submenu| submenu.parent_index);
        if let Some(now) = now
            && row.is_some()
            && open_parent.is_some()
            && row != open_parent
            && self.pointer_is_heading_into_submenu(point, now)
        {
            // Bound the grace so a pointer that stops here still settles.
            self.context_menu.pending_hover = Some(PendingHover {
                due: now + SUBMENU_AIM_GRACE,
            });
            return changed;
        }

        changed |= self.set_context_menu_hover(ContextMenuLevel::Root, row);
        let Some(row) = row else {
            // Off every row: leave the submenu as it is. A collapsed parent
            // row opens again once the pointer has been away from it.
            self.context_menu.pending_hover = None;
            self.context_menu.hover_open_suppressed = None;
            return changed;
        };
        // The pointer decides the target now; keyboard focus inside the
        // submenu would otherwise compete with it for Enter.
        self.set_context_menu_level_focus(ContextMenuLevel::Submenu, None);
        if self.context_menu.hover_open_suppressed != Some(row) {
            self.context_menu.hover_open_suppressed = None;
        }
        if open_parent == Some(row) || self.context_menu.hover_open_suppressed == Some(row) {
            self.context_menu.pending_hover = None;
            return changed;
        }
        match now {
            Some(now) => {
                self.context_menu.pending_hover = Some(PendingHover {
                    due: now + SUBMENU_HOVER_DELAY,
                });
            }
            None => {
                changed |= match self.context_menu_row_submenu(row) {
                    Some(kind) => self.open_context_submenu_kind(row, kind, false),
                    None => self.close_context_submenu(false),
                };
            }
        }
        changed
    }

    /// When the next hover deadline needs the event loop awake.
    pub fn context_menu_hover_timeout(&self, now: Instant) -> Option<Duration> {
        self.context_menu
            .pending_hover
            .map(|pending| pending.due.saturating_duration_since(now))
    }

    /// Event-loop pump: once a hover deadline passes, the row under the resting
    /// pointer decides which submenu is open. Returns whether it fired.
    pub fn tick_context_menu_hover(&mut self, now: Instant) -> bool {
        let due = self
            .context_menu
            .pending_hover
            .is_some_and(|pending| now >= pending.due);
        if !due || !self.is_context_menu_open() {
            return false;
        }
        self.context_menu.pending_hover = None;
        let (x, y) = self.pointer.screen();
        if self.context_menu_level_at(x, y) != Some(ContextMenuLevel::Root) {
            return true;
        }
        let point = (f64::from(x), f64::from(y));
        let row = self.context_menu_index_at(x, y);
        if self.update_parent_menu_hover(point, row, None) {
            self.needs_redraw = true;
        }
        true
    }

    /// Whether the latest pointer move stays inside the triangle between its
    /// fresh previous position and the near edge of the open submenu.
    fn pointer_is_heading_into_submenu(&self, point: (f64, f64), now: Instant) -> bool {
        let (Some(aim), Some(pane)) = (self.context_menu.aim, self.context_menu.submenu_layout)
        else {
            return false;
        };
        if now.saturating_duration_since(aim.at) > SUBMENU_AIM_GRACE {
            return false;
        }
        let edge_x = match self.context_menu.submenu_side {
            SubmenuSide::Right => pane.origin_x,
            SubmenuSide::Left => pane.origin_x + pane.width,
        };
        point_in_triangle(
            point,
            aim.point,
            (edge_x, pane.origin_y),
            (edge_x, pane.origin_y + pane.height),
        )
    }

    /// Moves one menu's hover. Landing on a row clears that menu's keyboard
    /// focus. Returns whether the hover changed.
    fn set_context_menu_hover(&mut self, level: ContextMenuLevel, row: Option<usize>) -> bool {
        let Some((hover_index, keyboard_focus)) =
            level_selection_mut(&mut self.context_menu.state, level)
        else {
            return false;
        };
        if *hover_index == row {
            return false;
        }
        *hover_index = row;
        if row.is_some() {
            *keyboard_focus = None;
        }
        true
    }

    /// Updates hover state based on the provided pointer position.
    pub fn update_context_menu_hover_from_pointer(&mut self, x: i32, y: i32) {
        self.update_context_menu_hover_from_pointer_internal(x, y, Instant::now(), true);
    }

    /// Updates the keyboard focus entry for the context menu.
    pub fn set_context_menu_focus(&mut self, focus: Option<usize>) {
        self.set_context_menu_level_focus(ContextMenuLevel::Root, focus);
    }

    pub(super) fn set_context_menu_level_focus(
        &mut self,
        level: ContextMenuLevel,
        focus: Option<usize>,
    ) {
        let Some((hover_index, keyboard_focus)) =
            level_selection_mut(&mut self.context_menu.state, level)
        else {
            return;
        };
        let changed = *keyboard_focus != focus;
        *keyboard_focus = focus;
        if focus.is_some() {
            *hover_index = None;
            // The keyboard took over; a resting pointer must not undo it.
            self.context_menu.pending_hover = None;
        }
        if changed {
            self.needs_redraw = true;
        }
    }

    /// One menu's hover and keyboard focus, if that menu is open.
    pub(super) fn context_menu_level_selection(
        &self,
        level: ContextMenuLevel,
    ) -> Option<(Option<usize>, Option<usize>)> {
        let ContextMenuState::Open {
            hover_index,
            keyboard_focus,
            submenu,
            ..
        } = &self.context_menu.state
        else {
            return None;
        };
        match level {
            ContextMenuLevel::Root => Some((*hover_index, *keyboard_focus)),
            ContextMenuLevel::Submenu => {
                submenu.map(|submenu| (submenu.hover_index, submenu.keyboard_focus))
            }
        }
    }

    pub(crate) fn focus_context_menu_command(&mut self, command: MenuCommand) -> bool {
        if !self.is_context_menu_open() {
            return false;
        }
        let entries = self.context_menu_entries();
        for (index, entry) in entries.iter().enumerate() {
            if entry.disabled {
                continue;
            }
            if entry.command.as_ref() == Some(&command) {
                self.set_context_menu_focus(Some(index));
                return true;
            }
        }
        false
    }
}

fn level_selection_mut(
    state: &mut ContextMenuState,
    level: ContextMenuLevel,
) -> Option<(&mut Option<usize>, &mut Option<usize>)> {
    let ContextMenuState::Open {
        hover_index,
        keyboard_focus,
        submenu,
        ..
    } = state
    else {
        return None;
    };
    match level {
        ContextMenuLevel::Root => Some((hover_index, keyboard_focus)),
        ContextMenuLevel::Submenu => submenu
            .as_mut()
            .map(|submenu| (&mut submenu.hover_index, &mut submenu.keyboard_focus)),
    }
}

/// Whether `p` lies strictly inside the triangle `a`, `b`, `c`.
fn point_in_triangle(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let side = |(x0, y0): (f64, f64), (x1, y1): (f64, f64)| {
        (x1 - x0) * (p.1 - y0) - (y1 - y0) * (p.0 - x0)
    };
    let sides = [side(a, b), side(b, c), side(c, a)];
    sides.iter().all(|value| *value > 0.0) || sides.iter().all(|value| *value < 0.0)
}
