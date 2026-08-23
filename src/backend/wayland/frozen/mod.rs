mod capture;
mod ext_image_copy;
mod image;
mod portal;
mod state;

use wayland_client::protocol::wl_output;

/// Capture-time output identity retained beside an installed screen image.
///
/// This deliberately does not read mutable live output state. A selector token
/// must describe the output that produced the pixels, even if output metadata
/// changes before the selector is armed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) struct ScreenImageProvenance {
    pub output_id: u32,
    pub output_layout_generation: u64,
    pub output_scale: i32,
    pub output_transform: wl_output::Transform,
}

impl ScreenImageProvenance {
    pub fn new(
        output_id: u32,
        output_layout_generation: u64,
        output_scale: i32,
        output_transform: wl_output::Transform,
    ) -> Option<Self> {
        (output_scale > 0).then_some(Self {
            output_id,
            output_layout_generation,
            output_scale,
            output_transform,
        })
    }
}

pub(in crate::backend::wayland) use ext_image_copy::ExtImageCopyManagers;
pub use image::FrozenImage;
pub(in crate::backend::wayland) use image::{copy_shm_argb, validate_shm_buffer_layout};
pub(in crate::backend::wayland) use state::FrozenCaptureBackend;
pub use state::FrozenState;

type PortalCaptureResult = Result<
    (
        Option<u32>,
        u64,
        Option<crate::backend::wayland::frozen_geometry::OutputGeometry>,
        self::image::FrozenImage,
    ),
    crate::capture::types::CaptureError,
>;
