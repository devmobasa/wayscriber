use anyhow::{Context, Result};
use smithay_client_toolkit::{
    compositor::CompositorState,
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurfaceConfigure},
    },
    shm::{Shm, slot::SlotPool},
};
use wayland_client::{
    QueueHandle, Proxy,
    protocol::{wl_output, wl_surface},
};

use crate::draw::{Color, FontDescriptor, BLACK, BLUE, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};
use crate::input::Tool;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

use super::state::WaylandState;

#[derive(Clone, Debug)]
struct HitRegion {
    rect: (f64, f64, f64, f64), // x, y, w, h
    event: ToolbarEvent,
    kind: HitKind,
}

impl HitRegion {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.rect.0
            && x <= self.rect.0 + self.rect.2
            && y >= self.rect.1
            && y <= self.rect.1 + self.rect.3
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum HitKind {
    Click,
    DragSetThickness,
    DragSetFontSize,
}

#[derive(Debug)]
struct ToolbarSurface {
    name: &'static str,
    anchor: Anchor,
    margin: (i32, i32, i32, i32), // top, right, bottom, left
    logical_size: (u32, u32),
    wl_surface: Option<wl_surface::WlSurface>,
    layer_surface: Option<smithay_client_toolkit::shell::wlr_layer::LayerSurface>,
    pool: Option<SlotPool>,
    width: u32,
    height: u32,
    scale: i32,
    configured: bool,
    dirty: bool,
    hit_regions: Vec<HitRegion>,
}

impl ToolbarSurface {
    fn new(name: &'static str, anchor: Anchor, margin: (i32, i32, i32, i32)) -> Self {
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
        }
    }

    fn is_layer(&self, layer: &smithay_client_toolkit::shell::wlr_layer::LayerSurface) -> bool {
        self.layer_surface
            .as_ref()
            .map(|ls| ls.wl_surface().id() == layer.wl_surface().id())
            .unwrap_or(false)
    }

    fn is_surface(&self, surface: &wl_surface::WlSurface) -> bool {
        self.wl_surface
            .as_ref()
            .map(|s| s.id() == surface.id())
            .unwrap_or(false)
    }

    fn ensure_created(
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
            Layer::Overlay,
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

    fn destroy(&mut self) {
        self.layer_surface = None;
        self.wl_surface = None;
        self.pool = None;
        self.width = 0;
        self.height = 0;
        self.configured = false;
        self.dirty = false;
        self.hit_regions.clear();
    }

    fn handle_configure(&mut self, configure: &LayerSurfaceConfigure) -> bool {
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

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn set_logical_size(&mut self, size: (u32, u32)) {
        self.logical_size = size;
    }

    fn set_scale(&mut self, scale: i32) {
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

    fn render(&mut self, shm: &Shm, snapshot: &ToolbarSnapshot, hover: Option<(f64, f64)>) -> Result<()> {
        if !self.configured || !self.dirty || self.width == 0 || self.height == 0 {
            return Ok(());
        }

        let (phys_w, phys_h) = (
            self.width.saturating_mul(self.scale as u32),
            self.height.saturating_mul(self.scale as u32),
        );

        if self.pool.is_none() {
            let buffer_size = (phys_w * phys_h * 4) as usize;
            let pool = SlotPool::new(buffer_size, shm).context("Failed to create toolbar SlotPool")?;
            self.pool = Some(pool);
        }

        let pool = self.pool.as_mut().context("Toolbar pool missing")?;
        let (buffer, canvas) = pool
            .create_buffer(
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
                wayland_client::protocol::wl_shm::Format::Argb8888,
            )
            .context("Failed to create toolbar buffer")?;

        let surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                canvas.as_mut_ptr(),
                cairo::Format::ARgb32,
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
            )
        }
        .context("Failed to create cairo surface for toolbar")?;
        let ctx = cairo::Context::new(&surface).context("Failed to create cairo context for toolbar")?;

        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint()?;
        ctx.set_operator(cairo::Operator::Over);

        if self.scale > 1 {
            ctx.scale(self.scale as f64, self.scale as f64);
        }

        self.hit_regions.clear();
        draw_panel_background(&ctx, self.width as f64, self.height as f64);

        match self.name {
            "wayscriber-toolbar-top" => {
                render_top_strip(
                    &ctx,
                    self.width as f64,
                    self.height as f64,
                    snapshot,
                    &mut self.hit_regions,
                    hover,
                );
            }
            "wayscriber-toolbar-side" => {
                render_side_palette(
                    &ctx,
                    self.width as f64,
                    self.height as f64,
                    snapshot,
                    &mut self.hit_regions,
                    hover,
                );
            }
            _ => {}
        }

        surface.flush();
        drop(ctx);
        drop(surface);

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

    fn maybe_update_scale(&mut self, output: Option<&wl_output::WlOutput>, scale: i32) {
        if output.is_some() {
            self.set_scale(scale);
        }
    }

    fn hit_at(&self, x: f64, y: f64) -> Option<(ToolbarEvent, bool)> {
        for hit in &self.hit_regions {
            if hit.contains(x, y) {
                let start_drag = matches!(hit.kind, HitKind::DragSetThickness | HitKind::DragSetFontSize);
                let event = match hit.kind {
                    HitKind::DragSetThickness => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        let value = 1.0 + t * (40.0 - 1.0);
                        ToolbarEvent::SetThickness(value)
                    }
                    HitKind::DragSetFontSize => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        let value = 8.0 + t * (72.0 - 8.0);
                        ToolbarEvent::SetFontSize(value)
                    }
                    HitKind::Click => hit.event.clone(),
                };
                return Some((event, start_drag));
            }
        }
        None
    }

