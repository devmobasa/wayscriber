use super::contact_owner::{ContactMotion, ContactOwner, held_motion};
use contacts::PointerContacts;
use log::warn;
use smithay_client_toolkit::seat::pointer::{CursorIcon, PointerData, ThemedPointer};
use wayland_client::{
    Connection, Proxy,
    protocol::{wl_pointer, wl_seat, wl_surface, wl_touch},
};
use wayland_protocols::wp::{
    pointer_constraints::zv1::client::zwp_locked_pointer_v1::ZwpLockedPointerV1,
    relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1,
};

use super::seat_devices::SeatDevices;
use crate::{
    input::state::{RegionInputSource, ToastPress},
    ui::{OnboardingCardPress, ZoomChipPress},
};

mod chrome;
mod contacts;
mod touch;
use chrome::PendingChromePress;
use touch::TouchState;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::backend::wayland) enum TouchTarget {
    #[default]
    None,
    Canvas,
    Toolbar,
    InlineToolbar,
    Foreign,
}

pub(in crate::backend::wayland) struct TouchEnd {
    pub(in crate::backend::wayland) surface: wl_surface::WlSurface,
    pub(in crate::backend::wayland) position: (f64, f64),
    pub(in crate::backend::wayland) target: TouchTarget,
}

#[derive(Debug, Clone, Copy, Default)]
struct BoardPanGesture {
    panning: bool,
    last_pos: (f64, f64),
    key_held: bool,
}

impl BoardPanGesture {
    fn start(&mut self, position: (f64, f64)) {
        self.panning = true;
        self.last_pos = position;
    }

    fn stop(&mut self) {
        self.panning = false;
    }

