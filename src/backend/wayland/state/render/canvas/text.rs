mod ime;

use super::super::super::*;
use crate::draw::Shape;
use ime::{build_text_preview, paint_preedit_selection};
use std::ops::Range;

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

            // Use vertical bar cursor when editing existing text, underscore
            // for new text.
            let cursor_glyph = if is_editing_existing { "|" } else { "_" };

            // In-progress IME composition is transient. A collapsed cursor is
            // represented by the editor caret, a range by a highlight, and
            // -1/-1 hides it exactly as text-input-v3 requests.
            let (preview_text, preedit_selection) =
                build_text_preview(buffer, self.input_state.ime_preedit(), cursor_glyph);
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
                    self.render_preedit_selection(
                        ctx,
                        *x,
                        *y,
                        &preview_text,
                        preedit_selection.as_ref(),
                    );
                    // Underline only the composition text, not an injected
                    // caret glyph used for a collapsed preedit cursor.
                    self.render_preedit_underline(
                        ctx,
                        *x,
                        *y,
                        buffer,
                        self.input_state
                            .ime_preedit()
                            .map(|preedit| preedit.text.as_str())
                            .unwrap_or(""),
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
                    self.render_preedit_selection(
                        ctx,
                        *x,
                        *y,
                        &preview_text,
                        preedit_selection.as_ref(),
                    );
                }
            }
        }
    }

    /// Underline the IME preedit span for plain text so composing text reads
    /// as distinct from committed text. Single-line only: skipped when the
    /// buffer/preedit wraps or contains newlines, where a straight underline
    /// would land on the wrong line (the preedit still renders as text).
    fn render_preedit_underline(
        &self,
        ctx: &cairo::Context,
        x: i32,
        y: i32,
        buffer: &str,
        composed: &str,
    ) {
        if composed.is_empty()
            || self.input_state.text_wrap_width.is_some()
            || buffer.contains('\n')
            || composed.contains('\n')
        {
            return;
        }
        let size = self.input_state.current_font_size;
        let font_desc = self.input_state.font_descriptor.to_pango_string(size);
        // Right edge of a rendered prefix (ink offset + width), matching how
        // render_text lays glyphs left-to-right from the baseline origin.
        let right_edge = |text: &str| -> f64 {
            if text.is_empty() {
                return 0.0;
            }
            crate::draw::shape::measure_text_with_context(ctx, text, &font_desc, size, None)
                .map(|m| m.ink_x + m.ink_width)
                .unwrap_or(0.0)
        };
        let start_x = x as f64 + right_edge(buffer);
        let end_x = x as f64 + right_edge(&format!("{buffer}{composed}"));
        if end_x <= start_x {
            return;
        }
        // The text baseline sits at `y`; drop the underline just below it.
        let underline_y = y as f64 + size * 0.12;
        let color = self.input_state.current_color;
        ctx.save().ok();
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        ctx.set_line_width((size * 0.05).max(1.0));
        ctx.move_to(start_x, underline_y);
        ctx.line_to(end_x, underline_y);
        let _ = ctx.stroke();
        ctx.restore().ok();
    }

    /// Overlay a Pango-backed highlight for a non-collapsed preedit cursor
    /// range. Rendering the same layout preserves byte indices, wrapping, and
    /// line placement for both plain text and sticky-note previews.
    fn render_preedit_selection(
        &self,
        ctx: &cairo::Context,
        x: i32,
        y: i32,
        preview_text: &str,
        selection: Option<&Range<usize>>,
    ) {
        let Some(selection) = selection else {
            return;
        };
        if selection.is_empty() || selection.end > preview_text.len() {
            return;
        }
        let (Ok(start), Ok(end)) = (u32::try_from(selection.start), u32::try_from(selection.end))
        else {
            return;
        };

        let size = self.input_state.current_font_size;
        paint_preedit_selection(
            ctx,
            x,
            y,
            preview_text,
            start..end,
            &self.input_state.font_descriptor.to_pango_string(size),
            self.input_state.text_wrap_width,
        );
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
            } if !text.is_empty() => {
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
            Shape::StickyNote {
                x,
                y,
                text,
                background,
                size,
                font_descriptor,
                wrap_width,
            } if !text.is_empty() => {
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
