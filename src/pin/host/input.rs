//! Pointer, relative-pointer, wheel, and touch routing for pin-owned surfaces.

use smithay_client_toolkit::seat::pointer::{BTN_LEFT, PointerEvent, PointerEventKind};
use wayland_client::{Proxy, QueueHandle, backend::ObjectId, protocol::wl_pointer};
use wayland_protocols::wp::pointer_constraints::zv1::client::zwp_pointer_constraints_v1::Lifetime;

use super::PinHost;
use crate::pin::surface::{Control, control_at};
use crate::pin::surface::{InputOwner, Interaction, ReleaseAction, content_position};
use crate::pin::{PinId, geometry};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PinCursor {
    Default,
    Move,
    Pointer,
}

fn cursor_at(frame: crate::pin::PinFrame, surface_position: (f64, f64)) -> PinCursor {
    let position = content_position(surface_position);
    match control_at(frame, position) {
        Some(Control::Copy | Control::Close) => PinCursor::Pointer,
        None if position.0 >= 0.0
            && position.1 >= 0.0
            && position.0 < f64::from(frame.width)
            && position.1 < f64::from(frame.height) =>
        {
            PinCursor::Move
        }
        None => PinCursor::Default,
    }
}

fn merge_wheel(
    pending: &mut Option<(PinId, (f64, f64), f64)>,
    id: PinId,
    position: (f64, f64),
    steps: f64,
) {
    match pending.as_mut() {
        Some((pending_id, pending_position, total)) if *pending_id == id => {
            *pending_position = position;
            *total += steps;
        }
        Some(slot) => *slot = (id, position, steps),
        None => *pending = Some((id, position, steps)),
    }
}

fn pointer_event_pin<T: Copy>(
    mapped: Option<T>,
    owned_release: Option<T>,
    is_release: bool,
) -> Option<T> {
    if is_release {
        owned_release.or(mapped)
    } else {
        mapped
    }
}

impl PinHost {
    pub(super) fn handle_pointer_frame(
        &mut self,
        pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
        qh: &QueueHandle<Self>,
    ) {
        let Some((_, seat)) = self.pointers.get(&pointer.id()) else {
            return;
        };
        let seat = seat.clone();
        let seat_id = seat.id();
        let mut pending_drag = None;
        let mut pending_wheel: Option<(PinId, (f64, f64), f64)> = None;
        for event in events {
            let mapped = self.by_wl_surface.get(&event.surface.id()).copied();
            let owned_release = matches!(event.kind, PointerEventKind::Release { .. })
                .then(|| {
                    self.pins.iter().find_map(|(id, pin)| {
                        matches!(
                            &pin.interaction,
                            Interaction::PressedControl {
                                owner: InputOwner::Pointer { seat, .. },
                                ..
                            } | Interaction::Dragging {
                                owner: InputOwner::Pointer { seat, .. },
                                ..
                            } if *seat == seat_id
                        )
                        .then_some(*id)
                    })
                })
                .flatten();
            let Some(id) = pointer_event_pin(
                mapped,
                owned_release,
                matches!(event.kind, PointerEventKind::Release { .. }),
            ) else {
                continue;
            };
            match event.kind {
                PointerEventKind::Enter { serial } => {
                    self.cursor_serials.insert(pointer.id(), serial);
                    self.update_pointer_cursor(pointer, id, event.position);
                    if self.locked_pin != Some(id) {
                        let owner = self.pointer_drag_owner(seat_id.clone());
                        let candidate = self.pins.get_mut(&id).and_then(|pin| {
                            if let Some(owner) = owner.as_ref() {
                                pin.fallback_drag_origin(
                                    owner,
                                    event.position,
                                    pin.shell.committed_origin,
                                )
                            } else {
                                pin.update_hover(Some(event.position));
                                None
                            }
                        });
                        if let Some(candidate) = candidate {
                            pending_drag = Some((id, candidate));
                        }
                    }
                }
                PointerEventKind::Motion { .. } => {
                    self.update_pointer_cursor(pointer, id, event.position);
                    if self.locked_pin != Some(id) {
                        let owner = self.pointer_drag_owner(seat_id.clone());
                        let candidate = self.pins.get_mut(&id).and_then(|pin| {
                            if let Some(owner) = owner.as_ref() {
                                pin.fallback_drag_origin(
                                    owner,
                                    event.position,
                                    pin.shell.committed_origin,
                                )
                            } else {
                                pin.update_hover(Some(event.position));
                                None
                            }
                        });
                        if let Some(candidate) = candidate {
                            pending_drag = Some((id, candidate));
                        }
                    }
                }
                PointerEventKind::Leave { .. } => {
                    if let Some(pin) = self.pins.get_mut(&id) {
                        pin.update_hover(None);
                    }
                }
                PointerEventKind::Press { button, .. } if button == BTN_LEFT => {
                    self.arbitrate_new_pointer_owner(id, &seat_id);
                    let owner = InputOwner::Pointer {
                        seat: seat_id.clone(),
                        button,
                    };
                    let dragging = if let Some(pin) = self.pins.get_mut(&id) {
                        pin.press(owner, event.position);
                        matches!(pin.interaction, Interaction::Dragging { .. })
                    } else {
                        false
                    };
                    if dragging {
                        self.try_lock_pointer(id, pointer, &event.surface, qh);
                    }
                }
                PointerEventKind::Release { button, .. } => {
                    let owner = InputOwner::Pointer {
                        seat: seat_id.clone(),
                        button,
                    };
                    let action = self.pins.get_mut(&id).map_or(ReleaseAction::None, |pin| {
                        pin.release(&owner, event.position)
                    });
                    self.apply_release(id, action);
                }
                PointerEventKind::Axis { vertical, .. } if !vertical.is_none() => {
                    let steps = if vertical.value120 != 0 {
                        -f64::from(vertical.value120) / 120.0
                    } else if vertical.discrete != 0 {
                        -f64::from(vertical.discrete)
                    } else {
                        -vertical.absolute / 15.0
                    };
                    merge_wheel(&mut pending_wheel, id, event.position, steps);
                }
                _ => {}
            }
        }
        if let Some((id, candidate)) = pending_drag {
            self.apply_drag_origin(id, candidate, qh);
        }
        if let Some((id, position, steps)) = pending_wheel {
            self.apply_wheel(id, position, steps, qh);
        }
    }

