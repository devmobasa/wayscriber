mod ime;

use super::super::super::*;
use super::CanvasRenderCtx;
use crate::draw::Shape;
use ime::{paint_preedit_selection, paint_preedit_underline};
use std::ops::Range;

impl WaylandState {
    pub(super) fn render_text_input_preview(&self, canvas: &CanvasRenderCtx<'_>) {
        let ctx = canvas.cairo;
        if let DrawingState::TextInput { x, y, .. } = &self.input_state.state {
            let is_editing_existing = self.input_state.text_editing.edit_target().is_some();

            // The faded original ("ghost") is a "where it was" reference, useful
            // only once the block has been repositioned. While editing in place
            // (origin unchanged) it overlaps the live text and reads as undeleted
            // text, so it stays hidden until the block actually moves. The input
            // layer owns this predicate so the damage bounds and the render agree.
            if self.input_state.text_edit_ghost_visible()
                && let Some((_, snapshot)) = self.input_state.text_editing.edit_target()
            {
                self.render_text_edit_ghost(canvas, &snapshot.shape);
            }

            // Render entry animation if active
            if let Some(progress) = self.input_state.text_edit_entry_progress() {
                self.render_text_edit_entry_animation(ctx, *x, *y, progress);
            }

            // Use vertical bar cursor when editing existing text, underscore
            // for new text.
            let cursor_glyph = if is_editing_existing { "|" } else { "_" };

            // Compose the preview from the buffer, the caret, the selection, and
            // any in-progress IME composition. The caret glyph is placed at the
            // caret byte offset; a selection is drawn as a highlight (no glyph);
            // a preedit is inserted at the caret with its own cursor/underline.
            let Some(preview) = self.input_state.text_input_preview(cursor_glyph) else {
                return;
            };
            let decoration_color = text_preview_decoration_color(
                self.input_state.text_editing.mode(),
                self.input_state.style.current_color,
            );
            match self.input_state.text_editing.mode() {
                crate::input::TextInputMode::Plain => {
                    crate::draw::render_text_with_halo_with_measurer(
                        self.render.text_measurer(),
                        ctx,
                        *x,
                        *y,
                        &preview.text,
                        self.input_state.style.current_color,
                        self.input_state.style.current_font_size,
                        &self.input_state.style.font_descriptor,
                        self.input_state.style.text_background_enabled,
                        self.input_state.style.text_wrap_width,
                        canvas.canvas.text_halo_enabled,
                    );
                }
                crate::input::TextInputMode::StickyNote => {
                    crate::draw::render_sticky_note_preview_with_measurer(
                        self.render.text_measurer(),
                        ctx,
                        *x,
                        *y,
                        &preview.text,
                        self.input_state.style.current_color,
                        self.input_state.style.current_font_size,
                        &self.input_state.style.font_descriptor,
                        self.input_state.style.text_wrap_width,
                    );
                }
            }
            self.render_selection_highlight(ctx, *x, *y, &preview.text, preview.highlight.as_ref());
            self.render_preedit_underline(
                ctx,
                *x,
                *y,
                &preview.text,
                preview.underline.as_ref(),
                decoration_color,
            );
            self.render_caret_line(
                canvas,
                *x,
                *y,
                &preview.text,
                preview.caret,
                decoration_color,
            );
        }
    }

    /// Draw the caret as a thin vertical line at `caret` (a byte offset into
    /// `preview_text`). Unlike injecting a glyph, this leaves the text run —
    /// and the ghost of any original text being edited — unshifted, and Pango's
    /// cursor position keeps it correct on wrapped and multiline text.
    fn render_caret_line(
        &self,
        canvas: &CanvasRenderCtx<'_>,
        x: i32,
        y: i32,
        preview_text: &str,
        caret: Option<usize>,
        color: crate::draw::Color,
    ) {
        let Some(caret) = caret else {
            return;
        };
        let size = self.input_state.style.current_font_size;
        let font_desc = self.input_state.style.font_descriptor.to_pango_string(size);
        let Some(geom) = self.render.text_measurer().caret_geometry_text(
            preview_text,
            &font_desc,
            self.input_state.style.text_wrap_width,
            caret,
        ) else {
            return;
        };
        let caret_x = x as f64 + geom.x;
        let top = y as f64 + geom.y_from_baseline;
        let bottom = top + geom.height;
        render_caret_stroke(
            canvas.cairo,
            caret_x,
            top,
            bottom,
            color,
            size,
            canvas.canvas.text_halo_enabled,
        );
    }

    /// Underline the IME preedit span (a byte range into `preview_text`) so
    /// composing text reads as distinct from committed text. Pango owns the
    /// range placement so bidi, contextual shaping, wrapping, and newlines use
    /// the same full layout as the text itself.
    fn render_preedit_underline(
        &self,
        ctx: &cairo::Context,
        x: i32,
        y: i32,
        preview_text: &str,
        range: Option<&Range<usize>>,
        color: crate::draw::Color,
    ) {
        let Some(range) = range else {
            return;
        };
        if range.start >= range.end || range.end > preview_text.len() {
            return;
        }
        let (Ok(start), Ok(end)) = (u32::try_from(range.start), u32::try_from(range.end)) else {
            return;
        };
        let size = self.input_state.style.current_font_size;
        let font_desc = self.input_state.style.font_descriptor.to_pango_string(size);
        paint_preedit_underline(
            ctx,
            (x, y),
            preview_text,
            start..end,
            &font_desc,
            self.input_state.style.text_wrap_width,
            color,
        );
    }

