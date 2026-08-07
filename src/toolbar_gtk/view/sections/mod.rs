//! Content builders for the top strip's Canvas, Session, and Settings
//! popovers.

pub(in crate::toolbar_gtk) mod canvas_pane;
pub(in crate::toolbar_gtk) mod session_pane;
pub(in crate::toolbar_gtk) mod settings_pane;
mod step_undo;

use crate::ui::toolbar::ToolbarSnapshot;

use super::super::widgets::FeedbackSender;
use super::Updater;

/// Everything a section builder needs; `updaters` collects the closures
/// that keep the built widgets in sync with later snapshots.
pub(in crate::toolbar_gtk) struct SectionCtx<'a> {
    pub(in crate::toolbar_gtk) snapshot: &'a ToolbarSnapshot,
    pub(in crate::toolbar_gtk) feedback: FeedbackSender,
    pub(in crate::toolbar_gtk) scale: f64,
    pub(in crate::toolbar_gtk) use_icons: bool,
    pub(in crate::toolbar_gtk) updaters: &'a mut Vec<Updater>,
}

impl SectionCtx<'_> {
    pub(in crate::toolbar_gtk) fn sz(&self, value: f64) -> f64 {
        value * self.scale
    }

    pub(in crate::toolbar_gtk) fn px(&self, value: f64) -> i32 {
        (value * self.scale).round() as i32
    }
}