    fn update_pointer_cursor(
        &self,
        pointer: &wl_pointer::WlPointer,
        id: PinId,
        position: (f64, f64),
    ) {
        let Some(serial) = self.cursor_serials.get(&pointer.id()).copied() else {
            return;
        };
        let Some(device) = self.cursor_shape_devices.get(&pointer.id()) else {
            return;
        };
        let Some(pin) = self.pins.get(&id) else {
            return;
        };
        let shape = match cursor_at(pin.model.frame, position) {
            PinCursor::Default => Shape::Default,
            PinCursor::Move => Shape::Move,
            PinCursor::Pointer => Shape::Pointer,
        };
        device.set_shape(serial, shape);
    }

    pub(super) fn handle_relative_motion(
        &mut self,
        pointer_id: ObjectId,
        delta: (f64, f64),
        qh: &QueueHandle<Self>,
    ) {
        let Some(id) = self.locked_pin else {
            return;
        };
        let Some((_, seat)) = self.pointers.get(&pointer_id) else {
            return;
        };
        let owner = self.pointer_drag_owner(seat.id());
        let candidate = owner.as_ref().and_then(|owner| {
            self.pins
                .get_mut(&id)
                .and_then(|pin| pin.relative_drag_origin(owner, delta))
        });
        if let Some(candidate) = candidate {
            self.apply_drag_origin(id, candidate, qh);
        }
    }

    fn pointer_drag_owner(&self, seat: ObjectId) -> Option<InputOwner> {
        self.pins.values().find_map(|pin| match &pin.interaction {
            Interaction::Dragging {
                owner:
                    owner @ InputOwner::Pointer {
                        seat: owner_seat, ..
                    },
                ..
            } if *owner_seat == seat => Some(owner.clone()),
            _ => None,
        })
    }

    fn try_lock_pointer(
        &mut self,
        id: PinId,
        pointer: &wl_pointer::WlPointer,
        surface: &wayland_client::protocol::wl_surface::WlSurface,
        qh: &QueueHandle<Self>,
    ) {
        let relative = match self
            .relative_pointer_state
            .get_relative_pointer(pointer, qh)
        {
            Ok(relative) => relative,
            Err(_) => return,
        };
        match self.pointer_constraints.lock_pointer(
            surface,
            pointer,
            None,
            Lifetime::Persistent,
            qh,
        ) {
            Ok(locked) => {
                self.relative_pointer = Some(relative);
                self.locked_pointer = Some(locked);
                self.locked_pin = Some(id);
            }
            Err(_) => relative.destroy(),
        }
    }

    pub(super) fn apply_drag_origin(
        &mut self,
        id: PinId,
        candidate: (f64, f64),
        qh: &QueueHandle<Self>,
    ) {
        let Some(pin) = self.pins.get(&id) else {
            return;
        };
        let next = geometry::dragged_frame(pin.model.frame, candidate, pin.model.output_size);
        if next != pin.model.frame
            && let Err(error) = self.commit_frame(id, next, false, qh)
        {
            log::warn!("Could not move pin {id}: {error:#}");
        }
    }

    fn apply_wheel(&mut self, id: PinId, pointer: (f64, f64), steps: f64, qh: &QueueHandle<Self>) {
        let Some(pin) = self.pins.get(&id) else {
            return;
        };
        let pointer = content_position(pointer);
        let next = geometry::resized_frame(
            pin.model.frame,
            pointer,
            steps,
            (pin.model.image.width, pin.model.image.height),
            pin.model.output_size,
            pin.shell.scale.max(1) as u32,
        );
        let Ok(next) = next else {
            return;
        };
        let Ok(next) = super::output::fit_frame_to_surface_limit(
            next,
            (pin.model.image.width, pin.model.image.height),
            pin.model.output_size,
            pin.shell.scale.max(1) as u32,
        ) else {
            return;
        };
        if next != pin.model.frame
            && let Err(error) = self.commit_frame(id, next, true, qh)
        {
            log::debug!("Pin {id} resize refused: {error:#}");
        }
    }

