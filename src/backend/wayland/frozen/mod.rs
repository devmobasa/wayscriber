mod capture;
mod image;
mod portal;
mod state;

pub use image::FrozenImage;
pub use state::FrozenState;

type PortalCaptureResult = Result<(Option<u32>, self::image::FrozenImage), String>;
type PortalCaptureRx = std::sync::mpsc::Receiver<PortalCaptureResult>;
