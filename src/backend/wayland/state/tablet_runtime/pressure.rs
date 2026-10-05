use super::TabletState;
use crate::draw::TextMeasurer;
use crate::input::{InputState, tablet::try_apply_pressure_to_state_with};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum StylusDownAdmission {
    Canvas,
    Onboarding,
    Toast,
    Popover,
}

impl TabletState {
    /// Resolve opaque chrome before pressure can mutate the tool or its first sample.
    pub(in crate::backend::wayland) fn prepare_chrome_down(
        &mut self,
        input: &mut InputState,
        card: &mut super::super::onboarding::OnboardingCardChrome,
        position: (f64, f64),
        card_visible: bool,
    ) -> StylusDownAdmission {
        self.toast_press = None;

        if card.accept_stylus_press(card_visible, position) {
            return StylusDownAdmission::Onboarding;
        }

        // Toasts paint above the canvas and below the card, as for the mouse.
        if let Some(press) =
            input.toast_press_at(position.0.round() as i32, position.1.round() as i32)
        {
            self.toast_press = Some(press);
            return StylusDownAdmission::Toast;
        }

        if input.close_top_toolbar_menus() {
            return StylusDownAdmission::Popover;
        }

        StylusDownAdmission::Canvas
    }

    /// Apply pressure only after the press router admitted an actual drawing.
    /// Hover and consumed popup contacts never mutate the drawing tool.
    pub(in crate::backend::wayland) fn apply_canvas_pressure(
        &mut self,
        measurer: &TextMeasurer,
        input: &mut InputState,
        pressure: u32,
    ) {
        if !self.on_overlay
            || self.on_toolbar
            || self.contact_retired
            || self.on_inline_strip()
            || !self.is_canvas_gesture()
            || input.screen_modal_is_active()
            || input.command_palette_is_engaged()
            || !matches!(input.state, crate::input::DrawingState::Drawing { .. })
            || pressure == 0
        {
            return;
        }

        let first_sample = self.pressure_thickness.is_none();
        if !try_apply_pressure_to_state_with(
            measurer,
            pressure as f64 / 65535.0,
            input,
            self.settings,
        ) {
            return;
        }
        let thickness = input.style.current_thickness;
        if first_sample {
            input.replace_active_drawing_pressure_samples_with(measurer, thickness);
        }
        self.pressure_thickness = Some(thickness);
        self.peak_thickness = Some(
            self.peak_thickness
                .map_or(thickness, |peak| peak.max(thickness)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::state::contact_owner::ContactOwner;
    use crate::input::{
        MouseButton, state::test_support::make_test_input_state, tablet::TabletSettings,
    };

    #[test]
    fn a_hard_strip_tap_cannot_change_tool_thickness_or_the_next_light_canvas_stroke() {
        let measurer = TextMeasurer::default();
        let mut input = make_test_input_state();
        let initial = input.style.current_thickness;
        let mut tablet = TabletState::new(
            None,
            TabletSettings {
                enabled: true,
                pressure_enabled: true,
                min_thickness: 1.0,
                max_thickness: 20.0,
            },
        );
        tablet.on_overlay = true;

        tablet.pending_frame.pressure = Some(65535);
        tablet.bind_tip(ContactOwner::InlineToolbar);
        assert_eq!(tablet.pending_frame.pressure, None);
        tablet.apply_canvas_pressure(&measurer, &mut input, 65535);
        tablet.pending_frame.pressure = Some(65535);
        tablet.take_up_route(true);
        assert_eq!(tablet.pending_frame.pressure, None);
        tablet.apply_canvas_pressure(&measurer, &mut input, 65535);
        assert_eq!(input.style.current_thickness, initial);
        assert_eq!(tablet.pressure_thickness, None);
        assert_eq!(tablet.peak_thickness, None);

        // Pressure arrives before Down, then the caller takes the entire frame.
        input.on_mouse_press(MouseButton::Left, 600, 400);
        tablet.pending_frame.pressure = Some(4096);
        tablet.pending_frame.down = true;
        let pending = std::mem::take(&mut tablet.pending_frame);
        tablet.bind_tip(ContactOwner::Canvas);
        tablet.apply_canvas_pressure(&measurer, &mut input, pending.pressure.unwrap());
        assert!(tablet.peak_thickness.unwrap() < 3.0);
        assert_eq!(tablet.peak_thickness, tablet.pressure_thickness);
        assert_eq!(
            input.style.current_thickness,
            tablet.peak_thickness.unwrap()
        );
    }
}