    fn drag_at(&self, x: f64, y: f64) -> Option<ToolbarEvent> {
        for hit in &self.hit_regions {
            if hit.contains(x, y) {
                match hit.kind {
                    HitKind::DragSetThickness => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        let value = 1.0 + t * (40.0 - 1.0);
                        return Some(ToolbarEvent::SetThickness(value));
                    }
                    HitKind::DragSetFontSize => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        let value = 8.0 + t * (72.0 - 8.0);
                        return Some(ToolbarEvent::SetFontSize(value));
                    }
                    _ => {}
                }
            }
        }
        None
    }
}

/// Tracks the lifetime and visibility of the top + side toolbar surfaces.
#[derive(Debug)]
pub struct ToolbarSurfaceManager {
    /// Legacy combined visibility (for backwards compatibility)
    visible: bool,
    /// Whether the top toolbar is visible
    top_visible: bool,
    /// Whether the side toolbar is visible
    side_visible: bool,
    top: ToolbarSurface,
    side: ToolbarSurface,
    top_hover: Option<(f64, f64)>,
    side_hover: Option<(f64, f64)>,
}

impl Default for ToolbarSurfaceManager {
    fn default() -> Self {
        Self {
            visible: false,
            top_visible: false,
            side_visible: false,
            top: ToolbarSurface::new("wayscriber-toolbar-top", Anchor::TOP, (12, 12, 0, 12)),
            side: ToolbarSurface::new("wayscriber-toolbar-side", Anchor::LEFT, (24, 0, 24, 24)),
            top_hover: None,
            side_hover: None,
        }
    }
}

