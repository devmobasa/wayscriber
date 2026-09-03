use log::warn;
use smithay_client_toolkit::seat::pointer::{CursorIcon, PointerData, ThemedPointer};
use wayland_client::{
    Connection, Proxy,
    protocol::{wl_pointer, wl_surface, wl_touch},
};
use wayland_protocols::wp::{
    pointer_constraints::zv1::client::zwp_locked_pointer_v1::ZwpLockedPointerV1,
    relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::backend::wayland) enum TouchTarget {
    #[default]
    None,
    Overlay,
    Toolbar,
    InlineToolbar,
    Other,
}

pub(in crate::backend::wayland) struct TouchEnd {
    pub(in crate::backend::wayland) surface: wl_surface::WlSurface,
    pub(in crate::backend::wayland) position: (f64, f64),
    pub(in crate::backend::wayland) target: TouchTarget,
}

#[derive(Clone, Copy, Debug, Default)]
struct TouchState {
    active_id: Option<i32>,
    target: TouchTarget,
    last_position: Option<(f64, f64)>,
}

impl TouchState {
    fn begin(&mut self, id: i32, position: (f64, f64), target: TouchTarget) -> bool {
        if self.active_id.is_some() {
            return false;
        }
        self.active_id = Some(id);
        self.target = target;
        self.last_position = Some(position);
        true
    }

    fn update_position(&mut self, id: i32, position: (f64, f64)) -> bool {
        if self.active_id != Some(id) {
            return false;
        }
        self.last_position = Some(position);
        true
    }

    fn end(&mut self, id: i32) -> Option<((f64, f64), TouchTarget)> {
        if self.active_id != Some(id) {
            return None;
        }
        self.cancel()
    }

    fn cancel(&mut self) -> Option<((f64, f64), TouchTarget)> {
        let end = self.last_position.map(|position| (position, self.target));
        self.clear();
        end
    }

    fn set_target(&mut self, target: TouchTarget) {
        if self.active_id.is_some() {
            self.target = target;
        }
    }

    fn clear(&mut self) {
        self.active_id = None;
        self.target = TouchTarget::None;
        self.last_position = None;
    }
}

/// Pointer, cursor, pointer-lock, and single-contact touch protocol runtime.
pub(in crate::backend::wayland) struct PointerRuntime {
    themed_pointer: Option<ThemedPointer<PointerData>>,
    #[allow(dead_code)] // Retains the protocol object while the seat advertises touch.
    touch: Option<wl_touch::WlTouch>,
    active_touch: TouchState,
    active_touch_surface: Option<wl_surface::WlSurface>,
    locked_pointer: Option<ZwpLockedPointerV1>,
    current_pointer_shape: Option<CursorIcon>,
    relative_pointer: Option<ZwpRelativePointerV1>,
    cursor_hidden: bool,
}

impl PointerRuntime {
    pub(super) fn new() -> Self {
        Self {
            themed_pointer: None,
            touch: None,
            active_touch: TouchState::default(),
            active_touch_surface: None,
            locked_pointer: None,
            current_pointer_shape: None,
            relative_pointer: None,
            cursor_hidden: false,
        }
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
        self.reset_cursor_cache();
    }

    pub(in crate::backend::wayland) fn attach_touch(&mut self, touch: wl_touch::WlTouch) {
        self.touch = Some(touch);
    }

    pub(in crate::backend::wayland) fn detach_touch(&mut self) {
        self.touch = None;
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
mod tests {
    use super::{PointerRuntime, TouchState, TouchTarget};
    use smithay_client_toolkit::seat::pointer::CursorIcon;

    #[test]
    fn pointer_attachment_reset_clears_shape_and_hidden_state() {
        let mut runtime = PointerRuntime::new();
        runtime.current_pointer_shape = Some(CursorIcon::Crosshair);
        runtime.cursor_hidden = true;

        runtime.reset_cursor_cache();

        assert_eq!(runtime.current_pointer_shape, None);
        assert!(!runtime.cursor_hidden);
    }

    #[test]
    fn hide_and_show_transitions_are_idempotent() {
        let mut runtime = PointerRuntime::new();

        assert!(runtime.mark_cursor_hidden());
        assert!(!runtime.mark_cursor_hidden());
        assert!(runtime.show_cursor());
        assert!(!runtime.show_cursor());
    }

    #[test]
    fn active_touch_rejects_a_second_contact_and_foreign_end() {
        let mut touch = TouchState::default();
        assert!(touch.begin(7, (10.0, 20.0), TouchTarget::Overlay));
        assert!(!touch.begin(8, (30.0, 40.0), TouchTarget::Toolbar));
        assert_eq!(touch.end(8), None);
        assert_eq!(touch.end(7), Some(((10.0, 20.0), TouchTarget::Overlay)));
        assert_eq!(touch.end(7), None);
    }

    #[test]
    fn active_touch_updates_only_the_owned_contact() {
        let mut touch = TouchState::default();
        assert!(touch.begin(7, (10.0, 20.0), TouchTarget::Overlay));

        assert!(!touch.update_position(8, (30.0, 40.0)));
        assert!(touch.update_position(7, (50.0, 60.0)));
        assert_eq!(touch.end(7), Some(((50.0, 60.0), TouchTarget::Overlay)));
    }

    #[test]
    fn unlocking_an_empty_runtime_reports_no_transition() {
        let mut runtime = PointerRuntime::new();

        assert!(!runtime.unlock());
    }
}
