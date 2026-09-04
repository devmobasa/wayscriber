use std::time::Instant;

use crate::draw::{SpotlightMagnifierScratch, SpotlightMagnifierSource};

/// Render memory, warning latches, and wheel timing for Spotlight effects.
pub(in crate::backend::wayland) struct SpotlightRuntime {
    dimmed_last_frame: bool,
    magnifier_scratch: SpotlightMagnifierScratch,
    magnifier_warning_active: bool,
    wheel_idle_deadline: Option<Instant>,
    magnifier_page_warned_source: Option<SpotlightMagnifierSource>,
}

impl SpotlightRuntime {
    pub(super) fn new() -> Self {
        Self {
            dimmed_last_frame: false,
            magnifier_scratch: SpotlightMagnifierScratch::default(),
            magnifier_warning_active: false,
            wheel_idle_deadline: None,
            magnifier_page_warned_source: None,
        }
    }

    /// Records a visible warning for an unavailable page source and reports
    /// whether one is due. A complete source clears that memory.
    pub(in crate::backend::wayland) fn note_page_source(
        &mut self,
        source: SpotlightMagnifierSource,
        has_magnified_region: bool,
        show_toast: bool,
    ) -> bool {
        if source.is_complete() {
            self.magnifier_page_warned_source = None;
            return false;
        }
        if self.magnifier_page_warned_source == Some(source) || !has_magnified_region || !show_toast
        {
            return false;
        }
        self.magnifier_page_warned_source = Some(source);
        true
    }

    /// Arms one warning for a continuous run of failing visible renders.
    pub(in crate::backend::wayland) fn render_warning_due(&mut self, show_toast: bool) -> bool {
        if !show_toast || self.magnifier_warning_active {
            return false;
        }
        self.magnifier_warning_active = true;
        true
    }

    pub(in crate::backend::wayland) fn clear_render_warning(&mut self) {
        self.magnifier_warning_active = false;
    }

    pub(in crate::backend::wayland) fn note_frame(&mut self, dimmed: bool) {
        self.dimmed_last_frame = dimmed;
    }

    pub(in crate::backend::wayland) fn needs_dim_washout(&self, has_spotlight: bool) -> bool {
        has_spotlight || self.dimmed_last_frame
    }

    pub(in crate::backend::wayland) fn scratch_mut(&mut self) -> &mut SpotlightMagnifierScratch {
        &mut self.magnifier_scratch
    }

    pub(in crate::backend::wayland) fn wheel_idle_deadline(&self) -> Option<Instant> {
        self.wheel_idle_deadline
    }

    pub(in crate::backend::wayland) fn wheel_idle_deadline_mut(&mut self) -> &mut Option<Instant> {
        &mut self.wheel_idle_deadline
    }

    pub(in crate::backend::wayland) fn clear_wheel_idle_deadline(&mut self) {
        self.wheel_idle_deadline = None;
    }
}

#[cfg(test)]
mod tests {
    use super::SpotlightRuntime;
    use crate::draw::SpotlightMagnifierSource;

    #[test]
    fn page_warning_repeats_only_after_availability_changes() {
        let mut runtime = SpotlightRuntime::new();
        let unavailable = SpotlightMagnifierSource::IncompleteTransparent;

        assert!(runtime.note_page_source(unavailable, true, true));
        assert!(!runtime.note_page_source(unavailable, true, true));
        assert!(!runtime.note_page_source(SpotlightMagnifierSource::CompleteSolid, true, true));
        assert!(runtime.note_page_source(unavailable, true, true));
    }

    #[test]
    fn empty_or_suppressed_pages_do_not_consume_the_arrival_warning() {
        let mut runtime = SpotlightRuntime::new();
        let unavailable = SpotlightMagnifierSource::IncompleteTransparent;

        assert!(!runtime.note_page_source(unavailable, false, true));
        assert!(!runtime.note_page_source(unavailable, true, false));
        assert!(runtime.note_page_source(unavailable, true, true));
    }

    #[test]
    fn suppressed_render_does_not_consume_the_warning() {
        let mut runtime = SpotlightRuntime::new();

        assert!(!runtime.render_warning_due(false));
        assert!(runtime.render_warning_due(true));
        assert!(!runtime.render_warning_due(true));
        runtime.clear_render_warning();
        assert!(runtime.render_warning_due(true));
    }

    #[test]
    fn dim_washout_survives_the_first_frame_without_a_spotlight() {
        let mut runtime = SpotlightRuntime::new();
        runtime.note_frame(true);

        assert!(runtime.needs_dim_washout(false));
        runtime.note_frame(false);
        assert!(!runtime.needs_dim_washout(false));
        assert!(runtime.needs_dim_washout(true));
    }
}