impl ToolbarSurfaceManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if any toolbar is visible
    pub fn is_visible(&self) -> bool {
        self.visible || self.top_visible || self.side_visible
    }

    /// Returns true if the top toolbar is visible
    pub fn is_top_visible(&self) -> bool {
        self.top_visible || self.visible
    }

    /// Returns true if the side toolbar is visible
    pub fn is_side_visible(&self) -> bool {
        self.side_visible || self.visible
    }

    /// Set legacy combined visibility (shows/hides both)
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if visible {
            self.top_visible = true;
            self.side_visible = true;
        } else {
            self.top_visible = false;
            self.side_visible = false;
            self.top.destroy();
            self.side.destroy();
            self.top_hover = None;
            self.side_hover = None;
        }
    }

    /// Set visibility of the top toolbar only
    pub fn set_top_visible(&mut self, visible: bool) {
        self.top_visible = visible;
        if !visible {
            self.top.destroy();
            self.top_hover = None;
        }
        // Update legacy flag
        self.visible = self.top_visible && self.side_visible;
    }

    /// Set visibility of the side toolbar only
    pub fn set_side_visible(&mut self, visible: bool) {
        self.side_visible = visible;
        if !visible {
            self.side.destroy();
            self.side_hover = None;
        }
        // Update legacy flag
        self.visible = self.top_visible && self.side_visible;
    }

    pub fn is_toolbar_surface(&self, surface: &wl_surface::WlSurface) -> bool {
        self.top.is_surface(surface) || self.side.is_surface(surface)
    }

    pub fn is_toolbar_layer(
        &self,
        layer: &smithay_client_toolkit::shell::wlr_layer::LayerSurface,
    ) -> bool {
        self.top.is_layer(layer) || self.side.is_layer(layer)
    }

    pub fn ensure_created(
        &mut self,
        qh: &QueueHandle<WaylandState>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        scale: i32,
    ) {
        // Create top toolbar if visible
        if self.is_top_visible() {
            if self.top.logical_size == (0, 0) {
                self.top.set_logical_size((720, 64));
            }
            self.top.ensure_created(qh, compositor, layer_shell, scale);
        }

        // Create side toolbar if visible
        if self.is_side_visible() {
            if self.side.logical_size == (0, 0) {
                self.side.set_logical_size((300, 620));
            }
            self.side.ensure_created(qh, compositor, layer_shell, scale);
        }
    }

    pub fn handle_configure(
        &mut self,
        configure: &LayerSurfaceConfigure,
        layer: &smithay_client_toolkit::shell::wlr_layer::LayerSurface,
    ) -> bool {
        if self.top.is_layer(layer) {
            return self.top.handle_configure(configure);
        }
        if self.side.is_layer(layer) {
            return self.side.handle_configure(configure);
        }
        false
    }

    pub fn render(&mut self, shm: &Shm, snapshot: &ToolbarSnapshot, hover: Option<(f64, f64)>) {
        // Render top toolbar if visible
        if self.is_top_visible() {
            let top_hover = hover.or(self.top_hover);
            if let Err(err) = self.top.render(shm, snapshot, top_hover) {
                log::warn!("Failed to render top toolbar: {}", err);
            }
        }

        // Render side toolbar if visible
        if self.is_side_visible() {
            let side_hover = hover.or(self.side_hover);
            if let Err(err) = self.side.render(shm, snapshot, side_hover) {
                log::warn!("Failed to render side toolbar: {}", err);
            }
        }
    }

    pub fn maybe_update_scale(&mut self, output: Option<&wl_output::WlOutput>, scale: i32) {
        self.top.maybe_update_scale(output, scale);
        self.side.maybe_update_scale(output, scale);
    }

    pub fn mark_dirty(&mut self) {
        self.top.mark_dirty();
        self.side.mark_dirty();
    }

    pub fn pointer_press(
        &mut self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> Option<(ToolbarEvent, bool)> {
        if self.top.is_surface(surface) {
            return self.top.hit_at(position.0, position.1);
        }
        if self.side.is_surface(surface) {
            return self.side.hit_at(position.0, position.1);
        }
        None
    }

    pub fn pointer_motion(
        &mut self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> Option<ToolbarEvent> {
        if self.top.is_surface(surface) {
            if self.top_hover != Some(position) {
                self.top_hover = Some(position);
                self.top.mark_dirty();
            }
            return self.top.drag_at(position.0, position.1);
        }
        if self.side.is_surface(surface) {
            if self.side_hover != Some(position) {
                self.side_hover = Some(position);
                self.side.mark_dirty();
            }
            return self.side.drag_at(position.0, position.1);
        }
        None
    }

    pub fn pointer_leave(&mut self, surface: &wl_surface::WlSurface) {
        if self.top.is_surface(surface) {
            self.top_hover = None;
            self.top.mark_dirty();
        } else if self.side.is_surface(surface) {
            self.side_hover = None;
            self.side.mark_dirty();
        }
    }
}

fn draw_panel_background(ctx: &cairo::Context, width: f64, height: f64) {
    ctx.set_source_rgba(0.05, 0.05, 0.08, 0.78);
    draw_round_rect(ctx, 0.0, 0.0, width, height, 14.0);
    let _ = ctx.fill();
}

fn draw_group_card(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    ctx.set_source_rgba(0.12, 0.12, 0.18, 0.35);
    draw_round_rect(ctx, x, y, w, h, 8.0);
    let _ = ctx.fill();
}

fn render_top_strip(
    ctx: &cairo::Context,
    width: f64,
    height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
) {
    let buttons: &[(Tool, &str)] = &[
        (Tool::Pen, "Pen"),
        (Tool::Line, "Line"),
        (Tool::Rect, "Rect"),
        (Tool::Ellipse, "Circle"),
        (Tool::Arrow, "Arrow"),
        (Tool::Highlight, "Highlight"),
    ];

    let btn_w = 70.0;
    let btn_h = 36.0;
    let gap = 10.0;
    let mut x = 18.0;
    let y = (height - btn_h) / 2.0;

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(14.0);

    for (tool, label) in buttons {
        let is_active = snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
        let is_hover =
            hover.map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h)).unwrap_or(false);
        draw_button(ctx, x, y, btn_w, btn_h, is_active, is_hover);
        draw_label_center(ctx, x, y, btn_w, btn_h, label);
            hits.push(HitRegion {
                rect: (x, y, btn_w, btn_h),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
            });
            x += btn_w + gap;
        }

    // Text mode button
    let is_hover =
        hover.map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h)).unwrap_or(false);
    let text_active = snapshot.text_active;
    draw_button(ctx, x, y, btn_w, btn_h, text_active, is_hover);
    draw_label_center(ctx, x, y, btn_w, btn_h, "Text");
    hits.push(HitRegion {
        rect: (x, y, btn_w, btn_h),
        event: ToolbarEvent::EnterTextMode,
        kind: HitKind::Click,
    });

    // Pin and Close buttons on the right side
    let btn_size = 24.0;
    let btn_gap = 6.0;

    // Pin button
    let pin_x = width - btn_size * 2.0 - btn_gap - 12.0;
    let pin_y = (height - btn_size) / 2.0;
    let pin_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, pin_x, pin_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_pin_button(ctx, pin_x, pin_y, btn_size, snapshot.top_pinned, pin_hover);
    hits.push(HitRegion {
        rect: (pin_x, pin_y, btn_size, btn_size),
        event: ToolbarEvent::PinTopToolbar(!snapshot.top_pinned),
        kind: HitKind::Click,
    });

    // Close button
    let close_x = width - btn_size - 12.0;
    let close_y = (height - btn_size) / 2.0;
    let close_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, close_x, close_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_close_button(ctx, close_x, close_y, btn_size, close_hover);
    hits.push(HitRegion {
        rect: (close_x, close_y, btn_size, btn_size),
        event: ToolbarEvent::CloseTopToolbar,
        kind: HitKind::Click,
    });
}

