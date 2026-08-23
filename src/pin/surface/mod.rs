//! One persistent pin's model, shell role, bounded buffers, and interaction state.

mod buffers;
mod hit;
mod lifecycle;
mod render;
mod state;

pub(crate) use buffers::PinBuffers;
pub(crate) use hit::{
    CHROME_PADDING, Control, content_position, control_at, control_strip, surface_origin,
    surface_size,
};
pub(crate) use lifecycle::ShellEventIdentity;
pub(crate) use render::{Damage, RasterCache, build_static_raster, render_frame};
pub(crate) use state::{
    CopyVisual, InputOwner, Interaction, PinnedSurface, ReleaseAction, VisualState,
};
