//! Choosing a text font by looking at it.
//!
//! `Shift+T` steps through the short configured list. This is the other half:
//! a modal over every family the system has, so the short list is something the
//! user picked rather than something they inherited.
//!
//! Two design decisions worth stating, because both cost something:
//!
//! **Every row renders in its own font.** That is the whole point of a font
//! picker — nobody chooses a typeface by reading its name. Only the visible
//! rows are laid out, so the cost is bounded by the window rather than by the
//! 269 families a normal desktop has.
//!
//! **The list is enumerated once, off the input thread.** The Wayland backend
//! prewarms the process-wide catalog after its first committed frame. If the
//! picker wins that race, it opens immediately with a loading row and fills in
//! when the worker wakes the event loop.

mod input;
mod layout;
mod state;

pub(crate) use state::FontPickerState;

pub use layout::{FontPickerLayout, FontPickerRow, font_picker_layout, font_picker_rows};

use super::InputState;
use crate::draw::{FontDescriptor, families_match, system_font_catalog_is_ready};

/// The picker's memoized result list, keyed by what produced it.
pub type FontPickerResults = Option<((String, FontPickerFilter), Vec<String>)>;

/// Which slice of the system list the picker is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontPickerFilter {
    /// Every installed family.
    #[default]
    All,
    /// Only the families the font system reports as monospace.
    Monospace,
}

impl FontPickerFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All fonts",
            Self::Monospace => "Monospace",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Monospace,
            Self::Monospace => Self::All,
        }
    }
}

/// What the picker will change when a row is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontPickerTarget {
    /// Nothing is selected: the choice sets what the next label uses.
    ToolDefault,
    /// Text is selected: the choice restyles it.
    Selection,
}

impl FontPickerTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::ToolDefault => "Sets the font for new text",
            Self::Selection => "Restyles the selected text",
        }
    }
}

impl InputState {
    pub(crate) fn is_font_picker_open(&self) -> bool {
        self.font_picker.open
    }

    pub fn font_picker_is_loading(&self) -> bool {
        self.font_picker.loading
    }

    pub fn font_picker_load_failed(&self) -> bool {
        self.font_picker.load_failed
    }

    pub fn font_picker_query(&self) -> &str {
        &self.font_picker.query
    }

    pub fn font_picker_filter(&self) -> FontPickerFilter {
        self.font_picker.filter
    }

    pub fn font_picker_selected(&self) -> usize {
        self.font_picker.selected
    }

    pub fn font_picker_scroll(&self) -> usize {
        self.font_picker.scroll
    }

    /// What choosing a row would change, decided when the picker opens so the
    /// caption cannot disagree with what Enter does.
    pub fn font_picker_target(&self) -> FontPickerTarget {
        self.font_picker.target
    }

    /// Open the picker over the system font list without enumerating fonts in
    /// input dispatch.
    pub(crate) fn open_font_picker(&mut self) {
        self.open_font_picker_with_catalog_ready(system_font_catalog_is_ready());
    }

    fn open_font_picker_with_catalog_ready(&mut self, catalog_ready: bool) {
        self.close_modals_for_open(super::modal::ModalSurface::FontPicker);
        let target = if self.selection_has_text() {
            FontPickerTarget::Selection
        } else {
            FontPickerTarget::ToolDefault
        };
        self.font_picker.begin_open(catalog_ready, target);
        // Start on the font in use, so the picker opens showing where you are.
        //
        // Centred in the window this surface actually has room for, not in the
        // twelve-row ceiling: half of twelve is below the bottom of a six-row
        // panel, which would open the picker scrolled past the very row it
        // means to be showing.
        self.position_font_picker_on_current_family();
        // Reopening on top of an open picker must not leave the previous key
        // still repeating into the fresh list.
        self.clear_font_picker_repeat();
        // Record the panel this open is about to paint. Partial repaints damage
        // the panel they are leaving as well as the one they are arriving at,
        // and the first query is usually the one that shrinks the panel most —
        // without a starting point, the taller panel's lower half is never
        // repainted and stays on screen under the shorter one.
        self.font_picker.last_panel = self.font_picker_panel_bounds();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Fill a picker that opened while the worker was enumerating fonts.
    ///
    /// Returns whether the open surface changed and needs repainting.
    pub(crate) fn finish_font_picker_catalog_load(&mut self) -> bool {
        if !self.font_picker.finish_catalog_load() {
            return false;
        }
        debug_assert!(system_font_catalog_is_ready());
        self.position_font_picker_on_current_family();
        self.mark_font_picker_dirty();
        true
    }

    /// Replace the loading row with a stable error if the background worker
    /// could not produce a catalog. Reopening gives the backend one fresh try.
    pub(crate) fn fail_font_picker_catalog_load(&mut self) -> bool {
        if !self.font_picker.fail_catalog_load() {
            return false;
        }
        self.mark_font_picker_dirty();
        true
    }

    fn position_font_picker_on_current_family(&mut self) {
        if self.font_picker.loading {
            self.font_picker.selected = 0;
            self.font_picker.scroll = 0;
            return;
        }
        let current = self.font_picker_current_family();
        let families = self.font_picker_families();
        let window = self.font_picker_visible_rows(families.len());
        self.font_picker.selected = families
            .iter()
            .position(|family| families_match(family, &current))
            .unwrap_or(0);
        self.font_picker.scroll = self
            .font_picker
            .selected
            .saturating_sub(window / 2)
            .min(families.len().saturating_sub(window));
    }

    pub(crate) fn close_font_picker(&mut self) {
        if !self.font_picker.close() {
            return;
        }
        // The scrim covered the whole surface, so the whole surface comes back.
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// The family the picker considers current: the selected text's, or the
    /// tool's when nothing is selected.
    pub(crate) fn font_picker_current_family(&self) -> String {
        if self.font_picker.target == FontPickerTarget::Selection
            && let Some(family) = self.first_selected_text_family()
        {
            return family;
        }
        self.style.font_descriptor.family.clone()
    }

    /// Families most recently chosen here, most recent first.
    pub fn font_picker_recents(&self) -> &[String] {
        &self.font_picker.recents
    }

    /// The filtered, ranked list the picker is showing.
    ///
    /// Memoized on the query and filter: the renderer asks for it more than
    /// once per frame, and scoring walks every installed family.
    pub fn font_picker_families(&self) -> Vec<String> {
        self.font_picker.families()
    }

    /// Apply the highlighted family and close.
    pub(crate) fn commit_font_picker(&mut self) -> bool {
        let families = self.font_picker_families();
        let Some(family) = families.get(self.font_picker.selected).cloned() else {
            self.close_font_picker();
            return false;
        };
        let applied = self.apply_font_family(&family);
        if applied {
            self.font_picker.remember_choice(&family);
            log::info!("Font picker applied {family}");
            self.push_toast(
                super::ToastPriority::Info,
                FONT_PICKER_TOAST_SOURCE,
                super::Toast::info(format!("Font: {family}")),
            );
        }
        self.close_font_picker();
        applied
    }

    /// Set `family` on the selection, or on the tool when nothing is selected.
    fn apply_font_family(&mut self, family: &str) -> bool {
        if self.font_picker.target == FontPickerTarget::Selection && self.selection_has_text() {
            return self.apply_family_to_selected_text(family);
        }

        self.set_font_descriptor(FontDescriptor::new(
            family.to_string(),
            self.style.font_descriptor.weight.clone(),
            self.style.font_descriptor.style.clone(),
        ))
    }
}

const FONT_PICKER_TOAST_SOURCE: &str = "font-picker";

#[cfg(test)]
mod tests;
