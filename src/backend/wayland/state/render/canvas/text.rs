use super::super::super::*;
use crate::draw::Shape;

impl WaylandState {
    pub(super) fn render_text_input_preview(&self, ctx: &cairo::Context) {
        if let DrawingState::TextInput { x, y, buffer } = &self.input_state.state {
            // If editing an existing text shape, render the original as a ghost preview
            if let Some((_, snapshot)) = &self.input_state.text_edit_target {
                self.render_text_edit_ghost(ctx, &snapshot.shape);
            }

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

    /// Renders the original text as a semi-transparent ghost during editing.
    fn render_text_edit_ghost(&self, ctx: &cairo::Context, original_shape: &Shape) {
        let _ = ctx.save();
        // Apply transparency to show it as a ghost
        ctx.push_group();

        match original_shape {
            Shape::Text {
                x,
                y,
                text,
                color,
                size,
                font_descriptor,
                background_enabled,
                wrap_width,
            } => {
                if !text.is_empty() {
                    crate::draw::render_text(
                        ctx,
                        *x,
                        *y,
                        text,
                        *color,
                        *size,
                        font_descriptor,
                        *background_enabled,
                        *wrap_width,
                    );
                }
            }
            Shape::StickyNote {
                x,
                y,
                text,
                background,
                size,
                font_descriptor,
                wrap_width,
            } => {
                if !text.is_empty() {
                    crate::draw::render_sticky_note(
                        ctx,
                        *x,
                        *y,
                        text,
                        *background,
                        *size,
                        font_descriptor,
                        *wrap_width,
                    );
                }
            }
            _ => {}
        }

        let _ = ctx.pop_group_to_source();
        // Render the ghost with reduced opacity
        let _ = ctx.paint_with_alpha(0.25);
        let _ = ctx.restore();
    }
}
