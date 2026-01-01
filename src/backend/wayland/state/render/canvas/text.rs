use super::super::super::*;

impl WaylandState {
    pub(super) fn render_text_input_preview(&self, ctx: &cairo::Context) {
        if let DrawingState::TextInput { x, y, buffer } = &self.input_state.state {
            let preview_text = if buffer.is_empty() {
                "_".to_string() // Show cursor when buffer is empty
            } else {
                format!("{}_", buffer)
            };
            match self.input_state.text_input_mode {
                crate::input::TextInputMode::Plain => {
                    crate::draw::render_text(
                        ctx,
                        *x,
                        *y,
                        &preview_text,
                        self.input_state.current_color,
                        self.input_state.current_font_size,
                        &self.input_state.font_descriptor,
                        self.input_state.text_background_enabled,
                        self.input_state.text_wrap_width,
                    );
                }
                crate::input::TextInputMode::StickyNote => {
                    crate::draw::render_sticky_note(
                        ctx,
                        *x,
                        *y,
                        &preview_text,
                        self.input_state.current_color,
                        self.input_state.current_font_size,
                        &self.input_state.font_descriptor,
                        self.input_state.text_wrap_width,
                    );
                }
            }
        }
    }
}
