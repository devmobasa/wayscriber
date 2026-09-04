use super::TextMeasurement;
use std::collections::HashMap;

/// Cache key for text measurements.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(super) struct TextCacheKey {
    text: String,
    font_desc_str: String,
    /// Size in hundredths of points for stable hashing
    size_hundredths: i32,
    /// Wrap width in pixels, or -1 for no wrap
    wrap_width: i32,
}

impl TextCacheKey {
    pub(super) fn new(text: &str, font_desc_str: &str, size: f64, wrap_width: Option<i32>) -> Self {
        Self {
            text: text.to_string(),
            font_desc_str: font_desc_str.to_string(),
            size_hundredths: (size * 100.0).round() as i32,
            wrap_width: wrap_width.unwrap_or(-1),
        }
    }
}

/// Numeric measurements retained by one text service.
/// Uses an LRU-style eviction when cache exceeds max size.
pub(super) struct TextMeasurementCache {
    pub(super) entries: HashMap<TextCacheKey, TextMeasurement>,
    access_order: Vec<TextCacheKey>,
    max_entries: usize,
}

impl TextMeasurementCache {
    pub(super) fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_entries),
            access_order: Vec::with_capacity(max_entries),
            max_entries,
        }
    }

    pub(super) fn get(&mut self, key: &TextCacheKey) -> Option<TextMeasurement> {
        if let Some(measurement) = self.entries.get(key) {
            // Move to end of access order (most recently used)
            if let Some(pos) = self.access_order.iter().position(|k| k == key) {
                self.access_order.remove(pos);
                self.access_order.push(key.clone());
            }
            Some(measurement.clone())
        } else {
            None
        }
    }

    pub(super) fn insert(&mut self, key: TextCacheKey, measurement: TextMeasurement) {
        // If key already exists, update it and move to end of access order
        if self.entries.contains_key(&key) {
            self.entries.insert(key.clone(), measurement);
            if let Some(pos) = self.access_order.iter().position(|k| k == &key) {
                self.access_order.remove(pos);
            }
            self.access_order.push(key);
            return;
        }

        // Evict oldest entries if at capacity
        while self.entries.len() >= self.max_entries && !self.access_order.is_empty() {
            let oldest = self.access_order.remove(0);
            self.entries.remove(&oldest);
        }

        self.entries.insert(key.clone(), measurement);
        self.access_order.push(key);
    }
}
