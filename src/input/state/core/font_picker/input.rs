//! Keyboard and pointer handling while the font picker owns input.

use std::time::{Duration, Instant};

use super::InputState;
use super::layout::{font_picker_layout, font_picker_row_at};
use crate::input::events::Key;

/// Rows one wheel tick moves. Three rather than the command palette's one: a
/// palette holds tens of commands and this holds every font installed, so a
/// tick that moves a single row turns a 269-family list into 269 ticks.
const FONT_PICKER_WHEEL_ROWS: usize = 3;

/// How long a navigation key must be held before it starts repeating.
const REPEAT_INITIAL_DELAY: Duration = Duration::from_millis(280);
/// Interval the repeat starts at, matching the command palette's.
const REPEAT_INTERVAL: Duration = Duration::from_millis(55);
/// Interval the repeat ramps down to while the key stays held.
const REPEAT_FAST_INTERVAL: Duration = Duration::from_millis(20);
/// How long of holding it takes to reach [`REPEAT_FAST_INTERVAL`].
///
/// The palette repeats at one flat rate, which is right for a list of tens.
/// This list is every font on the system, and crossing it at the flat rate
/// takes about fifteen seconds — long enough that people give up and reach for
/// the mouse. Ramping keeps a short press precise and makes a long hold
/// actually travel.
const REPEAT_RAMP: Duration = Duration::from_millis(1000);

/// Whether holding this key should keep moving the highlight.
///
/// Navigation only. A query is a handful of characters, so `Backspace` repeat
/// would be a way to lose one by accident rather than a way to get anywhere.
fn repeats(key: Key) -> bool {
    matches!(key, Key::Up | Key::Down | Key::PageUp | Key::PageDown)
}

impl InputState {
    /// Route one key while the picker is open. Returns whether it was consumed.
    ///
    /// Every printable character goes into the query, so a family name can be
    /// typed straight in without a mode change. That is also why the filter
    /// toggle is `Tab` rather than a letter.
    pub(crate) fn handle_font_picker_key(&mut self, key: Key, text: Option<&str>) -> bool {
        if !self.font_picker.open {
            return false;
        }
        match key {
            Key::Escape => {
                self.close_font_picker();
            }
            Key::Return => {
                self.commit_font_picker();
            }
            Key::Tab => {
                self.font_picker.filter = self.font_picker.filter.next();
                self.font_picker.results.replace(None);
                self.reset_font_picker_position();
            }
            Key::Down => self.move_font_picker_selection(1),
            Key::Up => self.move_font_picker_selection(-1),
            Key::PageDown => {
                let page = self.font_picker_page() as i64;
                self.move_font_picker_selection(page);
            }
            Key::PageUp => {
                let page = self.font_picker_page() as i64;
                self.move_font_picker_selection(-page);
            }
            Key::Home => self.set_font_picker_selection(0),
            Key::End => {
                let last = self.font_picker_families().len().saturating_sub(1);
                self.set_font_picker_selection(last);
            }
            Key::Space => {
                self.font_picker.query.push(' ');
                self.font_picker.results.replace(None);
                self.reset_font_picker_position();
            }
            Key::Backspace => {
                self.font_picker.query.pop();
                self.font_picker.results.replace(None);
                self.reset_font_picker_position();
            }
            _ => {
                let Some(text) =
                    text.filter(|text| !text.is_empty() && text.chars().all(|c| !c.is_control()))
                else {
                    // Unhandled keys are still swallowed: a modal that let
                    // stray keys through to the canvas would draw behind itself.
                    return true;
                };
                self.font_picker.query.push_str(text);
                self.font_picker.results.replace(None);
                self.reset_font_picker_position();
            }
        }
        // A held navigation key keeps moving. The backend's own repeat timer is
        // retired while a modal is engaged, or it would feed the canvas behind
        // this panel, so the picker owns its repeat the way the palette does.
        if repeats(key) {
            self.start_font_picker_repeat(key);
        } else {
            self.clear_font_picker_repeat();
        }
        self.mark_font_picker_dirty();
        true
    }

