mod capture;
mod portal;
mod state;
mod view;

pub use state::ZoomState;
#[allow(unused_imports)]
pub(in crate::backend::wayland) use state::{
    ZoomCaptureId, ZoomSourceOutcome, ZoomSourceTerminal, ZoomTerminalReport, ZoomWaiter,
    ZoomWaiterOwner, ZoomWaiterRegistry,
};

const MIN_ZOOM_SCALE: f64 = 1.0;
const MAX_ZOOM_SCALE: f64 = 8.0;

type PortalCaptureResult = Result<
    (
        Option<u32>,
        u64,
        crate::backend::wayland::frozen::ScreenImageProvenance,
        crate::backend::wayland::frozen::FrozenImage,
    ),
    crate::capture::types::CaptureError,
>;
