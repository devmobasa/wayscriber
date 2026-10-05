use std::collections::HashMap;

use wayland_client::{backend::ObjectId, protocol::wl_surface};
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_manager_v2::ZwpTabletManagerV2, zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2,
    zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2, zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2,
    zwp_tablet_pad_v2::ZwpTabletPadV2, zwp_tablet_seat_v2::ZwpTabletSeatV2,
    zwp_tablet_tool_v2::ZwpTabletToolV2, zwp_tablet_v2::ZwpTabletV2,
};

use super::PendingStylusFrame;

use super::contact_owner::{ContactMotion, ContactOwner, ReleaseRoute, held_motion, release_route};
use crate::backend::wayland::TabletToolType;
use crate::input::{Tool, tablet::TabletSettings};

mod pressure;
pub(in crate::backend::wayland) use pressure::StylusDownAdmission;

pub(in crate::backend::wayland) struct HoverTransition {
    pub(in crate::backend::wayland) previous: Option<(f64, f64)>,
    pub(in crate::backend::wayland) next: Option<(f64, f64)>,
}

/// Protocol objects and active-contact state for tablet-input-v2.
pub(in crate::backend::wayland) struct TabletState {
    pub(in crate::backend::wayland) manager: Option<ZwpTabletManagerV2>,
    pub(in crate::backend::wayland) seats: Vec<ZwpTabletSeatV2>,
    pub(in crate::backend::wayland) devices: Vec<ZwpTabletV2>,
    pub(in crate::backend::wayland) tools: Vec<ZwpTabletToolV2>,
    pub(in crate::backend::wayland) pads: Vec<ZwpTabletPadV2>,
    pub(in crate::backend::wayland) pad_groups: Vec<ZwpTabletPadGroupV2>,
    pub(in crate::backend::wayland) pad_rings: Vec<ZwpTabletPadRingV2>,
    pub(in crate::backend::wayland) pad_strips: Vec<ZwpTabletPadStripV2>,
    pub(in crate::backend::wayland) settings: TabletSettings,
    pub(in crate::backend::wayland) found_logged: bool,
    tip_owner: Option<ContactOwner>,
    over_inline_strip: bool,
    pub(in crate::backend::wayland) on_overlay: bool,
    pub(in crate::backend::wayland) on_toolbar: bool,
    pub(in crate::backend::wayland) pressure_thickness: Option<f64>,
    pub(in crate::backend::wayland) surface: Option<wl_surface::WlSurface>,
    pub(in crate::backend::wayland) last_pos: Option<(f64, f64)>,
    pub(in crate::backend::wayland) peak_thickness: Option<f64>,
    pub(in crate::backend::wayland) pending_frame: PendingStylusFrame,
    pub(in crate::backend::wayland) contact_retired: bool,
    /// A tip-down on a toast, resolved when the tip lifts.
    pub(in crate::backend::wayland) toast_press: Option<crate::input::state::ToastPress>,
    pub(in crate::backend::wayland) tool_types: HashMap<ObjectId, TabletToolType>,
    pub(in crate::backend::wayland) auto_switched_to_eraser: bool,
    pub(in crate::backend::wayland) pre_eraser_tool_override: Option<Tool>,
}

/// Where a stylus motion goes after screen modals and move drags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::backend::wayland) enum StylusMotionRoute {
    /// The proximity surface is the layer-shell toolbar (unchanged path).
    LayerShellToolbar,
    /// The strip owns the tip: strip motion only.
    InlineStrip,
    /// No recorded tip: strip hover first, then the canvas.
    InlineHover,
    /// Canvas motion, queued for the frame.
    Canvas,
}

impl TabletState {
    pub(in crate::backend::wayland) fn record_immediate_motion(&mut self, position: (f64, f64)) {
        self.last_pos = Some(position);
    }

    pub(in crate::backend::wayland) fn current_or_pending_position(
        &self,
        fallback: (i32, i32),
    ) -> (f64, f64) {
        self.pending_frame
            .motion
            .or(self.last_pos)
            .unwrap_or((fallback.0 as f64, fallback.1 as f64))
    }

    pub(in crate::backend::wayland) fn hover_cursor_position(&self) -> Option<(f64, f64)> {
        (self.tip_owner.is_none() && !self.pending_frame.down && self.hovers_canvas())
            .then_some(self.last_pos)
            .flatten()
    }

