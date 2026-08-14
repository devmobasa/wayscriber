mod capture;
mod ext_image_copy;
mod image;
mod portal;
mod state;

pub(in crate::backend::wayland) use ext_image_copy::ExtImageCopyManagers;
pub use image::FrozenImage;
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