    fn advance(&mut self, position: (f64, f64)) -> (f64, f64) {
        let previous = self.last_pos;
        self.last_pos = position;
        (position.0 - previous.0, position.1 - previous.1)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ReleaseSuppression {
    pointer: bool,
    touch: bool,
}

impl ReleaseSuppression {
    fn slot_mut(&mut self, source: RegionInputSource) -> Option<&mut bool> {
        match source {
            RegionInputSource::Pointer => Some(&mut self.pointer),
            RegionInputSource::Touch => Some(&mut self.touch),
            RegionInputSource::Stylus => None,
        }
    }

    fn arm(&mut self, source: RegionInputSource) {
        if let Some(slot) = self.slot_mut(source) {
            *slot = true;
        }
    }

    fn clear(&mut self, source: RegionInputSource) {
        if let Some(slot) = self.slot_mut(source) {
            *slot = false;
        }
    }

    fn take(&mut self, source: RegionInputSource) -> bool {
        self.slot_mut(source).is_some_and(std::mem::take)
    }
}

/// Pointer, cursor, pointer-lock, and single-contact touch protocol runtime.
pub(in crate::backend::wayland) struct PointerRuntime {
    themed_pointer: Option<ThemedPointer<PointerData>>,
    /// Each seat's `wl_touch`, kept while the seat advertises touch.
    touches: SeatDevices<wl_seat::WlSeat, wl_touch::WlTouch>,
    active_touch: TouchState,
    active_touch_surface: Option<wl_surface::WlSurface>,
    locked_pointer: Option<ZwpLockedPointerV1>,
    current_pointer_shape: Option<CursorIcon>,
    relative_pointer: Option<ZwpRelativePointerV1>,
    cursor_hidden: bool,
    position: (i32, i32),
    board_pan: BoardPanGesture,
    contacts: PointerContacts,
    chrome_press: PendingChromePress,
    release_suppression: ReleaseSuppression,
}

impl PointerRuntime {
    pub(super) fn new() -> Self {
        Self {
            themed_pointer: None,
            touches: SeatDevices::default(),
            active_touch: TouchState::default(),
            active_touch_surface: None,
            locked_pointer: None,
            current_pointer_shape: None,
            relative_pointer: None,
            cursor_hidden: false,
            position: (0, 0),
            board_pan: BoardPanGesture::default(),
            contacts: PointerContacts::default(),
            chrome_press: PendingChromePress::default(),
            release_suppression: ReleaseSuppression::default(),
        }
    }

    pub(in crate::backend::wayland) fn bind_contact(&mut self, button: u32, owner: ContactOwner) {
        self.contacts.bind(button, owner);
    }

    pub(in crate::backend::wayland) fn route_canvas_press(
        &mut self,
        input: &mut crate::input::InputState,
        resources: crate::input::state::InputTextResources<'_>,
        button: u32,
        mouse_button: crate::input::MouseButton,
        [sx, sy, wx, wy]: [i32; 4],
    ) {
        let outcome =
            input.on_mouse_press_with_canvas_and_resources(resources, mouse_button, sx, sy, wx, wy);
        self.contacts.bind_canvas(button, outcome, input);
    }

    pub(in crate::backend::wayland) fn take_contact(
        &mut self,
        button: u32,
        input: &crate::input::InputState,
    ) -> Option<(ContactOwner, bool)> {
        self.contacts.take(button, input)
    }

    pub(in crate::backend::wayland) fn reconcile_contacts(
        &mut self,
        input: &crate::input::InputState,
        zoom_pan: bool,
        toolbar_drag: bool,
    ) {
        self.contacts
            .reconcile(input, zoom_pan, self.board_pan.panning, toolbar_drag);
    }

    pub(in crate::backend::wayland) fn contact_motion(&self) -> ContactMotion {
        held_motion(self.contacts.motion_owner())
    }

    pub(in crate::backend::wayland) fn attach_pointer(
        &mut self,
        pointer: ThemedPointer<PointerData>,
    ) {
        self.themed_pointer = Some(pointer);
        self.reset_cursor_cache();
    }

    pub(in crate::backend::wayland) fn detach_pointer(&mut self) {
        self.themed_pointer = None;
        self.contacts.clear();
        self.reset_cursor_cache();
    }

    /// Keep `seat`'s touch device, returning one it replaces for release.
    pub(in crate::backend::wayland) fn attach_touch(
        &mut self,
        seat: wl_seat::WlSeat,
        touch: wl_touch::WlTouch,
    ) -> Option<wl_touch::WlTouch> {
        self.touches.attach(seat, touch)
    }

    /// Stop keeping `seat`'s touch device and hand it back for release.
    pub(in crate::backend::wayland) fn detach_touch(
        &mut self,
        seat: &wl_seat::WlSeat,
    ) -> Option<wl_touch::WlTouch> {
        self.touches.detach(seat)
    }

    pub(in crate::backend::wayland) fn current_pointer(&self) -> Option<wl_pointer::WlPointer> {
        self.themed_pointer
            .as_ref()
            .map(|pointer| pointer.pointer().clone())
    }

    pub(in crate::backend::wayland) fn apply_cursor_icon(
        &mut self,
        conn: &Connection,
        icon: CursorIcon,
    ) -> bool {
        self.show_cursor();
        if self.current_pointer_shape == Some(icon) {
            return false;
        }
        let Some(pointer) = self.themed_pointer.as_ref() else {
            return false;
        };
        if let Err(err) = pointer.set_cursor(conn, icon) {
            warn!("Failed to set cursor icon: {err}");
            return false;
        }
        self.current_pointer_shape = Some(icon);
        true
    }

    pub(in crate::backend::wayland) fn hide_cursor(&mut self) -> bool {
        if self.cursor_hidden {
            return false;
        }
        let Some(pointer) = self.current_pointer() else {
            return false;
        };
        let serial = pointer.data::<PointerData>().and_then(|data| {
            data.latest_button_serial()
                .or_else(|| data.latest_enter_serial())
        });
        let Some(serial) = serial else {
            return false;
        };
        pointer.set_cursor(serial, None, 0, 0);
        self.mark_cursor_hidden()
    }

    pub(in crate::backend::wayland) fn show_cursor(&mut self) -> bool {
        if !self.cursor_hidden {
            return false;
        }
        self.reset_cursor_cache();
        true
    }

    pub(in crate::backend::wayland) fn reset_cursor_on_enter(&mut self) {
        self.reset_cursor_cache();
    }

    pub(in crate::backend::wayland) fn is_locked(&self) -> bool {
        self.locked_pointer.is_some()
    }

    pub(in crate::backend::wayland) fn lock_state(&self) -> (bool, bool) {
        (
            self.locked_pointer.is_some(),
            self.relative_pointer.is_some(),
        )
    }

    pub(in crate::backend::wayland) fn lock(
        &mut self,
        locked: ZwpLockedPointerV1,
        relative: Option<ZwpRelativePointerV1>,
    ) {
        self.locked_pointer = Some(locked);
        if let Some(relative) = relative {
            self.relative_pointer = Some(relative);
        }
    }

    pub(in crate::backend::wayland) fn attach_relative_pointer(
        &mut self,
        relative: ZwpRelativePointerV1,
    ) {
        self.relative_pointer = Some(relative);
    }

    pub(in crate::backend::wayland) fn unlock(&mut self) -> bool {
        let held = self.locked_pointer.is_some() || self.relative_pointer.is_some();
        if let Some(pointer) = self.locked_pointer.take() {
            pointer.destroy();
        }
        if let Some(pointer) = self.relative_pointer.take() {
            pointer.destroy();
        }
        held
    }

    pub(in crate::backend::wayland) fn begin_touch(
        &mut self,
        id: i32,
        position: (f64, f64),
        surface: wl_surface::WlSurface,
        target: TouchTarget,
    ) -> bool {
        if !self.active_touch.begin(id, position, target) {
            return false;
        }
        self.active_touch_surface = Some(surface);
        true
    }

    pub(in crate::backend::wayland) fn set_touch_target(&mut self, target: TouchTarget) {
        self.active_touch.set_target(target);
    }

    pub(in crate::backend::wayland) fn touch_position(
        &mut self,
        id: i32,
        position: (f64, f64),
    ) -> Option<(wl_surface::WlSurface, TouchTarget)> {
        if !self.active_touch.update_position(id, position) {
            return None;
        }
        Some((self.active_touch_surface.clone()?, self.active_touch.target))
    }

    pub(in crate::backend::wayland) fn end_touch(&mut self, id: i32) -> Option<TouchEnd> {
        let (position, target) = self.active_touch.end(id)?;
        let surface = self.active_touch_surface.take()?;
        Some(TouchEnd {
            surface,
            position,
            target,
        })
    }

    pub(in crate::backend::wayland) fn cancel_touch(&mut self) -> Option<TouchEnd> {
        let contact = self.active_touch.cancel();
        let surface = self.active_touch_surface.take();
        contact
            .zip(surface)
            .map(|((position, target), surface)| TouchEnd {
                surface,
                position,
                target,
            })
    }

    pub(in crate::backend::wayland) fn position(&self) -> (i32, i32) {
        self.position
    }

    pub(in crate::backend::wayland) fn set_position(&mut self, position: (i32, i32)) {
        self.position = position;
    }

    pub(in crate::backend::wayland) fn start_board_pan(&mut self, position: (f64, f64)) {
        self.board_pan.start(position);
    }

    pub(in crate::backend::wayland) fn stop_board_pan(&mut self) {
        self.board_pan.stop();
    }

    pub(in crate::backend::wayland) fn board_pan_active(&self) -> bool {
        self.board_pan.panning
    }

    pub(in crate::backend::wayland) fn board_pan_key_held(&self) -> bool {
        self.board_pan.key_held
    }

    pub(in crate::backend::wayland) fn set_board_pan_key_held(&mut self, held: bool) {
        self.board_pan.key_held = held;
    }

    pub(in crate::backend::wayland) fn advance_board_pan(
        &mut self,
        position: (f64, f64),
    ) -> (f64, f64) {
        self.board_pan.advance(position)
    }

    pub(in crate::backend::wayland) fn clear_chrome_press(&mut self) {
        // Another device can still owe a swallowed release when chrome targets
        // reset for a new press or release cleanup.
        self.chrome_press.clear();
    }

    pub(in crate::backend::wayland) fn arm_toast_press(&mut self, press: ToastPress) -> bool {
        self.chrome_press.arm_toast(press)
    }

    pub(in crate::backend::wayland) fn take_toast_press(&mut self) -> Option<ToastPress> {
        self.chrome_press.take_toast()
    }

    pub(in crate::backend::wayland) fn arm_status_hud_press(&mut self) -> bool {
        self.chrome_press.arm_status_hud()
    }

    pub(in crate::backend::wayland) fn take_status_hud_press(&mut self) -> bool {
        self.chrome_press.take_status_hud()
    }

    pub(in crate::backend::wayland) fn arm_zoom_chip_press(
        &mut self,
        press: ZoomChipPress,
    ) -> bool {
        self.chrome_press.arm_zoom_chip(press)
    }

    pub(in crate::backend::wayland) fn take_zoom_chip_press(&mut self) -> ZoomChipPress {
        self.chrome_press.take_zoom_chip()
    }

    pub(in crate::backend::wayland) fn arm_onboarding_card_press(
        &mut self,
        press: OnboardingCardPress,
    ) -> bool {
        self.chrome_press.arm_onboarding_card(press)
    }

    pub(in crate::backend::wayland) fn take_onboarding_card_press(
        &mut self,
    ) -> Option<OnboardingCardPress> {
        self.chrome_press.take_onboarding_card()
    }

    pub(in crate::backend::wayland) fn suppress_release(&mut self, source: RegionInputSource) {
        self.release_suppression.arm(source);
    }

    pub(in crate::backend::wayland) fn clear_suppressed_release(
        &mut self,
        source: RegionInputSource,
    ) {
        self.release_suppression.clear(source);
    }

    pub(in crate::backend::wayland) fn take_suppressed_release(
        &mut self,
        source: RegionInputSource,
    ) -> bool {
        self.release_suppression.take(source)
    }

    fn reset_cursor_cache(&mut self) {
        self.current_pointer_shape = None;
        self.cursor_hidden = false;
    }

    fn mark_cursor_hidden(&mut self) -> bool {
        if self.cursor_hidden {
            return false;
        }
        self.cursor_hidden = true;
        self.current_pointer_shape = None;
        true
    }
}

#[cfg(test)]
mod tests;
