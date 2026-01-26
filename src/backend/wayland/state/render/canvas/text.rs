use super::super::super::*;
use crate::draw::Shape;

impl WaylandState {
    pub(super) fn render_text_input_preview(&self, ctx: &cairo::Context) {
        if let DrawingState::TextInput { x, y, buffer } = &self.input_state.state {
            let is_editing_existing = self.input_state.text_edit_target.is_some();

            // If editing an existing text shape, render the original as a ghost preview
            if let Some((_, snapshot)) = &self.input_state.text_edit_target {
                self.render_text_edit_ghost(ctx, &snapshot.shape);
            }

            // Render entry animation if active
            if let Some(progress) = self.input_state.text_edit_entry_progress() {
                self.render_text_edit_entry_animation(ctx, *x, *y, progress);
            }

            // Use vertical bar cursor when editing existing text, underscore for new text
            let cursor = if is_editing_existing { "|" } else { "_" };
            let preview_text = if buffer.is_empty() {
                cursor.to_string()
            } else {
                format!("{}{}", buffer, cursor)
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
        // Render the ghost with increased opacity (was 0.25, now 0.40)
        let _ = ctx.paint_with_alpha(0.40);
        let _ = ctx.restore();

        // Render dashed border around ghost text bounds
        if let Some(bounds) = original_shape.bounding_box() {
            self.render_ghost_border(ctx, bounds);
        }
    }

    /// Renders a subtle dashed border around the ghost text area.
    fn render_ghost_border(&self, ctx: &cairo::Context, bounds: crate::util::Rect) {
        let _ = ctx.save();

        let padding = 4.0;
        let x = bounds.x as f64 - padding;
        let y = bounds.y as f64 - padding;
        let width = bounds.width as f64 + padding * 2.0;
        let height = bounds.height as f64 + padding * 2.0;

        // Teal color at 50% alpha
        ctx.set_source_rgba(0.2, 0.55, 0.65, 0.5);
        ctx.set_line_width(1.0);
        ctx.set_dash(&[4.0, 3.0], 0.0);

        ctx.rectangle(x, y, width, height);
        let _ = ctx.stroke();

        let _ = ctx.restore();
    }

    /// Renders the entry animation (teal glow pulse) when entering edit mode.
    fn render_text_edit_entry_animation(
        &self,
        ctx: &cairo::Context,
        x: i32,
        y: i32,
        progress: f64,
    ) {
        let _ = ctx.save();

        // Fade out effect: starts strong, fades to nothing
        let alpha = (1.0 - progress) * 0.6;
        let glow_radius = 30.0 + progress * 20.0; // Expands slightly as it fades

        // Create radial gradient for glow effect
        let gradient =
            cairo::RadialGradient::new(x as f64, y as f64, 0.0, x as f64, y as f64, glow_radius);

        // Teal glow color
        gradient.add_color_stop_rgba(0.0, 0.2, 0.55, 0.65, alpha);
        gradient.add_color_stop_rgba(0.5, 0.2, 0.55, 0.65, alpha * 0.5);
        gradient.add_color_stop_rgba(1.0, 0.2, 0.55, 0.65, 0.0);

        ctx.set_source(&gradient).ok();
        ctx.arc(
            x as f64,
            y as f64,
            glow_radius,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        let _ = ctx.fill();

        let _ = ctx.restore();
    }
}