    /// Repaint the panel rather than the screen.
    ///
    /// A held arrow ticks up to fifty times a second; a full-surface repaint at
    /// that rate is the whole canvas re-rendered per row. The panel is a known
    /// rectangle, and a query that changes the result count changes its height,
    /// so the panel being left is repainted along with the one arriving.
    pub(crate) fn mark_font_picker_dirty(&mut self) {
        self.needs_redraw = true;
        if !self.font_picker.open {
            self.font_picker.last_panel = None;
            self.dirty_tracker.mark_full();
            return;
        }
        let panel = self.font_picker_panel_bounds();
        self.dirty_tracker.mark_optional_rect(panel);
        if self.font_picker.last_panel != panel {
            self.dirty_tracker
                .mark_optional_rect(self.font_picker.last_panel);
        }
        self.font_picker.last_panel = panel;
    }

    /// The panel's rectangle, grown to cover the shadow it casts.
    pub(in crate::input::state::core) fn font_picker_panel_bounds(
        &self,
    ) -> Option<crate::util::Rect> {
        const SHADOW: f64 = 4.0;
        let layout = font_picker_layout(
            self.screen_width,
            self.screen_height,
            self.font_picker_families().len(),
        );
        crate::util::Rect::new(
            (layout.panel_x - SHADOW).floor() as i32,
            (layout.panel_y - SHADOW).floor() as i32,
            (layout.panel_width + SHADOW * 2.0).ceil() as i32,
            (layout.panel_height + SHADOW * 2.0).ceil() as i32,
        )
    }

    /// Move the highlight by `delta`, clamped rather than wrapped.
    ///
    /// Clamped because the list can be hundreds long: wrapping from the top to
    /// the bottom of 269 families is never what an arrow key meant.
    pub(crate) fn move_font_picker_selection(&mut self, delta: i64) {
        let count = self.font_picker_families().len();
        if count == 0 {
            return;
        }
        let next = (self.font_picker.selected as i64)
            .saturating_add(delta)
            .clamp(0, count as i64 - 1) as usize;
        self.set_font_picker_selection(next);
    }

    /// Highlight `index` and scroll the window the least amount that shows it.
    pub(crate) fn set_font_picker_selection(&mut self, index: usize) {
        let count = self.font_picker_families().len();
        if count == 0 {
            self.font_picker.selected = 0;
            self.font_picker.scroll = 0;
            return;
        }
        self.font_picker.selected = index.min(count - 1);
        // The window the surface actually shows, not the ceiling. A short
        // output draws fewer rows, and scrolling by the ceiling would leave the
        // highlight on a row below the panel's bottom edge.
        let visible = self.font_picker_visible_rows(count);
        if self.font_picker.selected < self.font_picker.scroll {
            self.font_picker.scroll = self.font_picker.selected;
        } else if self.font_picker.selected >= self.font_picker.scroll + visible {
            self.font_picker.scroll = self.font_picker.selected + 1 - visible;
        }
        let max_scroll = count.saturating_sub(visible);
        self.font_picker.scroll = self.font_picker.scroll.min(max_scroll);
        self.needs_redraw = true;
    }

    /// Rows the current surface has room to show, floored at one so the scroll
    /// arithmetic always has a window to work with.
    pub(crate) fn font_picker_visible_rows(&self, row_count: usize) -> usize {
        font_picker_layout(self.screen_width, self.screen_height, row_count)
            .visible_rows
            .max(1)
    }

    /// How far Page Up and Page Down move: one screenful of this surface.
    fn font_picker_page(&self) -> usize {
        self.font_picker_visible_rows(self.font_picker_families().len())
    }

    /// Back to the top after the result list changed under the highlight.
    fn reset_font_picker_position(&mut self) {
        self.font_picker.selected = 0;
        self.font_picker.scroll = 0;
    }

    /// Scroll the list by one wheel tick.
    ///
    /// The window moves and the highlight comes along only when the window
    /// would leave it behind — the same arrangement the command palette uses,
    /// so `Enter` always applies the row that is highlighted rather than
    /// whatever happens to be under the pointer.
    pub(crate) fn font_picker_wheel_scroll(&mut self, direction: i32) {
        if direction == 0 || !self.font_picker.open {
            return;
        }
        let count = self.font_picker_families().len();
        let window = self.font_picker_visible_rows(count);
        let max_scroll = count.saturating_sub(window);
        let next = if direction > 0 {
            (self.font_picker.scroll + FONT_PICKER_WHEEL_ROWS).min(max_scroll)
        } else {
            self.font_picker
                .scroll
                .saturating_sub(FONT_PICKER_WHEEL_ROWS)
        };
        if next == self.font_picker.scroll {
            return;
        }
        self.font_picker.scroll = next;
        self.font_picker.selected = self
            .font_picker
            .selected
            .clamp(next, (next + window).saturating_sub(1).min(count - 1));
        self.mark_font_picker_dirty();
    }