    pub(super) fn apply_release(&mut self, id: PinId, action: ReleaseAction) {
        match action {
            ReleaseAction::Copy => self.begin_copy(id),
            ReleaseAction::Close => self.close_pin(id),
            ReleaseAction::DragEnd => self.unlock_pointer_for(id),
            ReleaseAction::None => {}
        }
    }

    pub(super) fn handle_touch_down(
        &mut self,
        touch: &wayland_client::protocol::wl_touch::WlTouch,
        surface: &wayland_client::protocol::wl_surface::WlSurface,
        touch_id: i32,
        position: (f64, f64),
    ) {
        let Some((_, seat)) = self.touches.get(&touch.id()) else {
            return;
        };
        let seat = seat.clone();
        let Some(pin_id) = self.by_wl_surface.get(&surface.id()).copied() else {
            return;
        };
        self.arbitrate_new_owner(pin_id, &seat.id());
        let owner = InputOwner::Touch {
            seat: seat.id(),
            id: touch_id,
        };
        if let Some(pin) = self.pins.get_mut(&pin_id) {
            pin.press(owner, position);
            self.active_touches.insert(
                (touch.id(), touch_id),
                super::ActiveTouch { pin_id, position },
            );
        }
    }

    pub(super) fn handle_touch_motion(
        &mut self,
        touch: &wayland_client::protocol::wl_touch::WlTouch,
        touch_id: i32,
        position: (f64, f64),
        qh: &QueueHandle<Self>,
    ) {
        let key = (touch.id(), touch_id);
        let Some(active) = self.active_touches.get_mut(&key) else {
            return;
        };
        active.position = position;
        let pin_id = active.pin_id;
        let Some((_, seat)) = self.touches.get(&touch.id()) else {
            return;
        };
        let owner = InputOwner::Touch {
            seat: seat.id(),
            id: touch_id,
        };
        let candidate = self
            .pins
            .get_mut(&pin_id)
            .and_then(|pin| pin.fallback_drag_origin(&owner, position, pin.shell.committed_origin));
        if let Some(candidate) = candidate {
            self.apply_drag_origin(pin_id, candidate, qh);
        }
    }

    pub(super) fn handle_touch_up(
        &mut self,
        touch: &wayland_client::protocol::wl_touch::WlTouch,
        touch_id: i32,
    ) {
        let Some(active) = self.active_touches.remove(&(touch.id(), touch_id)) else {
            return;
        };
        let Some((_, seat)) = self.touches.get(&touch.id()) else {
            return;
        };
        let owner = InputOwner::Touch {
            seat: seat.id(),
            id: touch_id,
        };
        let action = self
            .pins
            .get_mut(&active.pin_id)
            .map_or(ReleaseAction::None, |pin| {
                pin.release(&owner, active.position)
            });
        self.apply_release(active.pin_id, action);
    }

    pub(super) fn cancel_touch(&mut self, touch: &wayland_client::protocol::wl_touch::WlTouch) {
        let ids: Vec<_> = self
            .active_touches
            .iter()
            .filter_map(|((owner, id), active)| {
                (owner == &touch.id()).then_some((*id, active.pin_id))
            })
            .collect();
        for (id, pin_id) in ids {
            self.active_touches.remove(&(touch.id(), id));
            if let Some(pin) = self.pins.get_mut(&pin_id) {
                pin.cancel_interaction();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_mapping_distinguishes_chrome_image_and_controls() {
        let frame = crate::pin::PinFrame::new(0, 0, 200, 120).unwrap();
        assert_eq!(cursor_at(frame, (2.0, 2.0)), PinCursor::Default);
        assert_eq!(cursor_at(frame, (28.0, 68.0)), PinCursor::Move);
        assert_eq!(cursor_at(frame, (188.0, 20.0)), PinCursor::Pointer);
    }

    #[test]
    fn wheel_samples_in_one_pointer_frame_coalesce_to_one_resize() {
        let id = PinId::new(1).unwrap();
        let mut pending = None;
        merge_wheel(&mut pending, id, (10.0, 20.0), 0.25);
        merge_wheel(&mut pending, id, (11.0, 21.0), 0.75);
        assert_eq!(pending, Some((id, (11.0, 21.0), 1.0)));
    }

    #[test]
    fn release_routes_to_the_implicit_grab_owner_over_another_pin() {
        assert_eq!(pointer_event_pin(Some(2_u64), Some(1), true), Some(1));
        assert_eq!(pointer_event_pin(Some(2_u64), Some(1), false), Some(2));
    }
}
