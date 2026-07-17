//! Side-toolbar adapter for the shared GTK drag lifecycle.

use super::super::drag as shared_drag;
use super::*;

impl SideBar {
    /// Keep the input surface parked and coalesce stable start-relative
    /// motion into the inline preview.
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
        let seq = self.offset_seq.clone();
        let pending = Rc::new(shared_drag::FrameCoalescedDrag::default());
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
            let start_seq = shared_drag::ReservedDragSequence::reserve(&frame_seq);
            let origin = begin_origin.get();
            let start = GtkToolbarFeedback::SetSideOffset {
                x: origin.0,
                y: origin.1,
                surface_size: crate::toolbar_gtk::GtkToolbarSurfaceSize::from_window(
                    &frame_window,
                ),
                seq: start_seq.value(),
                phase: GtkToolbarDragPhase::Start,
            };
            super::super::set_visual_hidden(
                &frame_window,
                &visual,
                GtkToolbarKind::Side,
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
                    super::super::set_visual_hidden(
                        &start_window,
                        &start_visual,
                        GtkToolbarKind::Side,
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
                let (cx, cy) = offsets.get();
                let (next_x, next_y) =
                    shared_drag::drag_frame_position(drag_origin.get(), frame.delta);
                let (x, y) = shared_drag::clamp_drag_offsets(
                    &window,
                    (next_x, next_y),
                    (BASE_MARGIN.1 as f64, BASE_MARGIN.0 as f64),
                    END_MARGIN,
                );
                offsets.set((x, y));
                seq.set(seq.get() + 1);
                crate::toolbar_gtk::drag_debug_log(format!(
                    "side frame generation={generation} seq={} phase={:?} delta=({:.3},{:.3}) origin=({:.3},{:.3}) before=({cx:.3},{cy:.3}) preview=({x:.3},{y:.3}) parked_margin=({}, {}) size={}x{}",
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
                let _ = feedback.send(GtkToolbarFeedback::SetSideOffset {
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
                "side raw generation={generation} start_relative=({dx:.3},{dy:.3})",
            ));
            update_pending.update(generation, dx, dy);
        });

        let end_pending = pending.clone();
        let end_generation = active_generation.clone();
        drag.connect_drag_end(move |_, dx, dy| {
            crate::toolbar_gtk::drag_debug_log(format!(
                "side end generation={} delta=({dx:.3},{dy:.3})",
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
            shared_drag::cancel_move_drag(
                GtkToolbarKind::Side,
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