    /// Overlay a Pango-backed highlight for a byte range into `preview_text` —
    /// either the editor's text selection or a non-collapsed preedit cursor.
    /// Rendering the same layout preserves byte indices, wrapping, and line
    /// placement for both plain text and sticky-note previews.
    fn render_selection_highlight(
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

        let size = self.input_state.style.current_font_size;
        paint_preedit_selection(
            ctx,
            x,
            y,
            preview_text,
            start..end,
            &self.input_state.style.font_descriptor.to_pango_string(size),
            self.input_state.style.text_wrap_width,
        );
    }

    /// Renders the original text as a semi-transparent ghost during editing.
    fn render_text_edit_ghost(&self, canvas: &CanvasRenderCtx<'_>, original_shape: &Shape) {
        let ctx = canvas.cairo;
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
                crate::draw::render_text_with_halo_with_measurer(
                    self.render.text_measurer(),
                    ctx,
                    *x,
                    *y,
                    text,
                    *color,
                    *size,
                    font_descriptor,
                    *background_enabled,
                    *wrap_width,
                    canvas.canvas.text_halo_enabled,
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
                crate::draw::render_sticky_note_with_measurer(
                    self.render.text_measurer(),
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
        if let Some(bounds) = original_shape.bounding_box_with(self.render.text_measurer()) {
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

fn render_caret_stroke(
    ctx: &cairo::Context,
    caret_x: f64,
    top: f64,
    bottom: f64,
    color: crate::draw::Color,
    size: f64,
    text_halo_enabled: bool,
) {
    ctx.save().ok();
    // Widths come from the draw layer so the damage tracker sizes the caret's
    // repaint rectangle from the exact same numbers.
    let line_width = crate::draw::caret_line_width(size);
    if text_halo_enabled {
        // The caret stands where the glyphs will, so it asks the same question
        // they do: what is behind this point on the canvas?
        let background_luminance = crate::draw::painted_background_luminance(
            ctx,
            (
                caret_x,
                top,
                crate::draw::caret_outline_width(size),
                bottom - top,
            ),
        );
        let outline = crate::draw::text_outline_color(color, background_luminance);
        ctx.set_source_rgba(outline.r, outline.g, outline.b, outline.a);
        ctx.set_line_width(crate::draw::caret_outline_width(size));
        ctx.move_to(caret_x, top);
        ctx.line_to(caret_x, bottom);
        let _ = ctx.stroke();
    }
    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(line_width);
    ctx.move_to(caret_x, top);
    ctx.line_to(caret_x, bottom);
    let _ = ctx.stroke();
    ctx.restore().ok();
}

fn text_preview_decoration_color(
    mode: crate::input::TextInputMode,
    current_color: crate::draw::Color,
) -> crate::draw::Color {
    match mode {
        crate::input::TextInputMode::Plain => current_color,
        crate::input::TextInputMode::StickyNote => {
            crate::draw::sticky_note_foreground(current_color)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{render_caret_stroke, text_preview_decoration_color};
    use crate::draw::Color;
    use crate::input::TextInputMode;

    #[test]
    fn sticky_note_preedit_decorations_use_the_note_foreground() {
        let background = Color {
            r: 0.95,
            g: 0.9,
            b: 0.2,
            a: 1.0,
        };

        assert_eq!(
            text_preview_decoration_color(TextInputMode::StickyNote, background),
            crate::draw::sticky_note_foreground(background)
        );
        assert_eq!(
            text_preview_decoration_color(TextInputMode::Plain, background),
            background
        );
    }

    fn caret_pixels(text_halo_enabled: bool) -> Vec<u8> {
        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, 80, 100).expect("caret surface");
        {
            let ctx = cairo::Context::new(&surface).expect("caret context");
            ctx.set_source_rgb(1.0, 1.0, 1.0);
            ctx.paint().expect("white backdrop");
            render_caret_stroke(
                &ctx,
                40.0,
                20.0,
                80.0,
                Color::new(0.96, 0.2, 0.25, 1.0),
                36.0,
                text_halo_enabled,
            );
        }
        surface.flush();
        surface.data().expect("caret pixels").to_vec()
    }

    #[test]
    fn caret_restores_cairo_state_with_and_without_halo() {
        for text_halo_enabled in [false, true] {
            let surface =
                cairo::ImageSurface::create(cairo::Format::ARgb32, 80, 100).expect("caret surface");
            let ctx = cairo::Context::new(&surface).expect("caret context");
            ctx.translate(3.0, 5.0);
            ctx.scale(1.25, 1.5);
            ctx.set_line_width(7.0);
            ctx.set_dash(&[2.0, 3.0], 1.0);
            ctx.set_source_rgba(0.1, 0.2, 0.3, 0.4);
            let matrix = ctx.matrix();
            let source = cairo::SolidPattern::try_from(ctx.source())
                .expect("solid source")
                .rgba()
                .expect("source color");

            render_caret_stroke(
                &ctx,
                40.0,
                20.0,
                50.0,
                Color::new(0.96, 0.2, 0.25, 1.0),
                36.0,
                text_halo_enabled,
            );

            assert_eq!(ctx.matrix(), matrix);
            assert_eq!(ctx.line_width(), 7.0);
            assert_eq!(ctx.dash(), (vec![2.0, 3.0], 1.0));
            assert_eq!(
                cairo::SolidPattern::try_from(ctx.source())
                    .expect("restored solid source")
                    .rgba()
                    .expect("restored source color"),
                source,
            );
        }
    }

    #[test]
    fn text_entry_caret_honours_the_configured_halo_setting() {
        assert!(
            caret_pixels(true) != caret_pixels(false),
            "caret rendering must honor the frame text halo policy",
        );
    }
}
