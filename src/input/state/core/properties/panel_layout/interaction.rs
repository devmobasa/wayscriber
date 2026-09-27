use super::super::super::base::InputState;
use super::super::types::PropertiesPanelHit;
use crate::draw::TextMeasurer;

/// How far one wheel tick scrolls overflowing rows.
const WHEEL_SCROLL_STEP: f64 = 40.0;

impl InputState {
    /// The panel part under `(x, y)`, before any disabled-row filtering.
    pub fn properties_panel_hit_at(&self, x: i32, y: i32) -> Option<PropertiesPanelHit> {
        let layout = self.properties.layout.as_ref()?;
        let panel = self.properties.panel.as_ref()?;
        layout.hit_at(panel, x as f64, y as f64)
    }

    /// The part under `(x, y)` that responds to the pointer. A locked row is
    /// inert; the header stays live, which is how locked shapes get unlocked.
    pub(crate) fn properties_panel_active_hit_at(
        &self,
        x: i32,
        y: i32,
    ) -> Option<PropertiesPanelHit> {
        let hit = self.properties_panel_hit_at(x, y)?;
        let panel = self.properties.panel.as_ref()?;
        match hit.row() {
            Some(row) if panel.entries.get(row).is_none_or(|entry| entry.disabled) => None,
            _ => Some(hit),
        }
    }

    /// The row under `(x, y)`, if any.
    pub fn properties_panel_index_at(&self, x: i32, y: i32) -> Option<usize> {
        self.properties_panel_hit_at(x, y)?.row()
    }

    /// Whether `(x, y)` is over the open panel.
    pub fn properties_panel_contains(&self, x: i32, y: i32) -> bool {
        self.properties
            .layout
            .as_ref()
            .is_some_and(|layout| layout.contains(x as f64, y as f64))
    }

    pub(super) fn update_properties_panel_hover_from_pointer_internal(
        &mut self,
        x: i32,
        y: i32,
        trigger_redraw: bool,
    ) {
        // A color picker opened from the panel sits over it and owns the
        // pointer, so nothing underneath may light up.
        let new_hover = if self.is_color_picker_popup_open() {
            None
        } else {
            self.properties_panel_active_hit_at(x, y)
        };
        let Some(panel) = self.properties.panel.as_mut() else {
            return;
        };

        if panel.hover != new_hover {
            panel.hover = new_hover;
            if trigger_redraw {
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
            }
        }
    }

    pub fn update_properties_panel_hover_from_pointer(&mut self, x: i32, y: i32) {
        self.update_properties_panel_hover_from_pointer_internal(x, y, true);
    }

    /// A primary press at `(x, y)`. Returns false when it landed off the
    /// panel. The pressed part is remembered for the release, and its row
    /// takes (quiet) keyboard focus so arrow keys continue from the click.
    pub(crate) fn press_properties_panel_at(&mut self, x: i32, y: i32) -> bool {
        if !self.properties_panel_contains(x, y) {
            return false;
        }
        let hit = self.properties_panel_active_hit_at(x, y);
        if let Some(panel) = self.properties.panel.as_mut() {
            panel.pressed = hit;
            if let Some(row) = hit.and_then(PropertiesPanelHit::row) {
                panel.keyboard_focus = Some(row);
                panel.focus_visible = false;
            }
        }
        true
    }

    /// A primary release at `(x, y)`. It activates the pressed part only when
    /// the pointer is still on it, so dragging off a button cancels the
    /// click. A release off the panel with nothing pressed closes it.
    pub(crate) fn release_properties_panel_at_with(
        &mut self,
        measurer: &TextMeasurer,
        x: i32,
        y: i32,
    ) {
        let pressed = self
            .properties
            .panel
            .as_mut()
            .and_then(|panel| panel.pressed.take());
        match pressed {
            Some(hit) => {
                if self.properties_panel_active_hit_at(x, y) == Some(hit) {
                    let _ = self.activate_properties_panel_hit_with(measurer, hit);
                }
            }
            None if !self.properties_panel_contains(x, y) => self.close_properties_panel(),
            None => {}
        }
    }

    /// A wheel tick at `(x, y)`. Over a row it steps that row's property;
    /// anywhere else on the panel it is swallowed, so it cannot fall through
    /// to the canvas and resize the tool behind the panel. When the rows
    /// overflow a short screen and scroll, the wheel scrolls them instead.
    /// Returns whether the panel took the tick.
    pub(crate) fn properties_panel_wheel_with(
        &mut self,
        measurer: &TextMeasurer,
        x: i32,
        y: i32,
        scroll_direction: i32,
    ) -> bool {
        if !self.is_properties_panel_open() || !self.properties_panel_contains(x, y) {
            return false;
        }
        if self.scroll_properties_panel_rows(scroll_direction) {
            return true;
        }
        if let Some(row) = self.properties_panel_index_at(x, y) {
            let _ = self.step_properties_panel_row_by_wheel_with(measurer, row, scroll_direction);
        }
        true
    }

    /// Scrolls overflowing rows by one wheel tick. Returns false when the rows
    /// fit and do not scroll.
    fn scroll_properties_panel_rows(&mut self, scroll_direction: i32) -> bool {
        let Some(scroll) = self
            .properties
            .layout
            .as_ref()
            .and_then(|layout| layout.scroll)
        else {
            return false;
        };
        let offset = (scroll.offset + f64::from(scroll_direction) * WHEEL_SCROLL_STEP)
            .clamp(0.0, scroll.max_offset);

        if let Some(panel) = self.properties.panel.as_mut() {
            panel.scroll = offset;
            // The wheel took over from the keyboard, so a focused row may
            // scroll away.
            panel.focus_visible = false;
        }
        if let Some(layout) = self.properties.layout.as_mut()
            && let Some(scroll) = layout.scroll.as_mut()
        {
            scroll.offset = offset;
        }
        self.properties.request_hover_recalc();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }
}
