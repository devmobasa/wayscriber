use smithay_client_toolkit::{
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, LayerSurface},
    },
    shm::slot::SlotPool,
};
use wayland_client::{Proxy, protocol::wl_surface};

use crate::backend::wayland::toolbar::hit::HitRegion;

#[derive(Debug)]
pub struct ToolbarSurface {
    pub name: &'static str,
    pub anchor: Anchor,
    pub margin: (i32, i32, i32, i32), // top, right, bottom, left
    pub logical_size: (u32, u32),
    pub(super) wl_surface: Option<wl_surface::WlSurface>,
    pub(crate) layer_surface: Option<LayerSurface>,
    pub(super) pool: Option<SlotPool>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) scale: i32,
    pub(crate) configured: bool,
    pub(super) dirty: bool,
    pub(super) suppressed: bool,
    pub(super) hit_regions: Vec<HitRegion>,
    pub(super) hover: Option<(f64, f64)>,
    pub(super) focus_index: Option<usize>,
}

impl ToolbarSurface {
    pub fn new(name: &'static str, anchor: Anchor, margin: (i32, i32, i32, i32)) -> Self {
        Self {
            name,
            anchor,
            margin,
            logical_size: (0, 0),
            wl_surface: None,
            layer_surface: None,
            pool: None,
            width: 0,
            height: 0,
            scale: 1,
            configured: false,
            dirty: false,
            suppressed: false,
            hit_regions: Vec::new(),
            hover: None,
            focus_index: None,
        }
    }

    pub fn is_layer(&self, layer: &LayerSurface) -> bool {
        self.layer_surface
            .as_ref()
            .map(|ls| ls.wl_surface().id() == layer.wl_surface().id())
            .unwrap_or(false)
    }

    pub fn is_surface(&self, surface: &wl_surface::WlSurface) -> bool {
        self.wl_surface
            .as_ref()
            .map(|s| s.id() == surface.id())
            .unwrap_or(false)
    }
}