fn point_in_rect(px: f64, py: f64, x: f64, y: f64, w: f64, h: f64) -> bool {
    px >= x && px <= x + w && py >= y && py <= y + h
}

fn draw_close_button(ctx: &cairo::Context, x: f64, y: f64, size: f64, hover: bool) {
    // Background circle
    let r = size / 2.0;
    let cx = x + r;
    let cy = y + r;

    if hover {
        ctx.set_source_rgba(0.8, 0.3, 0.3, 0.9);
    } else {
        ctx.set_source_rgba(0.5, 0.5, 0.55, 0.7);
    }
    ctx.arc(cx, cy, r, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // X mark
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    ctx.set_line_width(2.0);
    let inset = size * 0.3;
    ctx.move_to(x + inset, y + inset);
    ctx.line_to(x + size - inset, y + size - inset);
    let _ = ctx.stroke();
    ctx.move_to(x + size - inset, y + inset);
    ctx.line_to(x + inset, y + size - inset);
    let _ = ctx.stroke();
}

fn draw_pin_button(ctx: &cairo::Context, x: f64, y: f64, size: f64, pinned: bool, hover: bool) {
    // Background
    let (r, g, b, a) = if pinned {
        (0.25, 0.6, 0.35, 0.95) // Green when pinned
    } else if hover {
        (0.35, 0.35, 0.45, 0.85)
    } else {
        (0.3, 0.3, 0.35, 0.7)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, size, size, 4.0);
    let _ = ctx.fill();

    // Pin icon (simple circle with line)
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    let cx = x + size / 2.0;
    let cy = y + size / 2.0;
    let pin_r = size * 0.2;

    // Pin head (circle)
    ctx.arc(cx, cy - pin_r * 0.5, pin_r, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // Pin needle (line)
    ctx.set_line_width(2.0);
    ctx.move_to(cx, cy + pin_r * 0.5);
    ctx.line_to(cx, cy + pin_r * 2.0);
    let _ = ctx.stroke();
}

fn render_side_palette(
    ctx: &cairo::Context,
    width: f64,
    _height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
) {
    let mut y = 12.0;
    let x = 16.0;
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(13.0);

    // Header row with pin and close buttons
    let btn_size = 22.0;
    let header_y = y;

    // Pin button (toggles pinned state)
    let pin_x = width - btn_size * 2.0 - 20.0;
    let pin_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, pin_x, header_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_pin_button(ctx, pin_x, header_y, btn_size, snapshot.side_pinned, pin_hover);
    hits.push(HitRegion {
        rect: (pin_x, header_y, btn_size, btn_size),
        event: ToolbarEvent::PinSideToolbar(!snapshot.side_pinned),
        kind: HitKind::Click,
    });

    // Close button
    let close_x = width - btn_size - 12.0;
    let close_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, close_x, header_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_close_button(ctx, close_x, header_y, btn_size, close_hover);
    hits.push(HitRegion {
        rect: (close_x, header_y, btn_size, btn_size),
        event: ToolbarEvent::CloseSideToolbar,
        kind: HitKind::Click,
    });

    y += btn_size + 8.0;

    // Colors
    draw_section_label(ctx, x, y, "Colors");
    y += 16.0;
    draw_group_card(ctx, x - 6.0, y - 10.0, width - 2.0 * x + 12.0, 90.0);

    let colors: &[(Color, &str)] = &[
        (RED, "Red"),
        (GREEN, "Green"),
        (BLUE, "Blue"),
        (YELLOW, "Yellow"),
        (ORANGE, "Orange"),
        (PINK, "Pink"),
        (WHITE, "White"),
        (BLACK, "Black"),
    ];

    let swatch = 26.0;
    let gap = 10.0;
    let mut cx = x;
    let mut row_y = y;
    for (color, _name) in colors {
        draw_swatch(ctx, cx, row_y, swatch, *color, *color == snapshot.color);
    hits.push(HitRegion {
        rect: (cx, row_y, swatch, swatch),
        event: ToolbarEvent::SetColor(*color),
        kind: HitKind::Click,
    });
        cx += swatch + gap;
    if cx + swatch > width - 20.0 {
        cx = x;
        row_y += swatch + gap;
    }
}
    y = row_y + swatch + gap + 14.0;

    // Thickness
    draw_section_label(ctx, x, y, "Thickness");
    y += 10.0;
    draw_group_card(ctx, x - 6.0, y - 10.0, width - 2.0 * x + 12.0, 110.0);
    let track_w = width - (2.0 * x);
    let track_h = 10.0;
    let track_y = y + 6.0;
    let knob_r = 9.0;
    let min_thick = 1.0;
    let max_thick = 40.0;
    let t = ((snapshot.thickness - min_thick) / (max_thick - min_thick)).clamp(0.0, 1.0);
    let knob_x = x + t * (track_w - knob_r * 2.0) + knob_r;

    // Track
    ctx.set_source_rgba(0.5, 0.5, 0.6, 0.6);
    ctx.rectangle(x, track_y, track_w, track_h);
    let _ = ctx.fill();
    // Knob
    ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
    ctx.arc(knob_x, track_y + track_h / 2.0, knob_r, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // Minus/Plus nudge buttons
    let btn_w = 30.0;
    let btn_h = 26.0;
    let btn_y = track_y + track_h + 6.0;
    draw_button(ctx, x, btn_y, btn_w, btn_h, false, false);
    draw_label_center(ctx, x, btn_y, btn_w, btn_h, "-");
    hits.push(HitRegion {
        rect: (x, btn_y, btn_w, btn_h),
        event: ToolbarEvent::NudgeThickness(-1.0),
        kind: HitKind::Click,
    });

    draw_button(ctx, x + btn_w + 8.0, btn_y, btn_w, btn_h, false, false);
    draw_label_center(ctx, x + btn_w + 8.0, btn_y, btn_w, btn_h, "+");
    hits.push(HitRegion {
        rect: (x + btn_w + 8.0, btn_y, btn_w, btn_h),
        event: ToolbarEvent::NudgeThickness(1.0),
        kind: HitKind::Click,
    });

    // Thickness readout
    let thickness_text = format!("{:.0}px", snapshot.thickness);
    draw_label_center(
        ctx,
        x + btn_w * 2.0 + 30.0,
        btn_y,
        80.0,
        btn_h,
        &thickness_text,
    );

    y = btn_y + btn_h + 18.0;

    // Font size slider
    draw_section_label(ctx, x, y, "Text size");
    y += 10.0;
    draw_group_card(ctx, x - 6.0, y - 10.0, width - 2.0 * x + 12.0, 110.0);
    let fs_track_w = width - (2.0 * x);
    let fs_track_h = 10.0;
    let fs_track_y = y + 6.0;
    let fs_knob_r = 9.0;
    let fs_min = 8.0;
    let fs_max = 72.0;
    let fs_t = ((snapshot.font_size - fs_min) / (fs_max - fs_min)).clamp(0.0, 1.0);
    let fs_knob_x = x + fs_t * (fs_track_w - fs_knob_r * 2.0) + fs_knob_r;

    ctx.set_source_rgba(0.5, 0.5, 0.6, 0.6);
    ctx.rectangle(x, fs_track_y, fs_track_w, fs_track_h);
    let _ = ctx.fill();
    ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
    ctx.arc(
        fs_knob_x,
        fs_track_y + fs_track_h / 2.0,
        fs_knob_r,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();

    // Font size nudges/readout
    let fs_btn_w = 30.0;
    let fs_btn_h = 26.0;
    let fs_btn_y = fs_track_y + fs_track_h + 6.0;
    draw_button(ctx, x, fs_btn_y, fs_btn_w, fs_btn_h, false, false);
    draw_label_center(ctx, x, fs_btn_y, fs_btn_w, fs_btn_h, "-");
    hits.push(HitRegion {
        rect: (x, fs_btn_y, fs_btn_w, fs_btn_h),
        event: ToolbarEvent::SetFontSize((snapshot.font_size - 2.0).max(fs_min)),
        kind: HitKind::Click,
    });

    draw_button(ctx, x + fs_btn_w + 8.0, fs_btn_y, fs_btn_w, fs_btn_h, false, false);
    draw_label_center(ctx, x + fs_btn_w + 8.0, fs_btn_y, fs_btn_w, fs_btn_h, "+");
    hits.push(HitRegion {
        rect: (x + fs_btn_w + 8.0, fs_btn_y, fs_btn_w, fs_btn_h),
        event: ToolbarEvent::SetFontSize((snapshot.font_size + 2.0).min(fs_max)),
        kind: HitKind::Click,
    });

    let fs_text = format!("{:.0}px", snapshot.font_size);
    draw_label_center(
        ctx,
        x + fs_btn_w * 2.0 + 24.0,
        fs_btn_y,
        80.0,
        fs_btn_h,
        &fs_text,
    );

    hits.push(HitRegion {
        rect: (x, fs_track_y - 8.0, fs_track_w, fs_track_h + 16.0),
        event: ToolbarEvent::SetFontSize(snapshot.font_size),
        kind: HitKind::DragSetFontSize,
    });

    y = fs_btn_y + fs_btn_h + 18.0;

    // Font selection
    draw_section_label(ctx, x, y, "Font");
    y += 10.0;
    draw_group_card(ctx, x - 6.0, y - 10.0, width - 2.0 * x + 12.0, 74.0);
    let font_btn_h = 28.0;
    let font_gap = 8.0;
    let font_btn_w = (width - 2.0 * x - font_gap) / 2.0;
    let fonts = [
        FontDescriptor::new("Sans".to_string(), "bold".to_string(), "normal".to_string()),
        FontDescriptor::new("Monospace".to_string(), "normal".to_string(), "normal".to_string()),
    ];
    let mut fx = x;
    let fy = y;
    for font in fonts {
        let is_active = font.family == snapshot.font.family;
        let font_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, fx, fy, font_btn_w, font_btn_h))
            .unwrap_or(false);
        draw_button(ctx, fx, fy, font_btn_w, font_btn_h, is_active, font_hover);
        draw_label_left(ctx, fx + 8.0, fy, font_btn_w, font_btn_h, &font.family);
        hits.push(HitRegion {
            rect: (fx, fy, font_btn_w, font_btn_h),
            event: ToolbarEvent::SetFont(font.clone()),
            kind: HitKind::Click,
        });
        fx += font_btn_w + font_gap;
    }
    y = fy + font_btn_h + 14.0;

    // Actions row
    draw_section_label(ctx, x, y, "Actions");
    y += 18.0;
    let action_w = (width - 2.0 * x - 16.0) / 3.0; // Fit 3 buttons with gaps
    let action_h = 30.0;
    let ax = x;
    draw_group_card(ctx, x - 6.0, y, width - 2.0 * x + 12.0, 130.0);
    y += 12.0;

    let actions: &[(ToolbarEvent, &str, bool)] = &[
        (ToolbarEvent::Undo, "Undo", snapshot.undo_available),
        (ToolbarEvent::Redo, "Redo", snapshot.redo_available),
        (ToolbarEvent::ClearCanvas, "Clear", true),
        (
            ToolbarEvent::ToggleFreeze,
            if snapshot.frozen_active { "Unfreeze" } else { "Freeze" },
            true,
        ),
        (ToolbarEvent::OpenConfigurator, "Config UI", true),
        (ToolbarEvent::OpenConfigFile, "Config file", true),
    ];
    for (idx, (evt, label, enabled)) in actions.iter().enumerate() {
        let row = idx / 3;
        let col = idx % 3;
        let bx = ax + (action_w + 8.0) * col as f64;
        let by = y + (action_h + 10.0) * row as f64;
        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, action_w, action_h))
            .unwrap_or(false);
        draw_button(ctx, bx, by, action_w, action_h, *enabled, is_hover);
        draw_label_center(ctx, bx, by, action_w, action_h, label);
        if *enabled {
            hits.push(HitRegion {
                rect: (bx, by, action_w, action_h),
                event: evt.clone(),
                kind: HitKind::Click,
            });
        }
    }

    y += action_h * 2.0 + 24.0;

    // Toggles
    draw_section_label(ctx, x, y, "Toggles");
    y += 18.0;
    let toggles: &[(ToolbarEvent, &str, bool)] = &[
        (
            ToolbarEvent::ToggleHighlightTool(!snapshot.highlight_tool_active),
            "Highlight tool",
            snapshot.highlight_tool_active,
        ),
        (
            ToolbarEvent::ToggleClickHighlight(!snapshot.click_highlight_enabled),
            "Click highlight",
            snapshot.click_highlight_enabled,
        ),
    ];

    let toggle_h = 30.0;
    let toggle_w = width - 2.0 * x;
    for (evt, label, active) in toggles {
        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, toggle_w, toggle_h))
            .unwrap_or(false);
        draw_toggle_button(ctx, x, y, toggle_w, toggle_h, *active, is_hover, label);
        hits.push(HitRegion {
            rect: (x, y, toggle_w, toggle_h),
            event: evt.clone(),
            kind: HitKind::Click,
        });
        y += toggle_h + 6.0;
    }

    // Thickness track drag region
    hits.push(HitRegion {
        rect: (x, track_y - 6.0, track_w, track_h + 12.0),
        event: ToolbarEvent::SetThickness(snapshot.thickness),
        kind: HitKind::DragSetThickness,
    });
}

