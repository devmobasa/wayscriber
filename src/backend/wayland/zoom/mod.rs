mod capture;
mod portal;
mod retry;
mod state;
mod view;

pub use state::ZoomState;
#[allow(unused_imports)]
pub(in crate::backend::wayland) use state::{
    ZoomCaptureBackend, ZoomCaptureId, ZoomSourceOutcome, ZoomSourceTerminal, ZoomTerminalReport,
    ZoomWaiter, ZoomWaiterOwner, ZoomWaiterRegistry,
};

use crate::backend::wayland::frozen::{FrozenImage, ScreenImageProvenance};
use crate::capture::types::CaptureError;

const MIN_ZOOM_SCALE: f64 = 1.0;
const MAX_ZOOM_SCALE: f64 = 8.0;

// Keep raster rejection beside the capture snapshot so layout validation runs
// before interpreting a size mismatch. Portal and decode failures remain terminal.
type PortalCaptureResult = Result<
    (
        Option<u32>,
        u64,
        ScreenImageProvenance,
        Result<FrozenImage, CaptureError>,
    ),
    CaptureError,
>;
