use crate::backend::wayland::frozen::FrozenImage;
use crate::draw::Color;
use crate::input::state::EyedropperCaptureSource;
use crate::input::state::{Toast, ToastPriority};

use super::WaylandState;
use super::screen_image::{
    DisplayedScreenImage, ScreenSourceEntry, displayed_screen_image, image_point_for_screen_point,
    screen_source_entry,
};

fn sample_at(image: &FrozenImage, image_x: f64, image_y: f64) -> Option<Color> {
    if image.width == 0 || image.height == 0 || image.stride <= 0 {
        return None;
    }
    let x = image_x.floor().clamp(0.0, f64::from(image.width - 1)) as usize;
    let y = image_y.floor().clamp(0.0, f64::from(image.height - 1)) as usize;
    let offset = y
        .checked_mul(image.stride as usize)?
        .checked_add(x.checked_mul(4)?)?;
    let pixel = image.data.get(offset..offset.checked_add(4)?)?;

    // Cairo ARgb32 is native-endian premultiplied BGRA on supported little-endian targets.
    let alpha = f64::from(pixel[3]) / 255.0;
    let unpremultiply = |value: u8| {
        if alpha > 0.0 {
            (f64::from(value) / 255.0 / alpha).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    Some(Color {
        r: unpremultiply(pixel[2]),
        g: unpremultiply(pixel[1]),
        b: unpremultiply(pixel[0]),
        a: 1.0,
    })
}

impl WaylandState {
    pub(in crate::backend::wayland) fn handle_pending_eyedropper_toggle(&mut self) {
        if !self.input_state.take_pending_eyedropper_toggle() {
            return;
        }
        if self.input_state.eyedropper_state().is_active()
            || self.input_state.eyedropper_state().is_pending()
        {
            self.cancel_eyedropper();
            return;
        }

        // The two screen modals are mutually exclusive; entering one ends the
        // other, including any temporary freeze that one owned.
        self.cancel_ocr();
        self.input_state.prepare_for_screen_modal();
        self.zoom.stop_pan();
        self.stop_board_pan();
        self.set_board_pan_key_held(false);
        // Entering a different modal interaction interrupts any unfinished
        // toolbar move; it is not an accepted drop.
        self.cancel_toolbar_move_drag();
        self.unlock_pointer();
        // Same as OCR: the cancelled gesture may belong to a pen that is still
        // down, and the pending branches below do not activate, so a tip-up
        // during the wait would commit that stroke's peak pressure to the tool.
        self.retire_stylus_contact();

        let decision = screen_source_entry(
            self.background_image_source().is_some(),
            self.input_state.board_is_transparent(),
            self.zoom.is_engaged(),
            self.zoom.active,
            self.frozen_enabled(),
        );
        match decision {
            ScreenSourceEntry::Activate => self.activate_eyedropper_sampler(false),
            ScreenSourceEntry::WaitForZoom => self
                .input_state
                .set_eyedropper_pending_capture(EyedropperCaptureSource::Zoom),
            ScreenSourceEntry::AutoFreeze => {
                self.input_state
                    .set_eyedropper_pending_capture(EyedropperCaptureSource::Frozen);
                self.input_state.request_frozen_toggle();
            }
            ScreenSourceEntry::RefuseWhileZoomedOnSolidBoard => {
                self.input_state.push_toast(
                    ToastPriority::Action,
                    "eyedropper",
                    Toast::info("Screen eyedropper isn't available while zoomed on a solid board.")
                        .action(
                            "Switch to transparent",
                            crate::config::Action::ReturnToTransparent,
                        ),
                );
            }
            ScreenSourceEntry::RefuseSolidBoard => {
                self.input_state.push_toast(ToastPriority::Action, "eyedropper", Toast::info("Screen eyedropper requires a transparent board or an active screen freeze.").action("Switch to transparent", crate::config::Action::ReturnToTransparent));
            }
            ScreenSourceEntry::CaptureUnavailable => {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "eyedropper",
                    Toast::warning(
                        "Screen eyedropper is unavailable because screen capture is not available.",
                    ),
                );
            }
            ScreenSourceEntry::ZoomImageUnavailable => {
                self.input_state.push_toast(ToastPriority::Info, "eyedropper", Toast::warning("Screen eyedropper is unavailable because zoom has no captured screen image."));
            }
        }
    }

    fn background_image_source(&self) -> Option<DisplayedScreenImage<'_>> {
        displayed_screen_image(
            &self.zoom,
            &self.frozen,
            self.input_state.board_is_transparent(),
        )
    }

    pub(in crate::backend::wayland) fn finish_pending_eyedropper_capture(
        &mut self,
        capture_source: EyedropperCaptureSource,
    ) {
        if self.input_state.eyedropper_state().pending_source() != Some(capture_source) {
            return;
        }
        if self.background_image_source().is_some() {
            self.activate_eyedropper_sampler(matches!(
                capture_source,
                EyedropperCaptureSource::Frozen
            ));
        } else {
            self.cancel_eyedropper();
            self.input_state
                .report_eyedropper_capture_failure_if_unreported();
        }
    }

    /// Arm the screen sampler. Retires an in-flight stylus contact for the same
    /// reason OCR does: from here the modal swallows the tip-up that would
    /// otherwise end it.
    fn activate_eyedropper_sampler(&mut self, auto_froze: bool) {
        self.retire_stylus_contact();
        self.input_state.activate_eyedropper(auto_froze);
    }

    pub(in crate::backend::wayland) fn update_eyedropper_hover(&mut self, x: f64, y: f64) {
        if self.input_state.eyedropper_is_active() {
            self.input_state.update_eyedropper_hover((x, y));
        }
    }

    pub(in crate::backend::wayland) fn cancel_eyedropper_if_source_missing(&mut self) {
        if self.input_state.eyedropper_is_active() && self.background_image_source().is_none() {
            self.cancel_eyedropper();
        }
    }

    pub(in crate::backend::wayland) fn sample_eyedropper(&mut self, x: f64, y: f64) -> bool {
        if !self.input_state.eyedropper_is_active() {
            return false;
        }
        let Some(source) = self.background_image_source() else {
            self.cancel_eyedropper();
            return true;
        };
        let (image_x, image_y) = self.eyedropper_image_coords(&source, x, y);
        let color = sample_at(source.image, image_x, image_y);
        if let Some(color) = color {
            self.input_state.apply_color_from_ui(color);
        } else {
            self.input_state.push_toast(
                ToastPriority::Critical,
                "eyedropper",
                Toast::error("Could not sample that screen pixel."),
            );
        }
        self.cancel_eyedropper();
        true
    }

    fn eyedropper_image_coords(
        &self,
        source: &DisplayedScreenImage<'_>,
        x: f64,
        y: f64,
    ) -> (f64, f64) {
        image_point_for_screen_point(
            source,
            &self.zoom,
            (self.surface.width(), self.surface.height()),
            (x, y),
        )
    }

    pub(in crate::backend::wayland) fn render_eyedropper_loupe(
        &self,
        ctx: &cairo::Context,
        screen_width: u32,
        screen_height: u32,
    ) {
        const PIXELS: i32 = 11;
        const CELL: f64 = 8.0;
        const GAP: f64 = 18.0;
        const LABEL_HEIGHT: f64 = 24.0;

        let Some((pointer_x, pointer_y)) = self.input_state.eyedropper_state().hover() else {
            return;
        };
        let Some(source) = self.background_image_source() else {
            return;
        };
        let (image_x, image_y) = self.eyedropper_image_coords(&source, pointer_x, pointer_y);
        let center_x = image_x.floor();
        let center_y = image_y.floor();
        let grid_size = f64::from(PIXELS) * CELL;
        let panel_w = grid_size + 12.0;
        let panel_h = grid_size + LABEL_HEIGHT + 12.0;
        let mut panel_x = pointer_x + GAP;
        let mut panel_y = pointer_y + GAP;
        if panel_x + panel_w > f64::from(screen_width) {
            panel_x = pointer_x - GAP - panel_w;
        }
        if panel_y + panel_h > f64::from(screen_height) {
            panel_y = pointer_y - GAP - panel_h;
        }
        panel_x = panel_x.clamp(4.0, (f64::from(screen_width) - panel_w - 4.0).max(4.0));
        panel_y = panel_y.clamp(4.0, (f64::from(screen_height) - panel_h - 4.0).max(4.0));

        let _ = ctx.save();
        ctx.set_source_rgba(0.04, 0.05, 0.07, 0.96);
        ctx.rectangle(panel_x, panel_y, panel_w, panel_h);
        let _ = ctx.fill();

        let grid_x = panel_x + 6.0;
        let grid_y = panel_y + 6.0;
        let half = PIXELS / 2;
        for row in 0..PIXELS {
            for col in 0..PIXELS {
                if let Some(color) = sample_at(
                    source.image,
                    center_x + f64::from(col - half),
                    center_y + f64::from(row - half),
                ) {
                    ctx.set_source_rgb(color.r, color.g, color.b);
                    ctx.rectangle(
                        grid_x + f64::from(col) * CELL,
                        grid_y + f64::from(row) * CELL,
                        CELL,
                        CELL,
                    );
                    let _ = ctx.fill();
                }
            }
        }

        let center = f64::from(half) * CELL;
        ctx.set_source_rgb(1.0, 1.0, 1.0);
        ctx.set_line_width(2.0);
        ctx.rectangle(grid_x + center, grid_y + center, CELL, CELL);
        let _ = ctx.stroke();
        ctx.set_source_rgb(0.0, 0.0, 0.0);
        ctx.set_line_width(1.0);
        ctx.rectangle(
            grid_x + center + 2.0,
            grid_y + center + 2.0,
            CELL - 4.0,
            CELL - 4.0,
        );
        let _ = ctx.stroke();

        if let Some(color) = sample_at(source.image, center_x, center_y) {
            let hex = format!(
                "#{:02X}{:02X}{:02X}",
                (color.r * 255.0).round() as u8,
                (color.g * 255.0).round() as u8,
                (color.b * 255.0).round() as u8
            );
            let swatch = 14.0;
            let label_y = grid_y + grid_size + 5.0;
            ctx.set_source_rgb(color.r, color.g, color.b);
            ctx.rectangle(grid_x, label_y, swatch, swatch);
            let _ = ctx.fill();
            ctx.set_source_rgb(1.0, 1.0, 1.0);
            ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            ctx.set_font_size(12.0);
            ctx.move_to(grid_x + swatch + 7.0, label_y + 12.0);
            let _ = ctx.show_text(&hex);
        }

        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.65);
        ctx.set_line_width(1.0);
        ctx.rectangle(panel_x + 0.5, panel_y + 0.5, panel_w - 1.0, panel_h - 1.0);
        let _ = ctx.stroke();
        let _ = ctx.restore();
    }

    pub(in crate::backend::wayland) fn cancel_eyedropper(&mut self) -> bool {
        let eyedropper_state = self.input_state.eyedropper_state();
        let was_active = eyedropper_state.is_engaged();
        let pending_source = eyedropper_state.pending_source();
        let auto_froze = self.input_state.cancel_eyedropper();
        if auto_froze {
            self.restore_xdg_after_frozen();
            if pending_source == Some(EyedropperCaptureSource::Frozen)
                && self.frozen.is_in_progress()
            {
                self.frozen.cancel(&mut self.input_state);
                self.exit_overlay_suppression(super::OverlaySuppression::Frozen);
            } else {
                self.frozen.unfreeze(&mut self.input_state);
            }
        }
        was_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(pixel: [u8; 4]) -> FrozenImage {
        FrozenImage {
            width: 1,
            height: 1,
            stride: 4,
            data: pixel.to_vec(),
        }
    }

    #[test]
    fn sample_unpremultiplies_rgb_but_returns_opaque_color() {
        let color = sample_at(&image([25, 50, 100, 128]), 0.0, 0.0).unwrap();
        assert!((color.r - 0.78125).abs() < 0.01);
        assert!((color.g - 0.390625).abs() < 0.01);
        assert!((color.b - 0.1953125).abs() < 0.01);
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn transparent_pixel_is_safe_opaque_black() {
        assert_eq!(
            sample_at(&image([30, 20, 10, 0]), 0.0, 0.0),
            Some(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
        );
    }

    #[test]
    fn sample_clamps_to_image_edges_and_honors_stride() {
        let image = FrozenImage {
            width: 1,
            height: 2,
            stride: 8,
            data: vec![0, 0, 255, 255, 9, 9, 9, 9, 255, 0, 0, 255, 9, 9, 9, 9],
        };
        assert_eq!(sample_at(&image, 20.0, 20.0).unwrap().b, 1.0);
    }

    #[test]
    fn entry_waits_for_pending_zoom_instead_of_starting_frozen_capture() {
        assert_eq!(
            screen_source_entry(false, true, true, false, true),
            ScreenSourceEntry::WaitForZoom
        );
    }

    #[test]
    fn entry_distinguishes_solid_board_zoom_refusal() {
        assert_eq!(
            screen_source_entry(false, false, true, true, true),
            ScreenSourceEntry::RefuseWhileZoomedOnSolidBoard
        );
        assert_eq!(
            screen_source_entry(false, false, false, false, true),
            ScreenSourceEntry::RefuseSolidBoard
        );
    }

    #[test]
    fn entry_uses_existing_source_before_board_policy() {
        assert_eq!(
            screen_source_entry(true, false, false, false, true),
            ScreenSourceEntry::Activate
        );
    }

    #[test]
    fn entry_reports_missing_zoom_image_separately_from_missing_capture_support() {
        assert_eq!(
            screen_source_entry(false, true, true, true, true),
            ScreenSourceEntry::ZoomImageUnavailable
        );
        assert_eq!(
            screen_source_entry(false, true, false, false, false),
            ScreenSourceEntry::CaptureUnavailable
        );
    }
}
