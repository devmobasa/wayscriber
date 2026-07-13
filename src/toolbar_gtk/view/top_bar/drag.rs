//! GTK top-strip move-drag mechanics.
//!
//! Coalesces stable start-relative gesture coordinates while the GTK input
//! surface remains parked, then reports explicit lifecycle phases to the backend.

use super::*;

/// Keep only the newest start-relative `GestureDrag` offset and apply it once
/// per compositor frame. The gesture-owning surface stays stationary, so the
/// offset remains in one stable coordinate space for the whole drag.
#[derive(Default)]
pub(in crate::toolbar_gtk::view) struct FrameCoalescedDrag {
    next_generation: Cell<u64>,
    pending: RefCell<VecDeque<(u64, DragFrame)>>,
}

pub(in crate::toolbar_gtk::view) struct DragFrame {
    pub(in crate::toolbar_gtk::view) delta: (f64, f64),
    pub(in crate::toolbar_gtk::view) phase: GtkToolbarDragPhase,
}

impl FrameCoalescedDrag {
    pub(in crate::toolbar_gtk::view) fn begin(&self) -> u64 {
        let generation = self.next_generation.get().wrapping_add(1);
        self.next_generation.set(generation);
        generation
    }

    pub(in crate::toolbar_gtk::view) fn update(&self, generation: u64, dx: f64, dy: f64) {
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

    pub(in crate::toolbar_gtk::view) fn end(&self, generation: u64, dx: f64, dy: f64) {
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

    pub(in crate::toolbar_gtk::view) fn take_frame(&self, generation: u64) -> Option<DragFrame> {
        let mut pending = self.pending.borrow_mut();
        let index = pending
            .iter()
            .position(|(queued_generation, _)| *queued_generation == generation)?;
        pending.remove(index).map(|(_, frame)| frame)
    }
}

pub(in crate::toolbar_gtk::view) fn drag_frame_position(
    origin: (f64, f64),
    delta: (f64, f64),
) -> (f64, f64) {
    (origin.0 + delta.0, origin.1 + delta.1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::toolbar_gtk::view) enum CancelledDragAction {
    Ignore,
    Reveal,
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::toolbar_gtk::view) struct ReservedDragSequence(u64);

impl ReservedDragSequence {
    pub(in crate::toolbar_gtk::view) fn reserve(sequence: &Cell<u64>) -> Self {
        Self(sequence.get().wrapping_add(1))
    }

    pub(in crate::toolbar_gtk::view) fn value(self) -> u64 {
        self.0
    }

    pub(in crate::toolbar_gtk::view) fn publish(self, sequence: &Cell<u64>) {
        sequence.set(self.0);
    }
}

pub(in crate::toolbar_gtk::view) fn cancelled_drag_action(
    generation: u64,
    ready_generation: u64,
) -> CancelledDragAction {
    if generation == 0 {
        CancelledDragAction::Ignore
    } else if ready_generation == generation {
        CancelledDragAction::Finish
    } else {
        CancelledDragAction::Reveal
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::toolbar_gtk::view) fn cancel_move_drag(
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
        super::super::set_drag_visual_hidden(window, visual, kind, false);
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
        super::super::set_drag_visual_hidden(window, visual, kind, false);
    }
}

impl TopBar {
    /// Park the GTK input surface at its origin while the main overlay renders
    /// the moving preview. Moving this surface during the gesture changes GTK's
    /// local coordinate space and makes fast drags lag, overshoot, or reverse.
    /// The backend moves the transparent surface after the gesture ends.
    pub(super) fn attach_move_drag(&mut self, grip: &gtk4::DrawingArea) {
        if let Some(cancel) = self.move_drag_cancel.take() {
            cancel();
        }
        if let Some(previous) = self.move_drag.take() {
            previous.reset();
            self.window.remove_controller(&previous);
        }
        let drag = gtk4::GestureDrag::new();
        let window = self.window.clone();
        let feedback = self.feedback.clone();
        let drag_active = self.drag_active.clone();
        let offsets = self.offsets.clone();
        let base_x = self.base_x.clone();
        let seq = self.offset_seq.clone();
        let pending = Rc::new(FrameCoalescedDrag::default());
        let active_generation = Rc::new(Cell::new(0));
        let ready_generation = Rc::new(Cell::new(0));
        let drag_origin = Rc::new(Cell::new((0.0, 0.0)));

        let begin_active = drag_active.clone();
        let begin_blocked = self.drag_blocked.clone();
        let begin_pending = pending.clone();
        let begin_generation = active_generation.clone();
        let begin_ready = ready_generation.clone();
        let begin_origin = drag_origin.clone();
        let begin_window = window.downgrade();
        let begin_visual = self.root.downgrade();
        let begin_grip = grip.downgrade();
        let frame_offsets = offsets.clone();
        let frame_feedback = feedback.clone();
        let frame_base = base_x.clone();
        let frame_seq = seq.clone();
        drag.connect_drag_begin(move |gesture, start_x, start_y| {
            if begin_blocked.get() {
                gesture.set_state(gtk4::EventSequenceState::Denied);
                return;
            }
            let (Some(frame_window), Some(visual), Some(grip)) = (
                begin_window.upgrade(),
                begin_visual.upgrade(),
                begin_grip.upgrade(),
            ) else {
                gesture.set_state(gtk4::EventSequenceState::Denied);
                return;
            };
            let start = gtk4::graphene::Point::new(start_x as f32, start_y as f32);
            if !grip
                .compute_bounds(&frame_window)
                .is_some_and(|bounds| bounds.contains_point(&start))
            {
                gesture.set_state(gtk4::EventSequenceState::Denied);
                return;
            }
            begin_active.set(true);
            let generation = begin_pending.begin();
            begin_generation.set(generation);
            begin_origin.set(frame_offsets.get());
            let start_seq = ReservedDragSequence::reserve(&frame_seq);
            let origin = begin_origin.get();
            let start = GtkToolbarFeedback::SetTopOffset {
                x: origin.0,
                y: origin.1,
                surface_size: crate::toolbar_gtk::GtkToolbarSurfaceSize::from_window(
                    &frame_window,
                ),
                seq: start_seq.value(),
                phase: GtkToolbarDragPhase::Start,
            };
            super::super::set_drag_visual_hidden(
                &frame_window,
                &visual,
                GtkToolbarKind::Top,
                true,
            );
            let start_feedback = frame_feedback.clone();
            let start_window = frame_window.clone();
            let start_visual = visual.clone();
            let start_active = begin_active.clone();
            let start_generation = begin_generation.clone();
            let start_ready = begin_ready.clone();
            let start_sequence = frame_seq.clone();
            super::super::after_next_surface_paint(&frame_window, move || {
                if start_generation.get() != generation {
                    return;
                }
                if start_feedback.send(start).is_ok() {
                    start_seq.publish(&start_sequence);
                    start_ready.set(generation);
                } else {
                    super::super::set_drag_visual_hidden(
                        &start_window,
                        &start_visual,
                        GtkToolbarKind::Top,
                        false,
                    );
                    start_active.set(false);
                    start_generation.set(0);
                }
            });

            let pending = begin_pending.clone();
            let window = frame_window.clone();
            let offsets = frame_offsets.clone();
            let feedback = frame_feedback.clone();
            let base_x = frame_base.clone();
            let seq = frame_seq.clone();
            let drag_active = begin_active.clone();
            let active_generation = begin_generation.clone();
            let ready_generation = begin_ready.clone();
            let drag_origin = begin_origin.clone();
            frame_window.add_tick_callback(move |_, _| {
                if active_generation.get() != generation {
                    return gtk4::glib::ControlFlow::Break;
                }
                if ready_generation.get() != generation {
                    return gtk4::glib::ControlFlow::Continue;
                }
                let Some(frame) = pending.take_frame(generation) else {
                    return gtk4::glib::ControlFlow::Continue;
                };
                let base = base_x.get();
                let (cx, cy) = offsets.get();
                let (mut x, mut y) = drag_frame_position(drag_origin.get(), frame.delta);
                (x, y) =
                    clamp_drag_offsets(&window, (x, y), (base, BASE_MARGIN.0 as f64), END_MARGIN);
                offsets.set((x, y));
                seq.set(seq.get() + 1);
                crate::toolbar_gtk::drag_debug_log(format!(
                    "top frame generation={generation} seq={} phase={:?} delta=({:.3},{:.3}) origin=({:.3},{:.3}) before=({cx:.3},{cy:.3}) preview=({x:.3},{y:.3}) parked_margin=({}, {}) size={}x{}",
                    seq.get(),
                    frame.phase,
                    frame.delta.0,
                    frame.delta.1,
                    drag_origin.get().0,
                    drag_origin.get().1,
                    window.margin(Edge::Left),
                    window.margin(Edge::Top),
                    window.width(),
                    window.height(),
                ));
                let _ = feedback.send(GtkToolbarFeedback::SetTopOffset {
                    x,
                    y,
                    surface_size: crate::toolbar_gtk::GtkToolbarSurfaceSize::from_window(&window),
                    seq: seq.get(),
                    phase: frame.phase,
                });
                if frame.phase.is_end() {
                    if active_generation.get() == generation {
                        drag_active.set(false);
                        active_generation.set(0);
                        ready_generation.set(0);
                    }
                    gtk4::glib::ControlFlow::Break
                } else {
                    gtk4::glib::ControlFlow::Continue
                }
            });
        });

        let update_pending = pending.clone();
        let update_generation = active_generation.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let generation = update_generation.get();
            crate::toolbar_gtk::drag_debug_log(format!(
                "top raw generation={generation} start_relative=({dx:.3},{dy:.3})",
            ));
            update_pending.update(generation, dx, dy);
        });

        let end_pending = pending.clone();
        let end_generation = active_generation.clone();
        drag.connect_drag_end(move |_, dx, dy| {
            crate::toolbar_gtk::drag_debug_log(format!(
                "top end generation={} delta=({dx:.3},{dy:.3})",
                end_generation.get(),
            ));
            end_pending.end(end_generation.get(), dx, dy);
        });

        let cancel_window = window.downgrade();
        let cancel_visual = self.root.downgrade();
        let cancel_feedback = feedback;
        let cancel_active = drag_active;
        let cancel_generation = active_generation;
        let cancel_ready = ready_generation;
        let cancel_offsets = offsets;
        let cancel_seq = seq;
        let cancel: Rc<dyn Fn()> = Rc::new(move || {
            let (Some(window), Some(visual)) = (cancel_window.upgrade(), cancel_visual.upgrade())
            else {
                cancel_active.set(false);
                cancel_generation.set(0);
                cancel_ready.set(0);
                return;
            };
            cancel_move_drag(
                GtkToolbarKind::Top,
                &window,
                &visual,
                &cancel_feedback,
                &cancel_active,
                &cancel_generation,
                &cancel_ready,
                &cancel_offsets,
                &cancel_seq,
            );
        });
        let signal_cancel = cancel.clone();
        drag.connect_cancel(move |_, _| signal_cancel());
        window.add_controller(drag.clone());
        self.move_drag = Some(drag);
        self.move_drag_cancel = Some(cancel);
    }
}

/// Keep a dragged bar inside the same start/end margins enforced by the
/// backend when it persists the final offsets.
pub(in crate::toolbar_gtk::view) fn clamp_drag_offsets(
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
