//! Shared GTK toolbar move-drag state and geometry.
//!
//! Top and side toolbar adapters both keep their GTK input surface parked
//! while the main overlay renders a moving preview. This module owns the
//! lifecycle mechanics that must remain identical across those adapters.

use gtk4::prelude::*;
use tokio::sync::mpsc;

use super::super::GtkToolbarDragPhase;

const VIEW_INTENT_CAPACITY: usize = 32;

#[derive(Debug, Clone, Copy)]
pub(in crate::toolbar_gtk) enum DragIntent {
    Begin { controller: u64 },
    Tick { generation: u64 },
    End { controller: u64, dx: f64, dy: f64 },
    Cancel { controller: u64 },
}

#[derive(Debug, Clone, Copy)]
pub(in crate::toolbar_gtk) enum ViewIntent {
    Top(DragIntent),
    Side(DragIntent),
    Tooltip(TooltipIntent),
}

#[derive(Debug, Clone, Copy)]
pub(in crate::toolbar_gtk) enum TooltipIntent {
    Activated { source: u64 },
}

#[derive(Clone)]
pub(in crate::toolbar_gtk) struct ViewIntentSender {
    intents: mpsc::Sender<ViewIntent>,
    failures: mpsc::UnboundedSender<String>,
}

impl ViewIntentSender {
    pub(in crate::toolbar_gtk) fn channel() -> (
        Self,
        mpsc::Receiver<ViewIntent>,
        mpsc::UnboundedReceiver<String>,
    ) {
        let (intents, intent_rx) = mpsc::channel(VIEW_INTENT_CAPACITY);
        let (failures, failure_rx) = mpsc::unbounded_channel();
        (Self { intents, failures }, intent_rx, failure_rx)
    }

    pub(super) fn send(&self, intent: ViewIntent) -> bool {
        match self.intents.try_send(intent) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                let _ = self.failures.send(
                    "GTK view-intent queue exhausted; restoring built-in toolbars".to_string(),
                );
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        }
    }

    pub(super) fn report_failure(&self, failure: String) {
        let _ = self.failures.send(failure);
    }
}

/// Keep only the newest start-relative `GestureDrag` offset and apply it once
/// per compositor frame. The gesture-owning surface stays stationary, so the
/// offset remains in one stable coordinate space for the whole drag.
#[derive(Default)]
pub(super) struct DragOwnerState {
    next_generation: u64,
    active: Option<ActiveDrag>,
    offsets: (f64, f64),
    sequence: u64,
    tick: Option<gtk4::TickCallbackId>,
}

struct ActiveDrag {
    generation: u64,
    origin: (f64, f64),
    reserved: ReservedDragSequence,
    ready: bool,
    end_delta: Option<(f64, f64)>,
}

pub(super) struct DragFrame {
    pub(super) origin: (f64, f64),
    pub(super) delta: (f64, f64),
    pub(super) phase: GtkToolbarDragPhase,
}

impl DragOwnerState {
    pub(super) fn begin(&mut self) -> u64 {
        self.remove_tick();
        let generation = self.next_generation.wrapping_add(1);
        self.next_generation = generation;
        let reserved = ReservedDragSequence::reserve(self.sequence);
        self.active = Some(ActiveDrag {
            generation,
            origin: self.offsets,
            reserved,
            ready: false,
            end_delta: None,
        });
        generation
    }

    pub(super) fn active(&self) -> bool {
        self.active.is_some()
    }

    pub(super) fn reserved_sequence(&self, generation: u64) -> Option<u64> {
        self.active
            .as_ref()
            .filter(|active| active.generation == generation)
            .map(|active| active.reserved.value())
    }

    pub(super) fn mark_ready(&mut self, generation: u64) -> bool {
        let Some(active) = self.active.as_mut() else {
            return false;
        };
        if active.generation != generation {
            return false;
        }
        active.ready = true;
        self.sequence = active.reserved.value();
        true
    }