    /// The pen over the canvas, tip up or down. The toolbar fade's reveal zone
    /// reads it.
    pub(in crate::backend::wayland) fn canvas_point(&self) -> Option<(f64, f64)> {
        self.hovers_canvas().then_some(self.last_pos).flatten()
    }

    fn hovers_canvas(&self) -> bool {
        self.on_overlay && !self.on_toolbar && !self.over_inline_strip
    }

    /// A canvas stroke or other canvas gesture holds the tip.
    pub(in crate::backend::wayland) fn is_canvas_gesture(&self) -> bool {
        matches!(self.tip_owner, Some(ContactOwner::Canvas))
    }

    /// The pen hovers the strip or the strip holds its tip.
    pub(in crate::backend::wayland) fn on_inline_strip(&self) -> bool {
        self.over_inline_strip || self.tip_owner == Some(ContactOwner::InlineToolbar)
    }

    pub(in crate::backend::wayland) fn set_over_inline_strip(&mut self, over: bool) {
        self.over_inline_strip = over;
    }

    pub(in crate::backend::wayland) fn bind_tip(&mut self, owner: ContactOwner) {
        if owner == ContactOwner::InlineToolbar {
            self.clear_pressure();
        }
        self.tip_owner = Some(owner);
    }

    pub(in crate::backend::wayland) fn consume_contact(&mut self) {
        self.pending_frame = PendingStylusFrame::default();
        self.tip_owner = None;
        self.contact_retired = true;
        self.clear_pressure();
    }

    fn clear_pressure(&mut self) {
        self.pending_frame.pressure = None;
        self.pressure_thickness = None;
        self.peak_thickness = None;
    }

    /// The frame committed the tip-up: the tip has no owner any more.
    pub(in crate::backend::wayland) fn lift_tip(&mut self) {
        self.tip_owner = None;
    }

    /// Proximity starts or ends: forget the tip and the strip hover.
    pub(in crate::backend::wayland) fn reset_contact(&mut self) {
        self.tip_owner = None;
        self.over_inline_strip = false;
        self.toast_press = None;
    }

    pub(in crate::backend::wayland) fn motion_route(
        &self,
        inline_active: bool,
    ) -> StylusMotionRoute {
        // A queued canvas down already owns routing even before its pressure frame.
        if self.pending_frame.down {
            return StylusMotionRoute::Canvas;
        }
        if self.on_toolbar {
            return StylusMotionRoute::LayerShellToolbar;
        }

        match held_motion(self.tip_owner) {
            ContactMotion::Toolbar => StylusMotionRoute::InlineStrip,
            ContactMotion::Hover if inline_active => StylusMotionRoute::InlineHover,
            _ => StylusMotionRoute::Canvas,
        }
    }

    /// Routes a tip-up at the `up` event. A strip record ends here, before any
    /// gate can consume the `up`. A canvas record ends when the frame commits
    /// the up (`lift_tip`).
    pub(in crate::backend::wayland) fn take_up_route(
        &mut self,
        inline_active: bool,
    ) -> ReleaseRoute {
        let route = release_route(self.tip_owner, inline_active);
        if route == ReleaseRoute::InlineToolbar {
            self.tip_owner = None;
            self.clear_pressure();
        }

        route
    }

    pub(in crate::backend::wayland) fn retire_contact(&mut self) -> HoverTransition {
        let canvas_tip = self.is_canvas_gesture();
        let had_contact = canvas_tip || self.pending_frame.down;
        self.pending_frame = PendingStylusFrame::default();
        self.contact_retired |= had_contact;
        let previous_hover = self.hover_cursor_position();
        if canvas_tip {
            self.tip_owner = None;
            self.clear_pressure();
        }
        HoverTransition {
            previous: previous_hover,
            next: self.hover_cursor_position(),
        }
    }

    pub(in crate::backend::wayland) fn take_retired_contact(&mut self) -> bool {
        std::mem::take(&mut self.contact_retired)
    }

