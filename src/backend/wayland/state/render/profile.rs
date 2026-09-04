use crate::render_profiles::RenderColorProfile;
use crate::util::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProfileMode {
    Off,
    Canvas,
    Ui,
    CanvasAndUi,
}

/// The selected profile is a frame value; reusable baseline storage stays in RenderRuntime.
#[derive(Debug)]
pub(super) struct FrameProfile {
    profile: Option<RenderColorProfile>,
    mode: ProfileMode,
}

/// A flushed ARGB8888 target and its physical damage regions.
pub(super) struct PixelBuffer<'a> {
    pub data: &'a mut [u8],
    pub width: i32,
    pub height: i32,
    pub stride: i32,
    pub damage: &'a [Rect],
}

impl FrameProfile {
    pub(super) fn new(
        profile: Option<RenderColorProfile>,
        remap_canvas: bool,
        remap_ui: bool,
    ) -> Self {
        let mode = if profile.is_none() {
            ProfileMode::Off
        } else {
            match (remap_canvas, remap_ui) {
                (false, false) => ProfileMode::Off,
                (true, false) => ProfileMode::Canvas,
                (false, true) => ProfileMode::Ui,
                (true, true) => ProfileMode::CanvasAndUi,
            }
        };
        Self { profile, mode }
    }

    pub(super) fn mode(&self) -> ProfileMode {
        self.mode
    }

    pub(super) fn needs_before_ui(&self, render_ui: bool) -> bool {
        self.mode == ProfileMode::Canvas || (self.mode == ProfileMode::Ui && render_ui)
    }

    /// Returns whether Cairo must be notified of rewritten canvas pixels.
    pub(super) fn before_ui(
        &self,
        pixels: PixelBuffer<'_>,
        baseline: &mut Vec<u8>,
        render_ui: bool,
    ) -> bool {
        let Some(profile) = self.profile.as_ref() else {
            return false;
        };
        match self.mode {
            ProfileMode::Canvas => {
                profile.remap_argb8888_regions(
                    pixels.data,
                    pixels.width,
                    pixels.height,
                    pixels.stride,
                    pixels.damage,
                );
                true
            }
            ProfileMode::Ui if render_ui => {
                baseline.resize(pixels.data.len(), 0);
                baseline.copy_from_slice(pixels.data);
                false
            }
            _ => false,
        }
    }

    /// Called after UI painting with the same visibility decision used by `before_ui`.
    pub(super) fn after_ui(&self, pixels: PixelBuffer<'_>, baseline: &[u8], render_ui: bool) {
        let Some(profile) = self.profile.as_ref() else {
            return;
        };
        match self.mode {
            ProfileMode::CanvasAndUi => profile.remap_argb8888_regions(
                pixels.data,
                pixels.width,
                pixels.height,
                pixels.stride,
                pixels.damage,
            ),
            ProfileMode::Ui if render_ui => profile.remap_argb8888_regions_changed_from(
                pixels.data,
                baseline,
                pixels.width,
                pixels.height,
                pixels.stride,
                pixels.damage,
            ),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
