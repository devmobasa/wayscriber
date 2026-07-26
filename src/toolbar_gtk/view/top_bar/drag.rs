//! GTK top-strip move-drag mechanics.
//!
//! Gesture callbacks publish bounded intents. `TopBar` remains the sole owner
//! of drag state and resolves those intents from the supervised GTK update
//! loop, so callbacks never share mutable lifecycle state.

use super::super::drag::{CancelledDragAction, DragIntent, ViewIntent};
use super::*;

impl TopBar {
    /// Park the GTK input surface at its origin while the main overlay renders
    /// the moving preview. Moving this surface during the gesture changes GTK's
    /// local coordinate space and makes fast drags lag, overshoot, or reverse.
    /// The backend moves the transparent surface after the gesture ends.
    pub(super) fn attach_move_drag(&mut self, grip: &gtk4::DrawingArea) {
        self.cancel_move_drag();
        if let Some(previous) = self.move_drag.take() {
            previous.reset();
            self.window.remove_controller(&previous);
        }

        self.move_drag_controller = self.move_drag_controller.wrapping_add(1);
        let controller = self.move_drag_controller;
        let drag = gtk4::GestureDrag::new();
        let begin_window = self.window.downgrade();
        let begin_grip = grip.downgrade();
        let begin_intents = self.intents.clone();
        drag.connect_drag_begin(move |gesture, start_x, start_y| {
            let (Some(window), Some(grip)) = (begin_window.upgrade(), begin_grip.upgrade()) else {
                gesture.set_state(gtk4::EventSequenceState::Denied);
                return;
            };
            let start = gtk4::graphene::Point::new(start_x as f32, start_y as f32);
            if !grip
                .compute_bounds(&window)
                .is_some_and(|bounds| bounds.contains_point(&start))
                || !begin_intents.send(ViewIntent::Top(DragIntent::Begin { controller }))
            {
                gesture.set_state(gtk4::EventSequenceState::Denied);
            }
        });

        let end_intents = self.intents.clone();
        drag.connect_drag_end(move |_, dx, dy| {
            crate::toolbar_gtk::drag_debug_log(format!(
                "top raw end controller={controller} delta=({dx:.3},{dy:.3})"
            ));
            let _ = end_intents.send(ViewIntent::Top(DragIntent::End { controller, dx, dy }));
        });

        let cancel_intents = self.intents.clone();
        drag.connect_cancel(move |_, _| {
            let _ = cancel_intents.send(ViewIntent::Top(DragIntent::Cancel { controller }));
        });

        drag.set_propagation_phase(if self.drag_blocked {
            gtk4::PropagationPhase::None
        } else {
            gtk4::PropagationPhase::Bubble
        });
        self.window.add_controller(drag.clone());
        self.move_drag = Some(drag);
    }

    pub(in crate::toolbar_gtk::view) async fn handle_drag_intent(
        &mut self,
        intent: DragIntent,
    ) -> Result<(), String> {
        match intent {
            DragIntent::Begin { controller } => {
                if controller != self.move_drag_controller || self.drag_blocked {
                    return Ok(());
                }
                self.cancel_move_drag_inner()?;
                let generation = self.drag.begin();
                let origin = self.drag.offsets();
                crate::toolbar_gtk::drag_debug_log(format!(
                    "top begin controller={controller} generation={generation} origin=({:.3},{:.3})",
                    origin.0, origin.1
                ));
                super::super::set_visual_hidden(
                    &self.window,
                    &self.root,
                    GtkToolbarKind::Top,
                    true,
                );
                let window = self.window.clone();
                match gtk4::glib::future_with_timeout(
                    std::time::Duration::from_millis(500),
                    super::super::after_next_surface_paint_counter(&window),
                )
                .await
                {
                    Ok(_) => self.start_drag_after_paint(generation),
                    Err(_) => {
                        self.cancel_move_drag_inner()?;
                        Err(
                            "timed out waiting for the top toolbar's hidden drag surface to paint"
                                .to_string(),
                        )
                    }
                }
            }
            DragIntent::Tick { generation } => self.apply_drag_frame(generation),
            DragIntent::End { controller, dx, dy } => {
                if controller == self.move_drag_controller {
                    self.drag.queue_end(dx, dy);
                }
                Ok(())
            }
            DragIntent::Cancel { controller } => {
                if controller == self.move_drag_controller {
                    self.cancel_move_drag_inner()?;
                }
                Ok(())
            }
        }
    }

