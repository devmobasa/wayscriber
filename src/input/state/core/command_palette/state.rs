use std::cell::RefCell;
use std::time::Instant;

use super::CommandPaletteResults;
use crate::config::Action;
use crate::input::Key;
use crate::palette_recents::PALETTE_RECENTS_CAP;

/// Mutable state and memoized search results owned by the command palette.
pub struct CommandPaletteState {
    pub open: bool,
    pub query: String,
    pub selected: usize,
    pub scroll: usize,
    pub(crate) repeat_key: Option<Key>,
    pub(crate) repeat_next_tick: Option<Instant>,
    pub(crate) recent: Vec<Action>,
    pub(crate) recents_dirty: bool,
    pub(in crate::input::state::core) results: RefCell<Option<CommandPaletteResults>>,
}

impl CommandPaletteState {
    pub(super) fn recent_bonus(&self, action: Action) -> i32 {
        self.recent
            .iter()
            .position(|recent| *recent == action)
            .map_or(0, |index| {
                (PALETTE_RECENTS_CAP.saturating_sub(index) as i32) * 20
            })
    }

    pub(super) fn record_action(&mut self, action: Action) {
        self.recent.retain(|recent| *recent != action);
        self.recent.insert(0, action);
        self.recent.truncate(PALETTE_RECENTS_CAP);
        self.recents_dirty = true;
    }

    pub(super) fn set_recents(&mut self, recents: Vec<Action>) {
        self.recent = recents;
        self.recent.truncate(PALETTE_RECENTS_CAP);
        self.recents_dirty = false;
    }

    pub(super) fn take_recents_dirty(&mut self) -> bool {
        std::mem::take(&mut self.recents_dirty)
    }

    pub(super) fn clear_recents_dirty(&mut self) {
        self.recents_dirty = false;
    }
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self {
            open: false,
            query: String::new(),
            selected: 0,
            scroll: 0,
            repeat_key: None,
            repeat_next_tick: None,
            recent: Vec::new(),
            recents_dirty: false,
            results: RefCell::new(None),
        }
    }
}
