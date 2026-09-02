use std::cell::RefCell;
use std::time::{Duration, Instant};

use super::{FontPickerFilter, FontPickerResults, FontPickerTarget};
use crate::draw::{families_match, try_monospace_font_families, try_system_font_families};
use crate::input::Key;
use crate::input::state::core::command_palette::fuzzy_score;
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
    pub(crate) repeat_key: Option<Key>,
    pub(crate) repeat_next_tick: Option<Instant>,
    pub(crate) repeat_started: Option<Instant>,
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
        self.repeat_key = None;
        self.repeat_next_tick = None;
        self.repeat_started = None;
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
        if self.repeat_key != Some(key) {
            self.repeat_key = Some(key);
            self.repeat_started = Some(now);
            self.repeat_next_tick = Some(now + REPEAT_INITIAL_DELAY);
        }
    }

    pub(crate) fn clear_repeat(&mut self) {
        self.repeat_key = None;
        self.repeat_next_tick = None;
        self.repeat_started = None;
    }

    pub(crate) fn release_repeat_key(&mut self, key: Key) {
        if self.repeat_key == Some(key) {
            self.clear_repeat();
        }
    }

    pub(crate) fn repeat_timeout(&self, now: Instant) -> Option<Duration> {
        if !self.open {
            return None;
        }
        self.repeat_next_tick
            .map(|next| next.saturating_duration_since(now))
    }

    pub(crate) fn schedule_next_repeat(&mut self, now: Instant) {
        let interval = self.repeat_interval(now);
        self.repeat_next_tick = Some(now + interval);
    }

    fn repeat_interval(&self, now: Instant) -> Duration {
        let Some(started) = self.repeat_started else {
            return REPEAT_INTERVAL;
        };
        let repeating = now
            .saturating_duration_since(started)
            .saturating_sub(REPEAT_INITIAL_DELAY);
        let progress = (repeating.as_secs_f64() / REPEAT_RAMP.as_secs_f64()).clamp(0.0, 1.0);
        let slow = REPEAT_INTERVAL.as_secs_f64();
        let fast = REPEAT_FAST_INTERVAL.as_secs_f64();
        Duration::from_secs_f64(slow + (fast - slow) * progress)
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
            repeat_key: None,
            repeat_next_tick: None,
            repeat_started: None,
            last_panel: None,
        }
    }
}
