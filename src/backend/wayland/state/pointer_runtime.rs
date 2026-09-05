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

use crate::{
    input::state::{RegionInputSource, ToastPress},
    ui::ZoomChipPress,
};

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

#[derive(Debug, Clone, Copy, Default)]
struct PendingChromePress {
    toast: Option<ToastPress>,
    status_hud: bool,
    zoom_chip: ZoomChipPress,
}

impl PendingChromePress {
    fn occupied(&self) -> bool {
        self.toast.is_some() || self.status_hud || self.zoom_chip.is_pending()
    }

    fn clear(&mut self) {
        self.toast = None;
        self.status_hud = false;
        self.zoom_chip = ZoomChipPress::None;
    }

    fn arm_toast(&mut self, press: ToastPress) -> bool {
        if self.occupied() {
            return false;
        }
        self.toast = Some(press);
        true
    }

    fn take_toast(&mut self) -> Option<ToastPress> {
        self.toast.take()
    }

    fn arm_status_hud(&mut self) -> bool {
        if self.occupied() {
            return false;
        }
        self.status_hud = true;
        true
    }

    fn take_status_hud(&mut self) -> bool {
        std::mem::take(&mut self.status_hud)
    }

    fn arm_zoom_chip(&mut self, press: ZoomChipPress) -> bool {
        if self.occupied() || !press.is_pending() {
            return false;
        }
        self.zoom_chip = press;
        true
    }