    pub(super) fn set_tick(&mut self, tick: gtk4::TickCallbackId) {
        self.remove_tick();
        self.tick = Some(tick);
    }

    pub(super) fn queue_end(&mut self, dx: f64, dy: f64) {
        if let Some(active) = self.active.as_mut() {
            active.end_delta = Some((dx, dy));
        }
    }

    pub(super) fn take_frame(
        &mut self,
        generation: u64,
        live_delta: Option<(f64, f64)>,
    ) -> Option<DragFrame> {
        let active = self.active.as_mut()?;
        if active.generation != generation || !active.ready {
            return None;
        }
        if let Some(delta) = active.end_delta.take() {
            return Some(DragFrame {
                origin: active.origin,
                delta,
                phase: GtkToolbarDragPhase::End,
            });
        }
        live_delta.map(|delta| DragFrame {
            origin: active.origin,
            delta,
            phase: GtkToolbarDragPhase::Move,
        })
    }

    #[cfg(test)]
    pub(super) fn origin(&self) -> Option<(f64, f64)> {
        self.active.as_ref().map(|active| active.origin)
    }

    pub(super) fn offsets(&self) -> (f64, f64) {
        self.offsets
    }

    pub(super) fn set_offsets(&mut self, offsets: (f64, f64)) {
        self.offsets = offsets;
    }

    pub(super) fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(super) fn advance_sequence(&mut self) -> u64 {
        self.sequence = self.sequence.wrapping_add(1);
        self.sequence
    }

    pub(super) fn cancel_action(&mut self) -> CancelledDragAction {
        let Some(active) = self.active.take() else {
            return CancelledDragAction::Ignore;
        };
        self.remove_tick();
        if active.ready {
            CancelledDragAction::Finish
        } else {
            CancelledDragAction::Reveal
        }
    }

    pub(super) fn finish(&mut self) {
        self.active = None;
        self.remove_tick();
    }

    fn remove_tick(&mut self) {
        if let Some(tick) = self.tick.take() {
            tick.remove();
        }
    }
}

impl Drop for DragOwnerState {
    fn drop(&mut self) {
        self.remove_tick();
    }
}

pub(super) fn drag_frame_position(origin: (f64, f64), delta: (f64, f64)) -> (f64, f64) {
    (origin.0 + delta.0, origin.1 + delta.1)
}

