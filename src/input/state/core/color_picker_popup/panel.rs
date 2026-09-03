use super::{
    ColorPickerPopupAction, ColorPickerPopupLayout, ColorPickerPopupState, PickerDrag,
    color_to_hex, rgb_to_hsv,
};
use crate::draw::Color;
use crate::input::Tool;

/// Modal state, cached geometry, and press identity for the color picker popup.
#[derive(Debug, Default)]
pub struct ColorPickerPopupPanel {
    pub(in crate::input::state) state: ColorPickerPopupState,
    pub(in crate::input::state) layout: Option<ColorPickerPopupLayout>,
    pub(in crate::input::state) generation: u64,
    pub(in crate::input::state) pressed_action: Option<ColorPickerPopupAction>,
}

impl ColorPickerPopupPanel {
    pub fn is_open(&self) -> bool {
        matches!(self.state, ColorPickerPopupState::Open { .. })
    }

    pub(crate) fn open(&mut self, tool: Tool, slot: Option<usize>, color: Color) {
        self.generation = self.generation.wrapping_add(1);
        self.pressed_action = None;
        self.state = ColorPickerPopupState::Open {
            tool,
            slot,
            original_color: color,
            current_color: color,
            hex_editing: false,
            hex_buffer: color_to_hex(color),
            dragging: None,
            picker_hsv: rgb_to_hsv(color.r, color.g, color.b),
            hex_selected: false,
            hover_pos: None,
        };
    }

    pub(crate) fn hide(&mut self) {
        self.state = ColorPickerPopupState::Hidden;
        self.layout = None;
        self.pressed_action = None;
    }

    pub fn slot(&self) -> Option<usize> {
        match &self.state {
            ColorPickerPopupState::Open { slot, .. } => *slot,
            ColorPickerPopupState::Hidden => None,
        }
    }

    pub fn current_color(&self) -> Option<Color> {
        match &self.state {
            ColorPickerPopupState::Open { current_color, .. } => Some(*current_color),
            ColorPickerPopupState::Hidden => None,
        }
    }

    pub(crate) fn current_generation(&self) -> Option<u64> {
        self.is_open().then_some(self.generation)
    }

    pub(crate) fn note_action_press(&mut self, x: f64, y: f64) -> bool {
        self.pressed_action = self.layout.and_then(|layout| layout.action_at(x, y));
        self.pressed_action.is_some()
    }

    pub(crate) fn clear_action_press(&mut self) {
        self.pressed_action = None;
    }

    pub(crate) fn take_action_press(&mut self) -> Option<ColorPickerPopupAction> {
        self.pressed_action.take()
    }

    pub fn layout(&self) -> Option<ColorPickerPopupLayout> {
        self.layout
    }

    pub(crate) fn update_layout(
        &mut self,
        screen_width: u32,
        screen_height: u32,
        show_default_button: bool,
    ) {
        self.layout = self.is_open().then(|| {
            ColorPickerPopupLayout::compute(screen_width, screen_height, show_default_button)
        });
    }

    pub(crate) fn clear_layout(&mut self) {
        self.layout = None;
    }

    pub(crate) fn set_dragging(&mut self, dragging: Option<PickerDrag>) {
        if let ColorPickerPopupState::Open {
            dragging: current, ..
        } = &mut self.state
        {
            *current = dragging;
        }
    }

    pub fn drag_target(&self) -> Option<PickerDrag> {
        match &self.state {
            ColorPickerPopupState::Open { dragging, .. } => *dragging,
            ColorPickerPopupState::Hidden => None,
        }
    }

    pub(crate) fn take_drag_target(&mut self) -> Option<PickerDrag> {
        let target = self.drag_target();
        self.set_dragging(None);
        target
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::RED;

    #[test]
    fn reopening_advances_generation_and_resets_transient_state() {
        let mut panel = ColorPickerPopupPanel::default();
        panel.open(Tool::Pen, Some(2), RED);
        let first = panel.current_generation().expect("open generation");
        panel.set_dragging(Some(PickerDrag::Hue));
        panel.hide();
        panel.open(Tool::Marker, None, RED);

        assert!(panel.current_generation().expect("reopened generation") > first);
        assert_eq!(panel.slot(), None);
        assert_eq!(panel.drag_target(), None);
        assert_eq!(panel.current_color(), Some(RED));
    }

    #[test]
    fn taking_a_drag_target_ends_the_drag() {
        let mut panel = ColorPickerPopupPanel::default();
        panel.open(Tool::Pen, None, RED);
        panel.set_dragging(Some(PickerDrag::SatVal));

        assert_eq!(panel.take_drag_target(), Some(PickerDrag::SatVal));
        assert_eq!(panel.take_drag_target(), None);
    }
}
