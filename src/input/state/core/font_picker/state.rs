use std::cell::RefCell;
use std::time::{Duration, Instant};

use super::{FontPickerFilter, FontPickerResults, FontPickerTarget};
use crate::draw::{families_match, try_monospace_font_families, try_system_font_families};
use crate::input::Key;
use crate::input::state::core::key_repeat::OverlayKeyRepeat;
use crate::input::state::core::search::fuzzy_score;
use crate::util::Rect;

/// How many families the picker keeps as recently used.
const RECENT_LIMIT: usize = 5;

const REPEAT_INITIAL_DELAY: Duration = Duration::from_millis(280);
const REPEAT_INTERVAL: Duration = Duration::from_millis(55);
const REPEAT_FAST_INTERVAL: Duration = Duration::from_millis(20);
const REPEAT_RAMP: Duration = Duration::from_millis(1000);

/// Mutable state owned by the font-picker modal.
pub(crate) struct FontPickerState {
    pub(crate) open: bool,
    pub(crate) loading: bool,
    pub(crate) load_failed: bool,
    pub(crate) query: String,
    pub(crate) selected: usize,
    pub(crate) scroll: usize,
    pub(crate) filter: FontPickerFilter,
    pub(crate) target: FontPickerTarget,
    pub(crate) recents: Vec<String>,
    pub(crate) results: RefCell<FontPickerResults>,
    pub(crate) repeat: OverlayKeyRepeat,
    pub(crate) last_panel: Option<Rect>,
}

impl FontPickerState {
    pub(crate) fn begin_open(&mut self, catalog_ready: bool, target: FontPickerTarget) {
        self.open = true;
        self.loading = !catalog_ready;
        self.load_failed = false;
        self.query.clear();
        self.filter = FontPickerFilter::All;
        self.target = target;
        self.results.replace(None);
    }

    pub(crate) fn finish_catalog_load(&mut self) -> bool {
        if !self.open || !self.loading {
            return false;
        }
        self.loading = false;
        self.load_failed = false;
        self.results.replace(None);
        true
    }

    pub(crate) fn fail_catalog_load(&mut self) -> bool {
        if !self.open || !self.loading {
            return false;
        }
        self.loading = false;
        self.load_failed = true;
        self.results.replace(None);
        true
    }

    pub(crate) fn close(&mut self) -> bool {
        if !self.open {
            return false;
        }
        self.open = false;
        self.loading = false;
        self.load_failed = false;
        self.query.clear();
        self.results.replace(None);
        self.repeat.clear();
        self.last_panel = None;
        true
    }

    /// The filtered, ranked list the picker is showing.
    pub(crate) fn families(&self) -> Vec<String> {
        let key = (self.query.clone(), self.filter);
        if let Some((cached_key, cached)) = self.results.borrow().as_ref()
            && *cached_key == key
        {
            return cached.clone();
        }
        let ranked = self.rank_families();
        self.results.replace(Some((key, ranked.clone())));
        ranked
    }

    fn rank_families(&self) -> Vec<String> {
        if self.loading || self.load_failed {
            return Vec::new();
        }
        let source: &[String] = match self.filter {
            FontPickerFilter::All => try_system_font_families(),
            FontPickerFilter::Monospace => try_monospace_font_families(),
        }
        .unwrap_or(&[]);
        if source.is_empty() {
            return Vec::new();
        }
        let query = self.query.trim().to_lowercase();

        if query.is_empty() {
            let mut ordered: Vec<String> = self
                .recents
                .iter()
                .filter(|family| source.iter().any(|name| families_match(name, family)))
                .cloned()
                .collect();
            let rest: Vec<String> = source
                .iter()
                .filter(|name| !ordered.iter().any(|kept| families_match(kept, name)))
                .cloned()
                .collect();
            ordered.extend(rest);
            return ordered;
        }

        let mut scored: Vec<(i32, &String)> = source
            .iter()
            .filter_map(|family| {
                let score = fuzzy_score(&query, family);
                (score > 0).then_some((score, family))
            })
            .collect();
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        scored
            .into_iter()
            .map(|(_, family)| family.clone())
            .collect()
    }

    pub(crate) fn reset_position(&mut self) {
        self.selected = 0;
        self.scroll = 0;
    }

    pub(crate) fn start_repeat(&mut self, key: Key, now: Instant) {
        self.repeat.start(key, now, REPEAT_INITIAL_DELAY);
    }

    pub(crate) fn clear_repeat(&mut self) {
        self.repeat.clear();
    }

    pub(crate) fn release_repeat_key(&mut self, key: Key) {
        self.repeat.release(key);
    }

    pub(crate) fn repeat_timeout(&self, now: Instant) -> Option<Duration> {
        if !self.open {
            return None;
        }
        self.repeat.timeout(now)
    }

    pub(crate) fn schedule_next_repeat(&mut self, now: Instant) {
        self.repeat.schedule_ramped(
            now,
            REPEAT_INITIAL_DELAY,
            REPEAT_INTERVAL,
            REPEAT_FAST_INTERVAL,
            REPEAT_RAMP,
        );
    }

    pub(crate) fn due_repeat_key(&self, now: Instant) -> Option<Key> {
        self.repeat.due_key(now)
    }

    pub(crate) fn remember_choice(&mut self, family: &str) {
        self.recents
            .retain(|existing| !families_match(existing, family));
        self.recents.insert(0, family.to_string());
        self.recents.truncate(RECENT_LIMIT);
        self.results.replace(None);
    }
}

impl Default for FontPickerState {
    fn default() -> Self {
        Self {
            open: false,
            loading: false,
            load_failed: false,
            query: String::new(),
            selected: 0,
            scroll: 0,
            filter: FontPickerFilter::All,
            target: FontPickerTarget::ToolDefault,
            recents: Vec::new(),
            results: RefCell::new(None),
            repeat: OverlayKeyRepeat::default(),
            last_panel: None,
        }
    }
}