/// Convert a floating toolbar offset into the integer layer-shell margin
/// actually applied by GTK, and return the normalized offset represented by
/// that margin.
pub(super) fn rounded_margin_and_offset(base: f64, offset: f64) -> (i32, f64) {
    let margin = (base + offset).round() as i32;
    (margin, margin as f64 - base)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CancelledDragAction {
    Ignore,
    Reveal,
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReservedDragSequence(u64);

impl ReservedDragSequence {
    pub(super) fn reserve(sequence: u64) -> Self {
        Self(sequence.wrapping_add(1))
    }

    pub(super) fn value(self) -> u64 {
        self.0
    }
}

/// Keep a dragged bar inside the same start/end margins enforced by the
/// backend when it persists the final offsets.
pub(super) fn clamp_drag_offsets(
    window: &gtk4::Window,
    (x, y): (f64, f64),
    (base_x, base_y): (f64, f64),
    (end_x, end_y): (f64, f64),
) -> (f64, f64) {
    if let Some(surface) = window.surface()
        && let Some(display) = gtk4::gdk::Display::default()
        && let Some(monitor) = display.monitor_at_surface(&surface)
    {
        let geometry = monitor.geometry();
        let (x, _, _) = crate::backend::wayland::clamp_floating_axis_offset(
            x,
            geometry.width() as f64,
            window.width() as f64,
            base_x,
            end_x,
        );
        let (y, _, _) = crate::backend::wayland::clamp_floating_axis_offset(
            y,
            geometry.height() as f64,
            window.height() as f64,
            base_y,
            end_y,
        );
        return (x, y);
    }
    (x.max(-base_x), y.max(-base_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_reads_the_latest_start_relative_offset_on_each_tick() {
        let mut drag = DragOwnerState::default();
        let first = drag.begin();
        assert_eq!(drag.reserved_sequence(first), Some(1));
        assert!(drag.mark_ready(first));

        let frame = drag
            .take_frame(first, Some((5.0, 7.0)))
            .expect("the test supplied a live gesture offset for the active drag");
        assert_eq!(frame.delta, (5.0, 7.0));
        assert_eq!(frame.phase, GtkToolbarDragPhase::Move);
        assert!(drag.take_frame(first, None).is_none());

        drag.queue_end(8.0, 9.0);
        let frame = drag
            .take_frame(first, Some((10.0, 11.0)))
            .expect("the test queued an end offset for the active drag");
        assert_eq!(frame.delta, (8.0, 9.0));
        assert_eq!(frame.phase, GtkToolbarDragPhase::End);
    }

    #[test]
    fn start_sequence_is_published_only_when_the_surface_is_ready() {
        let mut drag = DragOwnerState::default();
        drag.set_offsets((12.0, 18.0));
        let generation = drag.begin();

        assert_eq!(drag.origin(), Some((12.0, 18.0)));
        assert_eq!(drag.sequence(), 0);
        assert_eq!(drag.reserved_sequence(generation), Some(1));
        assert!(drag.mark_ready(generation));
        assert_eq!(drag.sequence(), 1);
        assert!(!drag.mark_ready(generation.wrapping_add(1)));
    }

    #[test]
    fn queued_end_waits_until_start_is_ready() {
        let mut drag = DragOwnerState::default();
        let generation = drag.begin();
        drag.queue_end(5.0, 7.0);

        assert!(drag.take_frame(generation, None).is_none());
        assert!(drag.mark_ready(generation));
        let frame = drag
            .take_frame(generation, None)
            .expect("the test made the queued end ready");
        assert_eq!(frame.delta, (5.0, 7.0));
        assert_eq!(frame.phase, GtkToolbarDragPhase::End);
    }

    #[test]
    fn rapid_start_relative_updates_do_not_accumulate() {
        let origin = (100.0, 200.0);
        let first = drag_frame_position(origin, (25.0, 40.0));
        let second = drag_frame_position(origin, (80.0, 90.0));

        assert_eq!(first, (125.0, 240.0));
        assert_eq!(second, (180.0, 290.0));
    }

    #[test]
    fn rounded_offset_matches_the_integer_layer_margin() {
        assert_eq!(rounded_margin_and_offset(12.0, 3.6), (16, 4.0));
        assert_eq!(rounded_margin_and_offset(24.0, -24.0), (0, -24.0));
        assert_eq!(rounded_margin_and_offset(100.25, 4.4), (105, 4.75));
    }

    #[test]
    fn consecutive_drags_replace_the_previous_generation() {
        let mut drag = DragOwnerState::default();
        let first = drag.begin();
        assert!(drag.mark_ready(first));
        drag.queue_end(4.0, 6.0);
        let second = drag.begin();
        assert!(drag.mark_ready(second));

        assert!(drag.take_frame(first, None).is_none());
        let second_frame = drag
            .take_frame(second, Some((1.0, 2.0)))
            .expect("the test supplied motion for the replacement drag");
        assert_eq!(second_frame.delta, (1.0, 2.0));
        assert_eq!(second_frame.phase, GtkToolbarDragPhase::Move);
    }

    #[test]
    fn cancellation_reveals_before_start_and_finishes_after_start() {
        let mut drag = DragOwnerState::default();
        assert_eq!(drag.cancel_action(), CancelledDragAction::Ignore);

        let before_ready = drag.begin();
        assert_eq!(drag.cancel_action(), CancelledDragAction::Reveal);
        assert_eq!(drag.sequence(), 0);

        let ready = drag.begin();
        assert_ne!(ready, before_ready);
        assert!(drag.mark_ready(ready));
        assert_eq!(drag.cancel_action(), CancelledDragAction::Finish);
        assert_eq!(drag.sequence(), 1);
    }
}
