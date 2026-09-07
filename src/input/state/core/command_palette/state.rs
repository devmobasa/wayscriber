use super::CommandPaletteResults;
use crate::config::Action;
use crate::input::state::core::key_repeat::OverlayKeyRepeat;
use crate::palette_recents::PALETTE_RECENTS_CAP;
use std::cell::RefCell;

/// Mutable state and memoized search results owned by the command palette.
pub struct CommandPaletteState {
    pub(super) open: bool,
    pub(super) query: String,
    pub(super) selected: usize,
    pub(super) scroll: usize,
    repeat: OverlayKeyRepeat,
    pub(super) recent: Vec<Action>,
    recents_dirty: bool,
    results: RefCell<Option<CommandPaletteResults>>,
}

impl CommandPaletteState {
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn selected(&self) -> usize {
        self.selected
    }
    pub fn scroll(&self) -> usize {
        self.scroll
    }
    pub(crate) fn recents(&self) -> &[Action] {
        &self.recent
    }
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.query_changed();
        self.repeat.clear();
    }
    pub fn close(&mut self) {
        self.open = false;
        self.repeat.clear();
    }
    pub fn set_query(&mut self, query: impl Into<String>) -> bool {
        let query = query.into();
        if self.query == query {
            return false;
        }
        self.query = query;
        self.query_changed();
        true
    }
    fn query_changed(&mut self) {
        self.selected = 0;
        self.scroll = 0;
        self.results.borrow_mut().take();
    }
    pub(super) fn append(&mut self, ch: char) {
        self.query.push(ch);
        self.query_changed();
    }
    pub(super) fn backspace(&mut self) -> bool {
        if self.query.pop().is_none() {
            return false;
        }
        self.query_changed();
        true
    }
    pub(super) fn delete_previous_word(&mut self) -> bool {
        if self.query.is_empty() {
            return false;
        }
        let separator = |ch: char| ch.is_whitespace() || ch == '+' || ch == '/';
        while self.query.chars().last().is_some_and(separator) {
            self.query.pop();
        }
        while self.query.chars().last().is_some_and(|ch| !separator(ch)) {
            self.query.pop();
        }
        self.query_changed();
        true
    }

    pub(super) fn recents_dirty(&self) -> bool {
        self.recents_dirty
    }
    pub(super) fn cached_results(
        &self,
        revision: u64,
    ) -> Option<Vec<&'static super::CommandEntry>> {
        let cached = self.results.borrow();
        let cached = cached.as_ref()?;
        (cached.query == self.query
            && cached.keymap_revision == revision
            && cached.recents == self.recent)
            .then(|| cached.results.clone())
    }
    pub(super) fn cache_results(&self, revision: u64, results: Vec<&'static super::CommandEntry>) {
        *self.results.borrow_mut() = Some(CommandPaletteResults {
            query: self.query.clone(),
            recents: self.recent.clone(),
            keymap_revision: revision,
            results,
        });
    }
    pub(super) fn select_command(&mut self, index: usize) {
        self.selected = index;
    }

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
        self.results.borrow_mut().take();
        self.recents_dirty = true;
    }

    pub(super) fn set_recents(&mut self, recents: Vec<Action>) {
        self.results.borrow_mut().take();
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
            repeat: OverlayKeyRepeat::default(),
            recent: Vec::new(),
            recents_dirty: false,
            results: RefCell::new(None),
        }
    }
}

impl CommandPaletteState {
    pub(super) fn reconcile_scroll(
        &mut self,
        rows: &[super::CommandPaletteListRow],
        capacity: usize,
    ) {
        let capacity = capacity.max(1);
        let selected = super::search::command_palette_display_index(rows, self.selected);
        self.scroll = self
            .scroll
            .min(rows.len().saturating_sub(capacity))
            .min(selected);
        if selected >= self.scroll + capacity {
            self.scroll = selected + 1 - capacity;
        }
    }