    fn start_drag_after_paint(&mut self, generation: u64) -> Result<(), String> {
        let Some(sequence) = self.drag.reserved_sequence(generation) else {
            return Ok(());
        };
        let origin = self.drag.offsets();
        let start = GtkToolbarFeedback::SetTopOffset {
            x: origin.0,
            y: origin.1,
            surface_size: crate::toolbar_gtk::GtkToolbarSurfaceSize::from_window(&self.window),
            seq: sequence,
            phase: GtkToolbarDragPhase::Start,
        };
        if self.feedback.send(start).is_err() {
            super::super::set_visual_hidden(&self.window, &self.root, GtkToolbarKind::Top, false);
            self.drag.finish();
            return Err(
                "GTK feedback mailbox closed while starting a top-toolbar drag".to_string(),
            );
        }
        if !self.drag.mark_ready(generation) {
            return Ok(());
        }

        let intents = self.intents.clone();
        let tick = self.window.add_tick_callback(move |_, _| {
            if intents.send(ViewIntent::Top(DragIntent::Tick { generation })) {
                gtk4::glib::ControlFlow::Continue
            } else {
                gtk4::glib::ControlFlow::Break
            }
        });
        self.drag.set_tick(tick);
        Ok(())
    }

    fn apply_drag_frame(&mut self, generation: u64) -> Result<(), String> {
        let live_delta = self.move_drag.as_ref().and_then(gtk4::GestureDrag::offset);
        let Some(frame) = self.drag.take_frame(generation, live_delta) else {
            return Ok(());
        };
        let (before_x, before_y) = self.drag.offsets();
        let (mut x, mut y) = super::super::drag::drag_frame_position(frame.origin, frame.delta);
        (x, y) = super::super::drag::clamp_drag_offsets(
            &self.window,
            (x, y),
            (self.base_x, BASE_MARGIN.0 as f64),
            END_MARGIN,
        );
        self.drag.set_offsets((x, y));
        let sequence = self.drag.advance_sequence();
        crate::toolbar_gtk::drag_debug_log(format!(
            "top frame generation={generation} seq={sequence} phase={:?} delta=({:.3},{:.3}) origin=({:.3},{:.3}) before=({before_x:.3},{before_y:.3}) preview=({x:.3},{y:.3}) parked_margin=({}, {}) size={}x{}",
            frame.phase,
            frame.delta.0,
            frame.delta.1,
            frame.origin.0,
            frame.origin.1,
            self.window.margin(Edge::Left),
            self.window.margin(Edge::Top),
            self.window.width(),
            self.window.height(),
        ));
        if self
            .feedback
            .send(GtkToolbarFeedback::SetTopOffset {
                x,
                y,
                surface_size: crate::toolbar_gtk::GtkToolbarSurfaceSize::from_window(&self.window),
                seq: sequence,
                phase: frame.phase,
            })
            .is_err()
        {
            super::super::set_visual_hidden(&self.window, &self.root, GtkToolbarKind::Top, false);
            self.drag.finish();
            return Err("GTK feedback mailbox closed while moving the top toolbar".to_string());
        }
        if frame.phase.is_end() {
            self.drag.finish();
        }
        Ok(())
    }

    fn cancel_move_drag_inner(&mut self) -> Result<(), String> {
        match self.drag.cancel_action() {
            CancelledDragAction::Ignore => Ok(()),
            CancelledDragAction::Reveal => {
                super::super::set_visual_hidden(
                    &self.window,
                    &self.root,
                    GtkToolbarKind::Top,
                    false,
                );
                Ok(())
            }
            CancelledDragAction::Finish => {
                let (x, y) = self.drag.offsets();
                let sequence = self.drag.advance_sequence();
                self.feedback
                    .send(GtkToolbarFeedback::SetTopOffset {
                        x,
                        y,
                        surface_size: crate::toolbar_gtk::GtkToolbarSurfaceSize::from_window(
                            &self.window,
                        ),
                        seq: sequence,
                        phase: GtkToolbarDragPhase::End,
                    })
                    .map_err(|()| {
                        "GTK feedback mailbox closed while cancelling a top-toolbar drag"
                            .to_string()
                    })
            }
        }
    }

    pub(super) fn cancel_move_drag(&mut self) {
        if let Err(error) = self.cancel_move_drag_inner() {
            super::super::set_visual_hidden(&self.window, &self.root, GtkToolbarKind::Top, false);
            self.intents.report_failure(error);
        }
    }

    pub(super) fn set_drag_blocked(&mut self, blocked: bool) {
        if blocked && !self.drag_blocked {
            self.cancel_move_drag();
        }
        self.drag_blocked = blocked;
        if let Some(drag) = self.move_drag.as_ref() {
            drag.set_propagation_phase(if blocked {
                gtk4::PropagationPhase::None
            } else {
                gtk4::PropagationPhase::Bubble
            });
        }
    }
}