    fn start_font_picker_repeat(&mut self, key: Key) {
        let now = Instant::now();
        // A different key restarts the ramp; the same key held keeps it.
        if self.font_picker.repeat_key != Some(key) {
            self.font_picker.repeat_key = Some(key);
            self.font_picker.repeat_started = Some(now);
            self.font_picker.repeat_next_tick = Some(now + REPEAT_INITIAL_DELAY);
        }
    }

    pub(crate) fn clear_font_picker_repeat(&mut self) {
        self.font_picker.repeat_key = None;
        self.font_picker.repeat_next_tick = None;
        self.font_picker.repeat_started = None;
    }

    /// Stop repeating when the held key comes up.
    pub(crate) fn release_font_picker_repeat_key(&mut self, key: Key) {
        if self.font_picker.repeat_key == Some(key) {
            self.clear_font_picker_repeat();
        }
    }

    /// Gap to the next repeat, ramping from [`REPEAT_INTERVAL`] down to
    /// [`REPEAT_FAST_INTERVAL`] over [`REPEAT_RAMP`] of holding.
    fn font_picker_repeat_interval(&self, now: Instant) -> Duration {
        let Some(started) = self.font_picker.repeat_started else {
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

    /// Time until the next repeat, for the event loop's timeout. Without it the
    /// loop sleeps until a real event and a held key never moves again.
    pub(crate) fn font_picker_repeat_timeout(&self, now: Instant) -> Option<Duration> {
        if !self.font_picker.open {
            return None;
        }
        self.font_picker
            .repeat_next_tick
            .map(|next| next.saturating_duration_since(now))
    }

    /// Fire one repeat if due. Returns whether anything moved.
    pub(crate) fn tick_font_picker_repeat(&mut self, now: Instant) -> bool {
        if !self.font_picker.open {
            self.clear_font_picker_repeat();
            return false;
        }
        let Some(key) = self.font_picker.repeat_key else {
            return false;
        };
        let Some(next) = self.font_picker.repeat_next_tick else {
            return false;
        };
        if now < next {
            return false;
        }
        let before = self.font_picker.selected;
        match key {
            Key::Up => self.move_font_picker_selection(-1),
            Key::Down => self.move_font_picker_selection(1),
            Key::PageUp => {
                let page = self.font_picker_page() as i64;
                self.move_font_picker_selection(-page);
            }
            Key::PageDown => {
                let page = self.font_picker_page() as i64;
                self.move_font_picker_selection(page);
            }
            _ => return false,
        }
        // Rescheduled from `now`, not from the deadline: a long frame must not
        // leave a burst of catch-up ticks queued behind it.
        self.font_picker.repeat_next_tick = Some(now + self.font_picker_repeat_interval(now));
        let moved = self.font_picker.selected != before;
        if moved {
            self.mark_font_picker_dirty();
        }
        moved
    }

    /// Highlight the row under the pointer. Returns whether one was hit.
    pub(crate) fn font_picker_hover(&mut self, x: f64, y: f64) -> bool {
        if !self.font_picker.open {
            return false;
        }
        let families = self.font_picker_families();
        let layout = font_picker_layout(self.screen_width, self.screen_height, families.len());
        let Some(index) = font_picker_row_at(layout, &families, self.font_picker.scroll, x, y)
        else {
            return false;
        };
        if index != self.font_picker.selected {
            self.font_picker.selected = index;
            self.mark_font_picker_dirty();
        }
        true
    }

    /// Apply the row under the pointer. Returns whether the press was consumed.
    ///
    /// A press outside the panel closes the picker, which is what clicking away
    /// from a modal means everywhere else in the overlay.
    pub(crate) fn font_picker_press(&mut self, x: f64, y: f64) -> bool {
        if !self.font_picker.open {
            return false;
        }
        let families = self.font_picker_families();
        let layout = font_picker_layout(self.screen_width, self.screen_height, families.len());
        if let Some(index) = font_picker_row_at(layout, &families, self.font_picker.scroll, x, y) {
            self.set_font_picker_selection(index);
            self.commit_font_picker();
            return true;
        }
        let inside_panel = x >= layout.panel_x
            && x <= layout.panel_x + layout.panel_width
            && y >= layout.panel_y
            && y <= layout.panel_y + layout.panel_height;
        if !inside_panel {
            self.close_font_picker();
        }
        true
    }
}