    pub(super) fn navigate(
        &mut self,
        key: crate::input::Key,
        rows: &[super::CommandPaletteListRow],
        capacity: usize,
    ) -> bool {
        use crate::input::Key;
        let Some(last) = rows.iter().rev().find_map(|row| row.command_index()) else {
            return false;
        };
        let before = (self.selected, self.scroll);
        match key {
            Key::Up => self.selected = self.selected.saturating_sub(1),
            Key::Down => self.selected = (self.selected + 1).min(last),
            Key::Home => {
                self.selected = 0;
                self.scroll = 0;
            }
            Key::End => {
                self.selected = last;
                self.scroll = rows.len().saturating_sub(capacity);
            }
            _ => return false,
        }
        let display = super::search::command_palette_display_index(rows, self.selected);
        if matches!(key, Key::Up)
            && capacity > 1
            && display > 0
            && matches!(rows[display - 1], super::CommandPaletteListRow::Header(_))
        {
            self.scroll = self.scroll.min(display - 1);
        }
        self.reconcile_scroll(rows, capacity);
        before != (self.selected, self.scroll)
    }

    pub(super) fn wheel_scroll(
        &mut self,
        direction: i32,
        rows: &[super::CommandPaletteListRow],
        capacity: usize,
    ) -> bool {
        if direction == 0 || !self.open {
            return false;
        }
        let capacity = capacity.max(1);
        let next = if direction > 0 {
            (self.scroll + 1).min(rows.len().saturating_sub(capacity))
        } else {
            self.scroll.saturating_sub(1)
        };
        if next == self.scroll {
            return false;
        }
        self.scroll = next;
        let end = (next + capacity).min(rows.len());
        let selected = super::search::command_palette_display_index(rows, self.selected);
        if selected < next {
            if let Some(index) = rows[next..end].iter().find_map(|row| row.command_index()) {
                self.selected = index;
            }
        } else if selected >= end
            && let Some(index) = rows[next..end]
                .iter()
                .rev()
                .find_map(|row| row.command_index())
        {
            self.selected = index;
        }
        // At one row, skip an isolated group header to keep a command reachable.
        if capacity == 1
            && matches!(
                rows.get(self.scroll),
                Some(super::CommandPaletteListRow::Header(_))
            )
        {
            if direction > 0 {
                if let Some((offset, index)) = rows[self.scroll..]
                    .iter()
                    .enumerate()
                    .find_map(|(i, row)| row.command_index().map(|index| (i, index)))
                {
                    self.scroll += offset;
                    self.selected = index;
                }
            } else if let Some((index, command)) = rows[..self.scroll]
                .iter()
                .enumerate()
                .rev()
                .find_map(|(i, row)| row.command_index().map(|index| (i, index)))
            {
                self.scroll = index;
                self.selected = command;
            }
        }
        true
    }
    pub(super) fn start_repeat(&mut self, key: crate::input::Key, now: std::time::Instant) {
        self.repeat
            .start(key, now, std::time::Duration::from_millis(280));
    }
    pub(crate) fn clear_repeat(&mut self) {
        self.repeat.clear();
    }
    pub(crate) fn release_repeat(&mut self, key: crate::input::Key) {
        self.repeat.release(key);
    }
    pub(super) fn repeat_timeout(&self, now: std::time::Instant) -> Option<std::time::Duration> {
        if self.open {
            self.repeat.timeout(now)
        } else {
            None
        }
    }
    pub(super) fn tick_repeat(
        &mut self,
        now: std::time::Instant,
        rows: &[super::CommandPaletteListRow],
        capacity: usize,
    ) -> bool {
        if !self.open {
            self.repeat.clear();
            return false;
        }
        let Some(key) = self.repeat.due_key(now) else {
            return false;
        };
        let changed = self.navigate(key, rows, capacity);
        self.repeat
            .schedule_fixed(now, std::time::Duration::from_millis(55));
        changed
    }
}
