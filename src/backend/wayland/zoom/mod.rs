mod capture;
mod portal;
mod state;
mod view;

pub use state::ZoomState;

const MIN_ZOOM_SCALE: f64 = 1.0;
const MAX_ZOOM_SCALE: f64 = 8.0;

type PortalCaptureResult =
    Result<(Option<u32>, crate::backend::wayland::frozen::FrozenImage), String>;
type PortalCaptureRx = std::sync::mpsc::Receiver<PortalCaptureResult>;