    pub(super) fn new(manager: Option<ZwpTabletManagerV2>, settings: TabletSettings) -> Self {
        Self {
            manager,
            seats: Vec::new(),
            devices: Vec::new(),
            tools: Vec::new(),
            pads: Vec::new(),
            pad_groups: Vec::new(),
            pad_rings: Vec::new(),
            pad_strips: Vec::new(),
            settings,
            found_logged: false,
            tip_owner: None,
            over_inline_strip: false,
            on_overlay: false,
            on_toolbar: false,
            pressure_thickness: None,
            surface: None,
            last_pos: None,
            peak_thickness: None,
            pending_frame: PendingStylusFrame::default(),
            contact_retired: false,
            toast_press: None,
            tool_types: HashMap::new(),
            auto_switched_to_eraser: false,
            pre_eraser_tool_override: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TabletState;
    use crate::input::tablet::TabletSettings;

    #[test]
    fn inline_hover_does_not_change_the_proximity_surface_or_steal_canvas_contact() {
        let mut state = TabletState::new(None, TabletSettings::default());
        state.on_overlay = true;
        state.last_pos = Some((10.0, 20.0));
        state.set_over_inline_strip(true);
        assert_eq!(state.hover_cursor_position(), None);
        assert!(!state.on_toolbar);
        state.set_over_inline_strip(false);
        assert_eq!(state.hover_cursor_position(), Some((10.0, 20.0)));

        state.pending_frame.down = true;
        assert_eq!(state.motion_route(true), super::StylusMotionRoute::Canvas);
        state.pending_frame.down = false;
        state.bind_tip(super::ContactOwner::Canvas);
        assert_eq!(state.motion_route(true), super::StylusMotionRoute::Canvas);
        assert_eq!(
            state.take_up_route(true),
            super::ReleaseRoute::CanvasSurface
        );
        assert!(state.is_canvas_gesture());
        state.lift_tip();
        assert!(!state.is_canvas_gesture());

        state.bind_tip(super::ContactOwner::InlineToolbar);
        assert_eq!(
            state.motion_route(false),
            super::StylusMotionRoute::InlineStrip
        );
        assert_eq!(
            state.take_up_route(false),
            super::ReleaseRoute::InlineToolbar
        );
        assert!(!state.on_inline_strip());
        state.set_over_inline_strip(true);
        state.reset_contact();
        assert!(!state.on_inline_strip());
    }

    #[test]
    fn retiring_contact_drops_buffered_input_and_consumes_one_tip_up() {
        let mut state = TabletState::new(None, TabletSettings::default());
        state.bind_tip(super::ContactOwner::Canvas);
        state.pressure_thickness = Some(7.0);
        state.peak_thickness = Some(9.0);
        state.pending_frame.down = true;
        state.pending_frame.pressure = Some(32_000);

        state.retire_contact();

        assert!(!state.is_canvas_gesture());
        assert_eq!(state.pressure_thickness, None);
        assert_eq!(state.peak_thickness, None);
        assert!(!state.pending_frame.down);
        assert_eq!(state.pending_frame.pressure, None);
        assert!(state.take_retired_contact());
        assert!(!state.take_retired_contact());
    }

    #[test]
    fn layer_shell_surface_keeps_its_motion_route_for_every_tip_owner() {
        for inline in [false, true] {
            for owner in [
                super::ContactOwner::Canvas,
                super::ContactOwner::InlineToolbar,
            ] {
                let mut state = TabletState::new(None, TabletSettings::default());
                state.on_toolbar = true;
                state.bind_tip(owner);
                assert_eq!(
                    state.motion_route(inline),
                    super::StylusMotionRoute::LayerShellToolbar
                );
                state.retire_contact();
                if matches!(owner, super::ContactOwner::Canvas) {
                    assert_eq!(state.tip_owner, None);
                    assert!(state.take_retired_contact());
                } else {
                    assert_eq!(state.tip_owner, Some(owner));
                    assert!(!state.take_retired_contact());
                    assert_eq!(
                        state.take_up_route(inline),
                        super::ReleaseRoute::InlineToolbar
                    );
                }
                assert_eq!(
                    state.motion_route(inline),
                    super::StylusMotionRoute::LayerShellToolbar
                );
            }
        }
    }

    #[test]
    fn palette_consumed_tip_drops_buffered_down_pressure_and_one_up() {
        let mut state = TabletState::new(None, TabletSettings::default());
        state.pending_frame.down = true;
        state.pending_frame.pressure = Some(65_535);
        state.consume_contact();
        assert!(!state.pending_frame.down);
        assert_eq!(state.pending_frame.pressure, None);
        assert_eq!(state.tip_owner, None);
        assert!(state.take_retired_contact());
        assert!(!state.take_retired_contact());
    }
}
