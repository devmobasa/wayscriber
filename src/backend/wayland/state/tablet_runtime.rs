use std::collections::HashMap;

use wayland_client::{backend::ObjectId, protocol::wl_surface};
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_manager_v2::ZwpTabletManagerV2, zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2,
    zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2, zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2,
    zwp_tablet_pad_v2::ZwpTabletPadV2, zwp_tablet_seat_v2::ZwpTabletSeatV2,
    zwp_tablet_tool_v2::ZwpTabletToolV2, zwp_tablet_v2::ZwpTabletV2,
};

use super::PendingStylusFrame;
use crate::backend::wayland::TabletToolType;
use crate::input::{Tool, tablet::TabletSettings};

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
    pub(in crate::backend::wayland) tip_down: bool,
    pub(in crate::backend::wayland) on_overlay: bool,
    pub(in crate::backend::wayland) on_toolbar: bool,
    pub(in crate::backend::wayland) base_thickness: Option<f64>,
    pub(in crate::backend::wayland) pressure_thickness: Option<f64>,
    pub(in crate::backend::wayland) surface: Option<wl_surface::WlSurface>,
    pub(in crate::backend::wayland) last_pos: Option<(f64, f64)>,
    pub(in crate::backend::wayland) peak_thickness: Option<f64>,
    pub(in crate::backend::wayland) pending_frame: PendingStylusFrame,
    pub(in crate::backend::wayland) contact_retired: bool,
    pub(in crate::backend::wayland) tool_types: HashMap<ObjectId, TabletToolType>,
    pub(in crate::backend::wayland) auto_switched_to_eraser: bool,
    pub(in crate::backend::wayland) pre_eraser_tool_override: Option<Tool>,
}

impl TabletState {
    pub(in crate::backend::wayland) fn hover_cursor_position(&self) -> Option<(f64, f64)> {
        (self.on_overlay && !self.on_toolbar && !self.tip_down)
            .then_some(self.last_pos)
            .flatten()
    }

    pub(in crate::backend::wayland) fn retire_contact(&mut self) -> HoverTransition {
        let had_contact = self.tip_down || self.pending_frame.down;
        self.pending_frame = PendingStylusFrame::default();
        self.contact_retired |= had_contact;
        let previous_hover = self.hover_cursor_position();
        if self.tip_down {
            self.tip_down = false;
            self.pressure_thickness = None;
            self.peak_thickness = None;
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
            tip_down: false,
            on_overlay: false,
            on_toolbar: false,
            base_thickness: None,
            pressure_thickness: None,
            surface: None,
            last_pos: None,
            peak_thickness: None,
            pending_frame: PendingStylusFrame::default(),
            contact_retired: false,
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
    fn retiring_contact_drops_buffered_input_and_consumes_one_tip_up() {
        let mut state = TabletState::new(None, TabletSettings::default());
        state.tip_down = true;
        state.pressure_thickness = Some(7.0);
        state.peak_thickness = Some(9.0);
        state.pending_frame.down = true;
        state.pending_frame.pressure = Some(32_000);

        state.retire_contact();

        assert!(!state.tip_down);
        assert_eq!(state.pressure_thickness, None);
        assert_eq!(state.peak_thickness, None);
        assert!(!state.pending_frame.down);
        assert_eq!(state.pending_frame.pressure, None);
        assert!(state.take_retired_contact());
        assert!(!state.take_retired_contact());
    }
}
