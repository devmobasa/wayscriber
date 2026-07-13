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

impl TopBar {
    /// Park the GTK input surface at its origin while the main overlay renders
    /// the moving preview. Moving this surface during the gesture changes GTK's
    /// local coordinate space and makes fast drags lag, overshoot, or reverse.
    /// The backend moves the transparent surface after the gesture ends.
    pub(super) fn attach_move_drag(&self, grip: &gtk4::DrawingArea) {
        let drag = gtk4::GestureDrag::new();
        let window = self.window.clone();
        let feedback = self.feedback.clone();
        let drag_active = self.drag_active.clone();
        let offsets = self.offsets.clone();
        let base_x = self.base_x.clone();
        let seq = self.offset_seq.clone();
        let pending = Rc::new(FrameCoalescedDrag::default());
        let active_generation = Rc::new(Cell::new(0));
        let drag_origin = Rc::new(Cell::new((0.0, 0.0)));

        let begin_active = drag_active.clone();
        let begin_blocked = self.drag_blocked.clone();
        let begin_pending = pending.clone();
        let begin_generation = active_generation.clone();
        let begin_origin = drag_origin.clone();
        let frame_window = window.clone();
        let frame_offsets = offsets.clone();
        let frame_feedback = feedback.clone();
        let frame_base = base_x.clone();
        let frame_seq = seq.clone();
        drag.connect_drag_begin(move |gesture, _, _| {
            if begin_blocked.get() {
                gesture.set_state(gtk4::EventSequenceState::Denied);
                return;
            }
            let Some(grip) = gesture.widget() else {
                return;
            };
            begin_active.set(true);
            let generation = begin_pending.begin();
            begin_generation.set(generation);
            begin_origin.set(frame_offsets.get());
            frame_seq.set(frame_seq.get() + 1);
            let origin = begin_origin.get();
            let _ = frame_feedback.send(GtkToolbarFeedback::SetTopOffset {
                x: origin.0,
                y: origin.1,
                surface_size: crate::toolbar_gtk::GtkToolbarSurfaceSize::from_window(
                    &frame_window,
                ),
                seq: frame_seq.get(),
                phase: GtkToolbarDragPhase::Start,
            });

            let pending = begin_pending.clone();
            let window = frame_window.clone();
            let offsets = frame_offsets.clone();
            let feedback = frame_feedback.clone();
            let base_x = frame_base.clone();
            let seq = frame_seq.clone();
            let drag_active = begin_active.clone();
            let active_generation = begin_generation.clone();
            let drag_origin = begin_origin.clone();
            grip.add_tick_callback(move |_, _| {
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

        drag.connect_drag_end(move |_, dx, dy| {
            crate::toolbar_gtk::drag_debug_log(format!(
                "top end generation={} delta=({dx:.3},{dy:.3})",
                active_generation.get(),
            ));
            pending.end(active_generation.get(), dx, dy);
        });
        grip.add_controller(drag);
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
