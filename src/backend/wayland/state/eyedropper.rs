use crate::backend::wayland::acquisition::{ScreenAcquisitionOutcome, ScreenAcquisitionOwner};
use crate::backend::wayland::frozen::FrozenImage;
use crate::backend::wayland::zoom::ZoomWaiterOwner;
use crate::draw::Color;
use crate::input::state::EyedropperCaptureSource;
use crate::input::state::{Toast, ToastPriority};

use super::WaylandState;
use super::acquisition::report_screen_source_activation_rejected_to;
use super::screen_image::{
    DisplayedScreenImage, ScreenSourceEntry, current_screen_source_token, displayed_screen_image,
    image_point_for_screen_point, screen_source_entry, screen_source_token,
};

pub(in crate::backend::wayland) fn sample_at(
    image: &FrozenImage,
    image_x: f64,
    image_y: f64,
) -> Option<Color> {
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
    pub(in crate::backend::wayland) fn handle_eyedropper_toggle(&mut self) {
        if self.input_state.eyedropper_state().is_active()
            || self.input_state.eyedropper_state().is_pending()
        {
            self.cancel_eyedropper();
            return;
        }

        // The two screen modals are mutually exclusive; entering one ends the
        // other, including any temporary freeze that one owned.
        self.cancel_ocr();
        self.input_state
            .prepare_for_screen_modal_with_measurer(self.render.text_measurer());
        self.zoom.stop_pan();
        self.pointer.stop_board_pan();
        self.pointer.set_board_pan_key_held(false);
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
            self.frozen.enabled(),
        );
        match decision {
            ScreenSourceEntry::Activate => {
                if !self.activate_eyedropper_sampler(None) {
                    self.cancel_eyedropper_ui_only();
                    report_screen_source_activation_rejected_to(
                        &mut self.input_state,
                        ScreenAcquisitionOwner::Eyedropper,
                    );
                }
            }
            ScreenSourceEntry::WaitForZoom => {
                if self.wait_for_current_zoom_capture(ZoomWaiterOwner::Eyedropper) {
                    self.input_state
                        .set_eyedropper_pending_capture(EyedropperCaptureSource::Zoom);
                } else {
                    self.report_zoom_image_unavailable();
                }
            }
            ScreenSourceEntry::AutoFreeze => {
                match self.acquisition.request(ScreenAcquisitionOwner::Eyedropper) {
                    Ok(_) => self
                        .input_state
                        .set_eyedropper_pending_capture(EyedropperCaptureSource::Frozen),
                    Err(_) => self.report_terminal(
                        ScreenAcquisitionOwner::Eyedropper,
                        &ScreenAcquisitionOutcome::Unavailable,
                    ),
                }
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
                self.report_zoom_image_unavailable();
            }
        }
    }

    fn report_zoom_image_unavailable(&mut self) {
        self.input_state.push_toast(
            ToastPriority::Info,
            "eyedropper",
            Toast::warning(
                "Screen eyedropper is unavailable because zoom has no captured screen image.",
            ),
        );
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
        owned_frozen_generation: Option<u64>,
    ) -> bool {
        if self.input_state.eyedropper_state().pending_source() != Some(capture_source) {
            return false;
        }
        self.activate_eyedropper_sampler(owned_frozen_generation)
    }

    /// Arm the screen sampler. Retires an in-flight stylus contact for the same
    /// reason OCR does: from here the modal swallows the tip-up that would
    /// otherwise end it.
    fn activate_eyedropper_sampler(&mut self, owned_frozen_generation: Option<u64>) -> bool {
        let Some(source) = self.background_image_source() else {
            return false;
        };
        let Some(token) = current_screen_source_token(
            &source,
            &self.zoom,
            &self.frozen,
            (self.surface.width(), self.surface.height()),
        ) else {
            return false;
        };
        self.retire_stylus_contact();
        self.acquisition.set_eyedropper_source(token);
        self.input_state
            .activate_eyedropper_with(self.render.text_measurer(), owned_frozen_generation);
        true
    }

    pub(in crate::backend::wayland) fn update_eyedropper_hover(&mut self, x: f64, y: f64) {
        self.cancel_screen_modals_if_source_changed();
        if self.input_state.eyedropper_is_active() {
            self.input_state.update_eyedropper_hover((x, y));
        }
    }

    pub(in crate::backend::wayland) fn sample_eyedropper(&mut self, x: f64, y: f64) -> bool {
        if !self.input_state.eyedropper_is_active() {
            return false;
        }
        self.cancel_screen_modals_if_source_changed();
        if !self.input_state.eyedropper_is_active() {
            return true;
        }
        let Some(source) = self.background_image_source() else {
            self.cancel_eyedropper();
            return true;
        };
        let (image_x, image_y) = self.eyedropper_image_coords(&source, x, y);
        let color = sample_at(source.image, image_x, image_y);
        if let Some(color) = color {
            self.input_state
                .apply_color_from_ui_with_measurer(self.render.text_measurer(), color);
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
        let token = screen_source_token(
            source,
            &self.zoom,
            &self.frozen,
            (self.surface.width(), self.surface.height()),
        );
        let point = image_point_for_screen_point(&token, (x, y));
        (point.x, point.y)
    }

    pub(in crate::backend::wayland) fn render_eyedropper_loupe(
        &self,
        ctx: &cairo::Context,
        screen_width: u32,
        screen_height: u32,
    ) {
        let Some((pointer_x, pointer_y)) = self.input_state.eyedropper_state().hover() else {
            return;
        };
        let Some(source) = self.background_image_source() else {
            return;
        };
        let (image_x, image_y) = self.eyedropper_image_coords(&source, pointer_x, pointer_y);
        let layout = crate::ui::compute_eyedropper_loupe_layout(
            (pointer_x, pointer_y),
            (screen_width, screen_height),
        );
        crate::ui::render_eyedropper_loupe(ctx, layout, (image_x, image_y), |x, y| {
            sample_at(source.image, x, y)
        });
    }

    pub(in crate::backend::wayland) fn cancel_eyedropper(&mut self) -> bool {
        let eyedropper_state = self.input_state.eyedropper_state();
        let was_active = eyedropper_state.is_engaged();
        let pending_acquisition = (eyedropper_state.pending_source()
            == Some(EyedropperCaptureSource::Frozen))
        .then(|| self.acquisition.slot())
        .flatten()
        .filter(|record| record.owner == ScreenAcquisitionOwner::Eyedropper)
        .map(|record| record.id);
        let owned_generation = eyedropper_state.owned_frozen_generation();
        self.cancel_modal_owner_resources(
            ScreenAcquisitionOwner::Eyedropper,
            pending_acquisition,
            owned_generation,
        );
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
