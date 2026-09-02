use std::cell::RefCell;
use std::time::Instant;

use super::{FontPickerFilter, FontPickerResults, FontPickerTarget};
use crate::input::Key;
use crate::util::Rect;

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
