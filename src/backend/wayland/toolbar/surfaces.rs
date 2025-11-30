use anyhow::Result;
use smithay_client_toolkit::{
    compositor::CompositorState,
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurfaceConfigure},
    },
    shm::{Shm, slot::SlotPool},
};
use wayland_client::{
    Proxy, QueueHandle,
    protocol::{wl_output, wl_surface},
};

use crate::backend::wayland::state::WaylandState;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::ui::toolbar::ToolbarSnapshot;

#[derive(Debug)]
pub struct ToolbarSurface {
    pub name: &'static str,
    pub anchor: Anchor,
    pub margin: (i32, i32, i32, i32), // top, right, bottom, left
    pub logical_size: (u32, u32),
    pub wl_surface: Option<wl_surface::WlSurface>,
    pub layer_surface: Option<smithay_client_toolkit::shell::wlr_layer::LayerSurface>,
    pub pool: Option<SlotPool>,
    pub width: u32,
    pub height: u32,
    pub scale: i32,
    pub configured: bool,
    pub dirty: bool,
    pub hit_regions: Vec<HitRegion>,
    pub hover: Option<(f64, f64)>,
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
            hit_regions: Vec::new(),
            hover: None,
        }
    }

    pub fn is_layer(&self, layer: &smithay_client_toolkit::shell::wlr_layer::LayerSurface) -> bool {
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

    pub fn ensure_created(
        &mut self,
        qh: &QueueHandle<WaylandState>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        scale: i32,
    ) {
        if self.layer_surface.is_some() {
            return;
        }

        let wl_surface = compositor.create_surface(qh);
        wl_surface.set_buffer_scale(scale);

        let layer_surface = layer_shell.create_layer_surface(
            qh,
            wl_surface.clone(),
            Layer::Overlay, // map in overlay layer so toolbars can stack above main surface
            Some(self.name),
            None,
        );
        layer_surface.set_anchor(self.anchor);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_margin(self.margin.0, self.margin.1, self.margin.2, self.margin.3);

        if self.logical_size != (0, 0) {
            layer_surface.set_size(self.logical_size.0, self.logical_size.1);
        }

        layer_surface.commit();

        self.wl_surface = Some(wl_surface);
        self.layer_surface = Some(layer_surface);
        self.scale = scale.max(1);
        self.dirty = true;
        self.configured = false;
    }

    pub fn destroy(&mut self) {
        self.layer_surface = None;
        self.wl_surface = None;
        self.pool = None;
        self.width = 0;
        self.height = 0;
        self.configured = false;
        self.dirty = false;
        self.hit_regions.clear();
        self.hover = None;
    }

    pub fn handle_configure(&mut self, configure: &LayerSurfaceConfigure) -> bool {
        if self.layer_surface.is_none() {
            return false;
        }

        if configure.new_size.0 > 0 && configure.new_size.1 > 0 {
            let changed = self.width != configure.new_size.0 || self.height != configure.new_size.1;
            self.width = configure.new_size.0;
            self.height = configure.new_size.1;
            if changed {
                self.pool = None;
            }
        }

        self.configured = true;
        self.dirty = true;
        true
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn set_logical_size(&mut self, size: (u32, u32)) {
        self.logical_size = size;
    }

    pub fn set_scale(&mut self, scale: i32) {
        let scale = scale.max(1);
        if self.scale != scale {
            self.scale = scale;
            self.pool = None;
            if let Some(layer) = self.layer_surface.as_mut() {
                let _ = layer.set_buffer_scale(scale as u32);
            } else if let Some(surface) = self.wl_surface.as_ref() {
                surface.set_buffer_scale(scale);
            }
            self.dirty = true;
        }
    }

    pub fn maybe_update_scale(&mut self, output: Option<&wl_output::WlOutput>, scale: i32) {
        if output.is_some() {
            self.set_scale(scale);
        }
    }

    /// Render helper used by the manager; keeps render impl closer to surface.
    pub fn render<F>(
        &mut self,
        shm: &Shm,
        snapshot: &ToolbarSnapshot,
        hover: Option<(f64, f64)>,
        render_fn: F,
    ) -> Result<()>
    where
        F: FnOnce(
            &cairo::Context,
            f64,
            f64,
            &ToolbarSnapshot,
            &mut Vec<HitRegion>,
            Option<(f64, f64)>,
        ) -> Result<()>,
    {
        if !self.configured || !self.dirty || self.width == 0 || self.height == 0 {
            return Ok(());
        }

        let (phys_w, phys_h) = (
            self.width.saturating_mul(self.scale as u32),
            self.height.saturating_mul(self.scale as u32),
        );

        if self.pool.is_none() {
            let buffer_size = (phys_w * phys_h * 4) as usize;
            if let Ok(pool) = SlotPool::new(buffer_size, shm) {
                self.pool = Some(pool);
            } else {
                return Ok(());
            }
        }

        let pool = match self.pool.as_mut() {
            Some(p) => p,
            None => return Ok(()),
        };
        let (buffer, canvas) = match pool.create_buffer(
            phys_w as i32,
            phys_h as i32,
            (phys_w * 4) as i32,
            wayland_client::protocol::wl_shm::Format::Argb8888,
        ) {
            Ok(buf) => buf,
            Err(_) => return Ok(()),
        };

        let surface = match unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                canvas.as_mut_ptr(),
                cairo::Format::ARgb32,
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
            )
        } {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        let ctx = match cairo::Context::new(&surface) {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };

        ctx.set_operator(cairo::Operator::Clear);
        let _ = ctx.paint();
        ctx.set_operator(cairo::Operator::Over);

        if self.scale > 1 {
            ctx.scale(self.scale as f64, self.scale as f64);
        }

        self.hit_regions.clear();
        render_fn(
            &ctx,
            self.width as f64,
            self.height as f64,
            snapshot,
            &mut self.hit_regions,
            hover,
        )?;

        surface.flush();

        if let Some(layer) = self.layer_surface.as_ref() {
            let wl_surface = layer.wl_surface();
            wl_surface.set_buffer_scale(self.scale);
            wl_surface.attach(Some(buffer.wl_buffer()), 0, 0);
            wl_surface.damage_buffer(0, 0, phys_w as i32, phys_h as i32);
            wl_surface.commit();
        }

        self.dirty = false;
        Ok(())
    }

    pub fn hit_at(
        &self,
        x: f64,
        y: f64,
    ) -> Option<(crate::backend::wayland::toolbar_intent::ToolbarIntent, bool)> {
        for hit in &self.hit_regions {
            if hit.contains(x, y) {
                let start_drag = matches!(
                    hit.kind,
                    crate::backend::wayland::toolbar::events::HitKind::DragSetThickness { .. }
                        | crate::backend::wayland::toolbar::events::HitKind::DragSetFontSize
                        | crate::backend::wayland::toolbar::events::HitKind::PickColor { .. }
                        | crate::backend::wayland::toolbar::events::HitKind::DragUndoDelay
                        | crate::backend::wayland::toolbar::events::HitKind::DragRedoDelay
                        | crate::backend::wayland::toolbar::events::HitKind::DragCustomUndoDelay
                        | crate::backend::wayland::toolbar::events::HitKind::DragCustomRedoDelay
                );
                use crate::backend::wayland::toolbar::events::HitKind::*;
                use crate::backend::wayland::toolbar_intent::ToolbarIntent;
                use crate::ui::toolbar::ToolbarEvent;
                let event = match hit.kind {
                    DragSetThickness { min, max } => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        let value = min + t * (max - min);
                        ToolbarEvent::SetThickness(value)
                    }
                    DragSetFontSize => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        let value = 8.0 + t * (72.0 - 8.0);
                        ToolbarEvent::SetFontSize(value)
                    }
                    PickColor { x: px, y: py, w, h } => {
                        let hue = ((x - px) / w).clamp(0.0, 1.0);
                        let value = (1.0 - (y - py) / h).clamp(0.0, 1.0);
                        ToolbarEvent::SetColor(
                            crate::backend::wayland::toolbar::events::hsv_to_rgb(hue, 1.0, value),
                        )
                    }
                    DragUndoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        ToolbarEvent::SetUndoDelay(
                            crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
                        )
                    }
                    DragRedoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        ToolbarEvent::SetRedoDelay(
                            crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
                        )
                    }
                    DragCustomUndoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        ToolbarEvent::SetCustomUndoDelay(
                            crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
                        )
                    }
                    DragCustomRedoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        ToolbarEvent::SetCustomRedoDelay(
                            crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
                        )
                    }
                    crate::backend::wayland::toolbar::events::HitKind::Click => hit.event.clone(),
                };
                return Some((ToolbarIntent(event), start_drag));
            }
        }
        None
    }

    pub fn drag_at(
        &self,
        x: f64,
        y: f64,
    ) -> Option<crate::backend::wayland::toolbar_intent::ToolbarIntent> {
        use crate::backend::wayland::toolbar::events::HitKind::*;
        use crate::backend::wayland::toolbar_intent::ToolbarIntent;
        use crate::ui::toolbar::ToolbarEvent;
        for hit in &self.hit_regions {
            if hit.contains(x, y) {
                match hit.kind {
                    DragSetThickness { min, max } => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        let value = min + t * (max - min);
                        return Some(ToolbarIntent(ToolbarEvent::SetThickness(value)));
                    }
                    DragSetFontSize => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        let value = 8.0 + t * (72.0 - 8.0);
                        return Some(ToolbarIntent(ToolbarEvent::SetFontSize(value)));
                    }
                    PickColor { x: px, y: py, w, h } => {
                        let hue = ((x - px) / w).clamp(0.0, 1.0);
                        let value = (1.0 - (y - py) / h).clamp(0.0, 1.0);
                        return Some(ToolbarIntent(ToolbarEvent::SetColor(
                            crate::backend::wayland::toolbar::events::hsv_to_rgb(hue, 1.0, value),
                        )));
                    }
                    DragUndoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        return Some(ToolbarIntent(ToolbarEvent::SetUndoDelay(
                            crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
                        )));
                    }
                    DragRedoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        return Some(ToolbarIntent(ToolbarEvent::SetRedoDelay(
                            crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
                        )));
                    }
                    DragCustomUndoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        return Some(ToolbarIntent(ToolbarEvent::SetCustomUndoDelay(
                            crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
                        )));
                    }
                    DragCustomRedoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        return Some(ToolbarIntent(ToolbarEvent::SetCustomRedoDelay(
                            crate::backend::wayland::toolbar::events::delay_secs_from_t(t),
                        )));
                    }
                    _ => {}
                }
            }
        }
        None
    }
}
