use std::time::{Duration, Instant};

use crate::config::{StatusBarStyle, StatusPosition};
use crate::ui::{StatusHudLayout, StatusHudSegmentKind, StatusHudTooltip};

/// How long the pointer rests on a segment before its tooltip shows, so a
/// pointer passing over the bar does not flash one.
const STATUS_TOOLTIP_DELAY: Duration = Duration::from_millis(450);

#[derive(Debug, Clone)]
pub(super) struct StatusHudRebuildInputs {
    pub(super) position: StatusPosition,
    pub(super) style: StatusBarStyle,
    pub(super) screen_width: u32,
    pub(super) screen_height: u32,
}

/// Cached geometry and pointer interaction state for the status HUD.
#[derive(Debug, Default)]
pub struct StatusHudState {
    pub(in crate::input::state) hover: Option<StatusHudSegmentKind>,
    /// When the pointer started resting on the hovered segment.
    hover_since: Option<Instant>,
    /// The tooltip laid out for this frame, once the hover delay passed.
    tooltip: Option<StatusHudTooltip>,
    pub(in crate::input::state) layout: Option<StatusHudLayout>,
    pub(super) rebuild_inputs: Option<StatusHudRebuildInputs>,
    pub(in crate::input::state) press_pending: bool,
}

impl StatusHudState {
    pub fn is_effectively_visible(&self) -> bool {
        self.rebuild_inputs.is_some() && self.layout.is_some()
    }

    pub fn layout(&self) -> Option<&StatusHudLayout> {
        self.layout.as_ref()
    }

    pub fn hover(&self) -> Option<StatusHudSegmentKind> {
        self.hover
    }

    /// The hovered segment once the pointer has rested on it long enough.
    pub(crate) fn tooltip_segment(&self, now: Instant) -> Option<StatusHudSegmentKind> {
        let since = self.hover_since?;
        (now.saturating_duration_since(since) >= STATUS_TOOLTIP_DELAY)
            .then_some(self.hover)
            .flatten()
    }

    /// Time left before a hovered segment's tooltip is due; `None` when there
    /// is no hover or the tooltip is already up.
    pub(crate) fn tooltip_wake_after(&self, now: Instant) -> Option<Duration> {
        if self.tooltip.is_some() {
            return None;
        }
        let since = self.hover_since?;
        self.hover?;

        Some(STATUS_TOOLTIP_DELAY.saturating_sub(now.saturating_duration_since(since)))
    }

    pub(crate) fn tooltip(&self) -> Option<&StatusHudTooltip> {
        self.tooltip.as_ref()
    }

    pub(crate) fn set_tooltip(&mut self, tooltip: Option<StatusHudTooltip>) {
        self.tooltip = tooltip;
    }

    pub(super) fn rebuild_inputs(&self) -> Option<StatusHudRebuildInputs> {
        self.rebuild_inputs.clone()
    }

    pub(super) fn replace_layout(
        &mut self,
        inputs: StatusHudRebuildInputs,
        layout: Option<StatusHudLayout>,
    ) {
        self.rebuild_inputs = Some(inputs);
        self.layout = layout;
    }

    pub(crate) fn clear_layout(&mut self) {
        self.layout = None;
        self.rebuild_inputs = None;
        self.hover = None;
        self.hover_since = None;
        self.tooltip = None;
    }

    pub(crate) fn clear_hover(&mut self) -> bool {
        self.hover_since = None;
        self.tooltip = None;
        self.hover.take().is_some()
    }

    pub(crate) fn update_hover(&mut self, hover: Option<StatusHudSegmentKind>) -> bool {
        if self.hover == hover {
            return false;
        }
        self.hover = hover;
        self.hover_since = hover.map(|_| Instant::now());
        self.tooltip = None;
        true
    }

    pub(in crate::input::state) fn set_press_pending(&mut self) {
        self.press_pending = true;
    }

    pub(in crate::input::state) fn clear_press_pending(&mut self) {
        self.press_pending = false;
    }

    pub(in crate::input::state) fn take_press_pending(&mut self) -> bool {
        std::mem::take(&mut self.press_pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_updates_only_on_identity_changes() {
        let mut state = StatusHudState::default();
        assert!(state.update_hover(Some(StatusHudSegmentKind::Tool)));
        assert!(!state.update_hover(Some(StatusHudSegmentKind::Tool)));
        assert!(state.clear_hover());
        assert!(!state.clear_hover());
    }

    /// A tooltip waits for the pointer to rest: it is not due at once, the
    /// event loop is told when it will be, and moving to another segment
    /// starts the wait again.
    #[test]
    fn a_segment_tooltip_waits_for_the_pointer_to_rest() {
        let mut state = StatusHudState::default();
        state.update_hover(Some(StatusHudSegmentKind::Board));
        let since = state.hover_since.expect("hover start");

        assert_eq!(state.tooltip_segment(since), None);
        assert_eq!(state.tooltip_wake_after(since), Some(STATUS_TOOLTIP_DELAY));
        assert_eq!(
            state.tooltip_segment(since + STATUS_TOOLTIP_DELAY),
            Some(StatusHudSegmentKind::Board)
        );

        state.update_hover(Some(StatusHudSegmentKind::Help));
        let restarted = state.hover_since.expect("new hover start");
        assert!(restarted >= since);
        assert_eq!(state.tooltip_segment(restarted), None);

        state.clear_hover();
        assert_eq!(state.tooltip_wake_after(restarted), None);
        assert_eq!(
            state.tooltip_segment(restarted + STATUS_TOOLTIP_DELAY),
            None
        );
    }

    #[test]
    fn taking_a_press_clears_it() {
        let mut state = StatusHudState::default();
        state.set_press_pending();
        assert!(state.take_press_pending());
        assert!(!state.take_press_pending());
    }
}
