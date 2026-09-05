use super::{TextCacheKey, TextMeasurement, TextMeasurementCache, configured_layout};
use std::cell::RefCell;

/// Canonical shape text measurements and cursor geometry for one owner.
/// Construction creates no Cairo/Pango resources. All geometry uses the same
/// measurement policy regardless of the eventual drawing destination.
pub struct TextMeasurer {
    cache: RefCell<TextMeasurementCache>,
    context: RefCell<Option<cairo::Context>>,
}

impl Default for TextMeasurer {
    fn default() -> Self {
        Self {
            cache: RefCell::new(TextMeasurementCache::new(256)),
            context: RefCell::new(None),
        }
    }
}

impl TextMeasurer {
    #[cfg(test)]
    pub(super) fn cache_len(&self) -> usize {
        self.cache.borrow().entries.len()
    }

    pub(super) fn with_measurement_context<R>(
        &self,
        f: impl FnOnce(&cairo::Context) -> R,
    ) -> Option<R> {
        let ctx = {
            let mut context = self.context.borrow_mut();
            if context.is_none() {
                let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).ok()?;
                let ctx = cairo::Context::new(&surface).ok()?;
                ctx.set_antialias(cairo::Antialias::Best);
                *context = Some(ctx);
            }
            context.as_ref()?.clone()
        };
        // Cairo retains its target surface. Release the initialization borrow
        // before calling Pango or any nested measurement operation.
        Some(f(&ctx))
    }
    pub(crate) fn measure(
        &self,
        text: &str,
        font_desc_str: &str,
        size: f64,
        wrap_width: Option<i32>,
    ) -> Option<TextMeasurement> {
        if text.is_empty() {
            return None;
        }

        let key = TextCacheKey::new(text, font_desc_str, size, wrap_width);

        // Check cache first
        let cached = self.cache.borrow_mut().get(&key);
        if let Some(measurement) = cached {
            return Some(measurement);
        }

        // Measure using shared context
        let measurement = self.with_measurement_context(|ctx| {
            let layout = configured_layout(ctx, text, font_desc_str, wrap_width);

            let (ink_rect, logical_rect) = layout.extents();
            let scale = pango::SCALE as f64;

            TextMeasurement {
                ink_x: ink_rect.x() as f64 / scale,
                ink_y: ink_rect.y() as f64 / scale,
                ink_width: ink_rect.width() as f64 / scale,
                ink_height: ink_rect.height() as f64 / scale,
                logical_x: logical_rect.x() as f64 / scale,
                logical_y: logical_rect.y() as f64 / scale,
                logical_width: logical_rect.width() as f64 / scale,
                logical_height: logical_rect.height() as f64 / scale,
                baseline: layout.baseline() as f64 / scale,
            }
        })?;

        // Cache the result
        self.cache.borrow_mut().insert(key, measurement.clone());

        Some(measurement)
    }
}

#[cfg(test)]
mod tests;