    fn take_zoom_chip(&mut self) -> ZoomChipPress {
        std::mem::replace(&mut self.zoom_chip, ZoomChipPress::None)
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
    position: (i32, i32),
    board_pan: BoardPanGesture,
    chrome_press: PendingChromePress,
    release_suppression: ReleaseSuppression,
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
            position: (0, 0),
            board_pan: BoardPanGesture::default(),
            chrome_press: PendingChromePress::default(),
            release_suppression: ReleaseSuppression::default(),
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
mod tests {
    use super::{PendingChromePress, PointerRuntime, TouchState, TouchTarget};
    use crate::{
        input::state::{RegionInputSource, ToastPress},
        ui::ZoomChipPress,
    };
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
        assert!(touch.begin(7, (10.0, 20.0), TouchTarget::Canvas));
        assert!(!touch.begin(8, (30.0, 40.0), TouchTarget::Toolbar));
        assert_eq!(touch.end(8), None);
        assert_eq!(touch.end(7), Some(((10.0, 20.0), TouchTarget::Canvas)));
        assert_eq!(touch.end(7), None);
    }

    #[test]
    fn active_touch_updates_only_the_owned_contact() {
        let mut touch = TouchState::default();
        assert!(touch.begin(7, (10.0, 20.0), TouchTarget::Canvas));

        assert!(!touch.update_position(8, (30.0, 40.0)));
        assert!(touch.update_position(7, (50.0, 60.0)));
        assert_eq!(touch.end(7), Some(((50.0, 60.0), TouchTarget::Canvas)));
    }

    #[test]
    fn chrome_press_priority_keeps_the_first_target() {
        let mut press = PendingChromePress::default();
        let toast = ToastPress::body(7);

        assert!(press.arm_toast(toast));
        assert!(!press.arm_status_hud());
        assert!(!press.arm_zoom_chip(ZoomChipPress::Passive));
        assert_eq!(press.take_toast(), Some(toast));
        assert_eq!(press.take_toast(), None);
    }

    #[test]
    fn clearing_chrome_press_preserves_both_release_latches() {
        let mut runtime = PointerRuntime::new();
        assert!(runtime.arm_status_hud_press());
        runtime.suppress_release(RegionInputSource::Pointer);
        runtime.suppress_release(RegionInputSource::Touch);

        runtime.clear_chrome_press();
        assert!(!runtime.take_status_hud_press());

        assert!(runtime.arm_toast_press(ToastPress::body(7)));
        runtime.clear_chrome_press();
        assert_eq!(runtime.take_toast_press(), None);

        assert!(runtime.arm_zoom_chip_press(ZoomChipPress::Passive));
        runtime.clear_chrome_press();
        assert_eq!(runtime.take_zoom_chip_press(), ZoomChipPress::None);
        assert!(runtime.take_suppressed_release(RegionInputSource::Pointer));
        assert!(runtime.take_suppressed_release(RegionInputSource::Touch));
    }

    #[test]
    fn release_suppression_is_owned_by_its_source() {
        let mut runtime = PointerRuntime::new();
        runtime.suppress_release(RegionInputSource::Pointer);
        runtime.suppress_release(RegionInputSource::Touch);
        runtime.clear_suppressed_release(RegionInputSource::Touch);

        assert!(!runtime.take_suppressed_release(RegionInputSource::Touch));
        assert!(runtime.take_suppressed_release(RegionInputSource::Pointer));
        assert!(!runtime.take_suppressed_release(RegionInputSource::Pointer));
        assert!(!runtime.take_suppressed_release(RegionInputSource::Stylus));
    }

    #[test]
    fn pointer_cleanup_preserves_a_pending_touch_release() {
        let mut runtime = PointerRuntime::new();
        runtime.suppress_release(RegionInputSource::Pointer);
        runtime.suppress_release(RegionInputSource::Touch);

        assert!(runtime.take_suppressed_release(RegionInputSource::Pointer));
        runtime.clear_chrome_press();

        assert!(!runtime.take_suppressed_release(RegionInputSource::Pointer));
        assert!(runtime.take_suppressed_release(RegionInputSource::Touch));
        assert!(!runtime.take_suppressed_release(RegionInputSource::Touch));
    }

    #[test]
    fn touch_cancellation_cleanup_preserves_a_pending_pointer_release() {
        let mut runtime = PointerRuntime::new();
        runtime.suppress_release(RegionInputSource::Pointer);
        runtime.suppress_release(RegionInputSource::Touch);

        runtime.clear_chrome_press();
        runtime.clear_suppressed_release(RegionInputSource::Touch);

        assert!(!runtime.take_suppressed_release(RegionInputSource::Touch));
        assert!(runtime.take_suppressed_release(RegionInputSource::Pointer));
    }

    #[test]
    fn held_eyedropper_pointer_release_does_not_finish_a_new_touch_stroke() {
        assert_interleaved_release_keeps_stroke(
            RegionInputSource::Pointer,
            RegionInputSource::Touch,
        );
    }

    #[test]
    fn held_region_touch_release_does_not_finish_a_new_pointer_stroke() {
        assert_interleaved_release_keeps_stroke(
            RegionInputSource::Touch,
            RegionInputSource::Pointer,
        );
    }

    fn assert_interleaved_release_keeps_stroke(
        consumed_source: RegionInputSource,
        drawing_source: RegionInputSource,
    ) {
        use crate::input::{MouseButton, state::DrawingState};

        let measurer = crate::draw::TextMeasurer::default();
        let ui_engine = crate::ui_text::UiTextEngine::default();
        let resources = crate::input::state::InputTextResources {
            measurer: &measurer,
            ui_engine: &ui_engine,
        };
        let mut runtime = PointerRuntime::new();
        let mut input = crate::input::state::test_support::make_test_input_state();

        // The modal press was consumed, but that device remains held while the
        // other device starts drawing. Both canvas press handlers reset chrome.
        runtime.suppress_release(consumed_source);
        runtime.clear_chrome_press();
        input.on_mouse_press_with_canvas_and_resources(
            resources,
            MouseButton::Left,
            10,
            20,
            10,
            20,
        );
        input.on_mouse_motion_with_canvas_and_resources(resources, 30, 40, 30, 40);
        assert!(matches!(input.state, DrawingState::Drawing { .. }));

        // Exercise the release gate shared by the pointer and touch handlers.
        // Falling through here would commit the other device's unfinished stroke.
        if runtime.take_suppressed_release(consumed_source) {
            runtime.clear_chrome_press();
        } else {
            input.on_mouse_release_with_canvas_and_resources(
                resources,
                MouseButton::Left,
                30,
                40,
                30,
                40,
            );
        }
        assert!(matches!(input.state, DrawingState::Drawing { .. }));
        assert!(input.boards.active_frame().shapes.is_empty());

        assert!(!runtime.take_suppressed_release(drawing_source));
        input.on_mouse_release_with_canvas_and_resources(
            resources,
            MouseButton::Left,
            50,
            60,
            50,
            60,
        );
        assert!(matches!(input.state, DrawingState::Idle));
        assert_eq!(input.boards.active_frame().shapes.len(), 1);
    }

    #[test]
    fn chrome_press_targets_are_taken_once() {
        let mut runtime = PointerRuntime::new();
        assert!(runtime.arm_zoom_chip_press(ZoomChipPress::Passive));

        assert_eq!(runtime.take_zoom_chip_press(), ZoomChipPress::Passive);
        assert_eq!(runtime.take_zoom_chip_press(), ZoomChipPress::None);
    }

    #[test]
    fn board_pan_advance_uses_and_updates_the_previous_sample() {
        let mut runtime = PointerRuntime::new();
        runtime.start_board_pan((10.0, 20.0));

        assert_eq!(runtime.advance_board_pan((13.5, 18.0)), (3.5, -2.0));
        assert_eq!(runtime.advance_board_pan((15.0, 22.0)), (1.5, 4.0));
    }

    #[test]
    fn pointer_position_round_trips() {
        let mut runtime = PointerRuntime::new();
        runtime.set_position((17, 23));

        assert_eq!(runtime.position(), (17, 23));
    }

    #[test]
    fn unlocking_an_empty_runtime_reports_no_transition() {
        let mut runtime = PointerRuntime::new();

        assert!(!runtime.unlock());
    }
}