fn draw_button(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, active: bool, hover: bool) {
    let (r, g, b, a) = if active {
        (0.25, 0.5, 0.95, 0.95)
    } else if hover {
        (0.35, 0.35, 0.45, 0.85)
    } else {
        (0.2, 0.22, 0.26, 0.75)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 6.0);
    let _ = ctx.fill();
}

fn draw_toggle_button(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, active: bool, hover: bool, label: &str) {
    // Draw button background
    let (r, g, b, a) = if hover {
        (0.35, 0.35, 0.45, 0.85)
    } else {
        (0.2, 0.22, 0.26, 0.75)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 6.0);
    let _ = ctx.fill();

    // Draw switch on right side
    let switch_w = 36.0;
    let switch_h = 18.0;
    let switch_x = x + w - switch_w - 10.0;
    let switch_y = y + (h - switch_h) / 2.0;
    let switch_r = switch_h / 2.0;

    // Switch track
    if active {
        ctx.set_source_rgba(0.25, 0.6, 0.35, 0.95);
    } else {
        ctx.set_source_rgba(0.3, 0.3, 0.35, 0.8);
    }
    draw_round_rect(ctx, switch_x, switch_y, switch_w, switch_h, switch_r);
    let _ = ctx.fill();

    // Switch knob
    let knob_r = switch_h / 2.0 - 2.0;
    let knob_x = if active {
        switch_x + switch_w - switch_r
    } else {
        switch_x + switch_r
    };
    let knob_y = switch_y + switch_h / 2.0;
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    ctx.arc(knob_x, knob_y, knob_r, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    // Draw label
    draw_label_left(ctx, x + 10.0, y, w - switch_w - 20.0, h, label);
}

fn draw_label_center(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, text: &str) {
    if let Ok(ext) = ctx.text_extents(text) {
        let tx = x + (w - ext.width()) / 2.0 - ext.x_bearing();
        let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.move_to(tx, ty);
        let _ = ctx.show_text(text);
    }
}

fn draw_label_left(ctx: &cairo::Context, x: f64, y: f64, _w: f64, h: f64, text: &str) {
    if let Ok(ext) = ctx.text_extents(text) {
        let ty = y + (h - ext.height()) / 2.0 - ext.y_bearing();
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
        ctx.move_to(x, ty);
        let _ = ctx.show_text(text);
    }
}

fn draw_section_label(ctx: &cairo::Context, x: f64, y: f64, text: &str) {
    ctx.set_source_rgba(0.8, 0.8, 0.85, 0.9);
    ctx.move_to(x, y);
    let _ = ctx.show_text(text);
}

fn draw_swatch(ctx: &cairo::Context, x: f64, y: f64, size: f64, color: Color, active: bool) {
    ctx.set_source_rgba(color.r, color.g, color.b, 1.0);
    draw_round_rect(ctx, x, y, size, size, 4.0);
    let _ = ctx.fill();

    // Add border to dark colors for visibility on dark background
    let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
    if luminance < 0.3 {
        ctx.set_source_rgba(0.5, 0.5, 0.5, 0.8);
        ctx.set_line_width(1.5);
        draw_round_rect(ctx, x, y, size, size, 4.0);
        let _ = ctx.stroke();
    }

    if active {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        ctx.set_line_width(2.0);
        draw_round_rect(ctx, x - 2.0, y - 2.0, size + 4.0, size + 4.0, 5.0);
        let _ = ctx.stroke();
    }
}

fn draw_round_rect(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    let r = radius.min(w / 2.0).min(h / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    ctx.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    ctx.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    ctx.arc(x + r, y + r, r, std::f64::consts::PI, std::f64::consts::PI * 1.5);
    ctx.close_path();
}
