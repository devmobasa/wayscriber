//! Shared GTK toolbar move-drag state and geometry.
//!
//! Top and side toolbar adapters both keep their GTK input surface parked
//! while the main overlay renders a moving preview. This module owns the
//! lifecycle mechanics that must remain identical across those adapters.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

use gtk4::prelude::*;

use super::super::widgets::FeedbackSender;
use super::super::{GtkToolbarDragPhase, GtkToolbarFeedback, GtkToolbarKind};

/// Keep only the newest start-relative `GestureDrag` offset and apply it once
/// per compositor frame. The gesture-owning surface stays stationary, so the
/// offset remains in one stable coordinate space for the whole drag.
#[derive(Default)]
pub(super) struct FrameCoalescedDrag {
    next_generation: Cell<u64>,
    pending: RefCell<VecDeque<(u64, DragFrame)>>,
}

pub(super) struct DragFrame {
    pub(super) delta: (f64, f64),
    pub(super) phase: GtkToolbarDragPhase,
}

impl FrameCoalescedDrag {
    pub(super) fn begin(&self) -> u64 {
        let generation = self.next_generation.get().wrapping_add(1);
        self.next_generation.set(generation);
        generation
    }

    pub(super) fn update(&self, generation: u64, dx: f64, dy: f64) {
        let mut pending = self.pending.borrow_mut();
        if let Some((queued_generation, frame)) = pending.back_mut()
            && *queued_generation == generation
            && frame.phase != GtkToolbarDragPhase::End
        {
            frame.delta = (dx, dy);
            return;
        }
        pending.push_back((
            generation,
            DragFrame {
                delta: (dx, dy),
                phase: GtkToolbarDragPhase::Move,
            },
        ));
    }

    pub(super) fn end(&self, generation: u64, dx: f64, dy: f64) {
        let mut pending = self.pending.borrow_mut();
        if let Some((queued_generation, frame)) = pending.back_mut()
            && *queued_generation == generation
            && frame.phase != GtkToolbarDragPhase::End
        {
            frame.delta = (dx, dy);
            frame.phase = GtkToolbarDragPhase::End;
            return;
        }
        pending.push_back((
            generation,
            DragFrame {
                // Start-relative offsets are idempotent while the input
                // surface is parked, so replaying the final coordinate cannot
                // accumulate motion or produce a release jump.
                delta: (dx, dy),
                phase: GtkToolbarDragPhase::End,
            },
        ));
    }

    pub(super) fn take_frame(&self, generation: u64) -> Option<DragFrame> {
        let mut pending = self.pending.borrow_mut();
        let index = pending
            .iter()
            .position(|(queued_generation, _)| *queued_generation == generation)?;
        pending.remove(index).map(|(_, frame)| frame)
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
    pub(super) fn reserve(sequence: &Cell<u64>) -> Self {
        Self(sequence.get().wrapping_add(1))
    }

    pub(super) fn value(self) -> u64 {
        self.0
    }

    pub(super) fn publish(self, sequence: &Cell<u64>) {
        sequence.set(self.0);
    }
}

pub(super) fn cancelled_drag_action(generation: u64, ready_generation: u64) -> CancelledDragAction {
    if generation == 0 {
        CancelledDragAction::Ignore
    } else if ready_generation == generation {
        CancelledDragAction::Finish
    } else {
        CancelledDragAction::Reveal
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn cancel_move_drag(
    kind: GtkToolbarKind,
    window: &gtk4::Window,
    visual: &gtk4::Box,
    feedback: &FeedbackSender,
    drag_active: &Cell<bool>,
    active_generation: &Cell<u64>,
    ready_generation: &Cell<u64>,
    offsets: &Cell<(f64, f64)>,
    seq: &Cell<u64>,
) {
    let generation = active_generation.replace(0);
    let action = cancelled_drag_action(generation, ready_generation.replace(0));
    if action == CancelledDragAction::Ignore {
        return;
    }
    drag_active.set(false);
    crate::toolbar_gtk::drag_debug_log(format!(
        "{kind:?} cancel generation={generation} action={action:?}"
    ));

    if action == CancelledDragAction::Reveal {
        super::set_visual_hidden(window, visual, kind, false);
        return;
    }

    seq.set(seq.get().wrapping_add(1));
    let (x, y) = offsets.get();
    let surface_size = crate::toolbar_gtk::GtkToolbarSurfaceSize::from_window(window);
    let end = match kind {
        GtkToolbarKind::Top => GtkToolbarFeedback::SetTopOffset {
            x,
            y,
            surface_size,
            seq: seq.get(),
            phase: GtkToolbarDragPhase::End,
        },
        GtkToolbarKind::Side => GtkToolbarFeedback::SetSideOffset {
            x,
            y,
            surface_size,
            seq: seq.get(),
            phase: GtkToolbarDragPhase::End,
        },
    };
    if feedback.send(end).is_err() {
        super::set_visual_hidden(window, visual, kind, false);
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
    fn updates_are_coalesced_to_the_latest_start_relative_offset() {
        let drag = FrameCoalescedDrag::default();
        let first = drag.begin();
        drag.update(first, 2.0, 3.0);
        drag.update(first, 5.0, 7.0);

        let frame = drag.take_frame(first).expect("latest motion is pending");
        assert_eq!(frame.delta, (5.0, 7.0));
        assert_eq!(frame.phase, GtkToolbarDragPhase::Move);
        assert!(drag.take_frame(first).is_none());

        drag.end(first, 5.0, 7.0);
        let frame = drag.take_frame(first).expect("drag end is pending");
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
    fn consecutive_drags_keep_separate_final_frames() {
        let drag = FrameCoalescedDrag::default();
        let first = drag.begin();
        drag.update(first, 4.0, 6.0);
        drag.end(first, 4.0, 6.0);
        let second = drag.begin();
        drag.update(second, 1.0, 2.0);

        let first_frame = drag.take_frame(first).expect("first drag end is retained");
        assert_eq!(first_frame.delta, (4.0, 6.0));
        assert_eq!(first_frame.phase, GtkToolbarDragPhase::End);

        let second_frame = drag
            .take_frame(second)
            .expect("second drag motion is retained");
        assert_eq!(second_frame.delta, (1.0, 2.0));
        assert_eq!(second_frame.phase, GtkToolbarDragPhase::Move);
    }

    #[test]
    fn cancellation_reveals_before_start_and_finishes_after_start() {
        assert_eq!(cancelled_drag_action(0, 0), CancelledDragAction::Ignore);
        assert_eq!(cancelled_drag_action(4, 0), CancelledDragAction::Reveal);
        assert_eq!(cancelled_drag_action(4, 4), CancelledDragAction::Finish);
    }

    #[test]
    fn cancellation_before_start_does_not_advance_sequence_or_rehide_visual() {
        let sequence = Cell::new(7);
        let reserved = ReservedDragSequence::reserve(&sequence);

        assert_eq!(reserved.value(), 8);
        assert_eq!(cancelled_drag_action(4, 0), CancelledDragAction::Reveal);
        assert_eq!(sequence.get(), 7);
        assert!(!super::super::drag_visual_should_be_hidden(
            None,
            GtkToolbarKind::Top,
            false,
            sequence.get(),
            7,
        ));
    }

    #[test]
    fn successful_start_publishes_the_reserved_sequence() {
        let sequence = Cell::new(7);
        let reserved = ReservedDragSequence::reserve(&sequence);
        reserved.publish(&sequence);
        assert_eq!(sequence.get(), 8);
    }
}
