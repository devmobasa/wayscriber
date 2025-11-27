#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

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
    Proxy, QueueHandle,
    protocol::{wl_output, wl_surface},
};

use crate::draw::{BLACK, BLUE, Color, FontDescriptor, GREEN, ORANGE, PINK, RED, WHITE, YELLOW};
use crate::input::Tool;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

use super::state::WaylandState;
use super::toolbar_icons;

#[derive(Clone, Debug)]
struct HitRegion {
    rect: (f64, f64, f64, f64), // x, y, w, h
    event: ToolbarEvent,
    kind: HitKind,
    tooltip: Option<&'static str>,
}

impl HitRegion {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.rect.0
            && x <= self.rect.0 + self.rect.2
            && y >= self.rect.1
            && y <= self.rect.1 + self.rect.3
    }
}

#[derive(Clone, Debug, PartialEq)]
enum HitKind {
    Click,
    DragSetThickness { min: f64, max: f64 },
    DragSetFontSize,
    PickColor { x: f64, y: f64, w: f64, h: f64 },
    DragUndoDelay,
    DragRedoDelay,
    DragCustomUndoDelay,
    DragCustomRedoDelay,
}

fn delay_secs_from_t(t: f64) -> f64 {
    const MIN_DELAY_S: f64 = 0.05;
    const MAX_DELAY_S: f64 = 5.0;
    MIN_DELAY_S + t.clamp(0.0, 1.0) * (MAX_DELAY_S - MIN_DELAY_S)
}

fn delay_t_from_ms(delay_ms: u64) -> f64 {
    const MIN_DELAY_S: f64 = 0.05;
    const MAX_DELAY_S: f64 = 5.0;
    let delay_s = (delay_ms as f64 / 1000.0).clamp(MIN_DELAY_S, MAX_DELAY_S);
    (delay_s - MIN_DELAY_S) / (MAX_DELAY_S - MIN_DELAY_S)
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

    fn render(
        &mut self,
        shm: &Shm,
        snapshot: &ToolbarSnapshot,
        hover: Option<(f64, f64)>,
    ) -> Result<()> {
        if !self.configured || !self.dirty || self.width == 0 || self.height == 0 {
            return Ok(());
        }

        let (phys_w, phys_h) = (
            self.width.saturating_mul(self.scale as u32),
            self.height.saturating_mul(self.scale as u32),
        );

        if self.pool.is_none() {
            let buffer_size = (phys_w * phys_h * 4) as usize;
            let pool =
                SlotPool::new(buffer_size, shm).context("Failed to create toolbar SlotPool")?;
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
        let ctx =
            cairo::Context::new(&surface).context("Failed to create cairo context for toolbar")?;

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
                let start_drag = matches!(
                    hit.kind,
                    HitKind::DragSetThickness { .. }
                        | HitKind::DragSetFontSize
                        | HitKind::PickColor { .. }
                        | HitKind::DragUndoDelay
                        | HitKind::DragRedoDelay
                        | HitKind::DragCustomUndoDelay
                        | HitKind::DragCustomRedoDelay
                );
                let event = match hit.kind {
                    HitKind::DragSetThickness { min, max } => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        let value = min + t * (max - min);
                        ToolbarEvent::SetThickness(value)
                    }
                    HitKind::DragSetFontSize => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        let value = 8.0 + t * (72.0 - 8.0);
                        ToolbarEvent::SetFontSize(value)
                    }
                    HitKind::PickColor { x: px, y: py, w, h } => {
                        let hue = ((x - px) / w).clamp(0.0, 1.0);
                        let value = (1.0 - (y - py) / h).clamp(0.0, 1.0);
                        ToolbarEvent::SetColor(hsv_to_rgb(hue, 1.0, value))
                    }
                    HitKind::DragUndoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        ToolbarEvent::SetUndoDelay(delay_secs_from_t(t))
                    }
                    HitKind::DragRedoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        ToolbarEvent::SetRedoDelay(delay_secs_from_t(t))
                    }
                    HitKind::DragCustomUndoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        ToolbarEvent::SetCustomUndoDelay(delay_secs_from_t(t))
                    }
                    HitKind::DragCustomRedoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        ToolbarEvent::SetCustomRedoDelay(delay_secs_from_t(t))
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
                    HitKind::DragSetThickness { min, max } => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        let value = min + t * (max - min);
                        return Some(ToolbarEvent::SetThickness(value));
                    }
                    HitKind::DragSetFontSize => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        let value = 8.0 + t * (72.0 - 8.0);
                        return Some(ToolbarEvent::SetFontSize(value));
                    }
                    HitKind::PickColor { x: px, y: py, w, h } => {
                        let hue = ((x - px) / w).clamp(0.0, 1.0);
                        let value = (1.0 - (y - py) / h).clamp(0.0, 1.0);
                        return Some(ToolbarEvent::SetColor(hsv_to_rgb(hue, 1.0, value)));
                    }
                    HitKind::DragUndoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        return Some(ToolbarEvent::SetUndoDelay(delay_secs_from_t(t)));
                    }
                    HitKind::DragRedoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        return Some(ToolbarEvent::SetRedoDelay(delay_secs_from_t(t)));
                    }
                    HitKind::DragCustomUndoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        return Some(ToolbarEvent::SetCustomUndoDelay(delay_secs_from_t(t)));
                    }
                    HitKind::DragCustomRedoDelay => {
                        let t = ((x - hit.rect.0) / hit.rect.2).clamp(0.0, 1.0);
                        return Some(ToolbarEvent::SetCustomRedoDelay(delay_secs_from_t(t)));
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
    /// Combined visibility flag (true when any toolbar visible)
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
        self.top_visible
    }

    /// Returns true if the side toolbar is visible
    pub fn is_side_visible(&self) -> bool {
        self.side_visible
    }

    /// Set combined visibility (shows/hides both toolbars)
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
        // Update combined flag: any toolbar visible keeps overlays alive.
        self.visible = self.top_visible || self.side_visible;
    }

    /// Set visibility of the side toolbar only
    pub fn set_side_visible(&mut self, visible: bool) {
        self.side_visible = visible;
        if !visible {
            self.side.destroy();
            self.side_hover = None;
        }
        // Update combined flag: any toolbar visible keeps overlays alive.
        self.visible = self.top_visible || self.side_visible;
    }

    pub fn is_toolbar_surface(&self, surface: &wl_surface::WlSurface) -> bool {
        self.top.is_surface(surface) || self.side.is_surface(surface)
    }

    pub fn destroy_all(&mut self) {
        self.top.destroy();
        self.side.destroy();
        self.top_hover = None;
        self.side_hover = None;
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
        snapshot: &ToolbarSnapshot,
    ) {
        let use_icons = snapshot.use_icons;

        // Create top toolbar if visible
        if self.is_top_visible() {
            // Dynamic size based on mode:
            // - Icon mode: 705px wide (7 tools + text + clear + highlight + icons checkbox + pin/close)
            //              80px tall (42px buttons at y=6, fill toggle below, tooltip space)
            // - Text mode: 845px wide (text labels need more space), 56px tall (no tooltips)
            let target_size = if use_icons { (705, 80) } else { (845, 56) };

            // Recreate if size changed
            if self.top.logical_size != (0, 0) && self.top.logical_size != target_size {
                self.top.destroy();
            }

            if self.top.logical_size == (0, 0) || self.top.logical_size != target_size {
                self.top.set_logical_size(target_size);
            }
            self.top.ensure_created(qh, compositor, layer_shell, scale);
        }

        // Create side toolbar if visible
        if self.is_side_visible() {
            // Calculate dynamic height based on which sections are expanded
            let base_height: u32 = 30; // Header
            let section_gap: u32 = 12;

            // Colors section: picker + basic colors + optional extended row
            let colors_h: u32 = 28 + 24 + 8 + 30 + if snapshot.show_more_colors { 30 } else { 0 };

            // Thickness + Text size sections (compact with sliders)
            let thickness_h: u32 = 52;
            let text_size_h: u32 = 52;

            // Font section
            let font_h: u32 = 50;

            // Actions section (now before Step Undo/Redo)
            let actions_h: u32 = if snapshot.show_actions_section {
                if use_icons {
                    20 + 24 + 6 + 42 * 2 + 6 * 2 // 2 rows of 42px icons (unified with top toolbar)
                } else {
                    20 + 24 + 6 + 24 * 5 + 5 * 5 // 5 rows of text buttons
                }
            } else {
                20 + 24 // Just checkbox
            };

            // Step Undo/Redo section (includes delay sliders if enabled)
            let delay_h = if snapshot.show_delay_sliders { 55 } else { 0 };
            let step_h: u32 = 20
                + 24
                + if snapshot.custom_section_enabled {
                    120
                } else {
                    0
                }
                + delay_h;

            let total_gaps = 6; // section separators (colors, thickness, text size, font, actions, step)
            let total_height = base_height
                + colors_h
                + thickness_h
                + text_size_h
                + font_h
                + actions_h
                + step_h
                + section_gap * total_gaps
                + 20;
            let target_size = (260, total_height);

            if self.side.logical_size != (0, 0) && self.side.logical_size != target_size {
                self.side.destroy();
            }

            if self.side.logical_size == (0, 0) || self.side.logical_size != target_size {
                self.side.set_logical_size(target_size);
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
    // High contrast semi-opaque dark background for readability on any content
    ctx.set_source_rgba(0.05, 0.05, 0.08, 0.92); // Increased opacity from 0.78 for better contrast
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
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(14.0);

    let use_icons = snapshot.use_icons;
    let gap = 8.0;
    let mut x = 16.0;

    // Tool definitions with icons and labels
    type IconFn = fn(&cairo::Context, f64, f64, f64);
    let buttons: &[(Tool, IconFn, &str)] = &[
        (
            Tool::Select,
            toolbar_icons::draw_icon_select as IconFn,
            "Select",
        ),
        (Tool::Pen, toolbar_icons::draw_icon_pen as IconFn, "Pen"),
        (
            Tool::Marker,
            toolbar_icons::draw_icon_marker as IconFn,
            "Marker",
        ),
        (
            Tool::Eraser,
            toolbar_icons::draw_icon_eraser as IconFn,
            "Eraser",
        ),
        (Tool::Line, toolbar_icons::draw_icon_line as IconFn, "Line"),
        (Tool::Rect, toolbar_icons::draw_icon_rect as IconFn, "Rect"),
        (
            Tool::Ellipse,
            toolbar_icons::draw_icon_circle as IconFn,
            "Circle",
        ),
        (
            Tool::Arrow,
            toolbar_icons::draw_icon_arrow as IconFn,
            "Arrow",
        ),
    ];

    if use_icons {
        // Icon mode: square buttons with icons
        // Position buttons near top (y=6) to leave room for fill toggle and tooltips below
        let btn_size = 42.0;
        let y = 6.0;
        let icon_size = 26.0;

        // Track Rect and Circle button positions for fill toggle placement
        let mut rect_x = 0.0;
        let mut circle_end_x = 0.0;

        for (tool, icon_fn, label) in buttons {
            // Track positions for Rect and Circle/Ellipse
            if *tool == Tool::Rect {
                rect_x = x;
            }
            if *tool == Tool::Ellipse {
                circle_end_x = x + btn_size;
            }

            let is_active = snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
            let is_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
                .unwrap_or(false);
            draw_button(ctx, x, y, btn_size, btn_size, is_active, is_hover);

            set_icon_color(ctx, is_hover);
            let icon_x = x + (btn_size - icon_size) / 2.0;
            let icon_y = y + (btn_size - icon_size) / 2.0;
            icon_fn(ctx, icon_x, icon_y, icon_size);

            hits.push(HitRegion {
                rect: (x, y, btn_size, btn_size),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
                tooltip: Some(*label),
            });
            x += btn_size + gap;
        }

        // Small fill toggle below Rect and Circle buttons
        let fill_y = y + btn_size + 2.0;
        let fill_w = circle_end_x - rect_x;
        let fill_h = 18.0;
        let fill_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, rect_x, fill_y, fill_w, fill_h))
            .unwrap_or(false);
        draw_mini_checkbox(
            ctx,
            rect_x,
            fill_y,
            fill_w,
            fill_h,
            snapshot.fill_enabled,
            fill_hover,
            "Fill",
        );
        hits.push(HitRegion {
            rect: (rect_x, fill_y, fill_w, fill_h),
            event: ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
            kind: HitKind::Click,
            tooltip: None,
        });

        // Text mode button with icon
        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
            .unwrap_or(false);
        draw_button(
            ctx,
            x,
            y,
            btn_size,
            btn_size,
            snapshot.text_active,
            is_hover,
        );
        set_icon_color(ctx, is_hover);
        toolbar_icons::draw_icon_text(
            ctx,
            x + (btn_size - icon_size) / 2.0,
            y + (btn_size - icon_size) / 2.0,
            icon_size,
        );
        hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::EnterTextMode,
            kind: HitKind::Click,
            tooltip: Some("Text"),
        });
        x += btn_size + gap;

        // Clear button with icon
        let clear_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
            .unwrap_or(false);
        draw_button(ctx, x, y, btn_size, btn_size, false, clear_hover);
        set_icon_color(ctx, clear_hover);
        toolbar_icons::draw_icon_clear(
            ctx,
            x + (btn_size - icon_size) / 2.0,
            y + (btn_size - icon_size) / 2.0,
            icon_size,
        );
        hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::ClearCanvas,
            kind: HitKind::Click,
            tooltip: Some("Clear"),
        });
        x += btn_size + gap;

        // Highlight toggle button (toggles both highlight tool and click highlight)
        let highlight_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
            .unwrap_or(false);
        draw_button(
            ctx,
            x,
            y,
            btn_size,
            btn_size,
            snapshot.any_highlight_active,
            highlight_hover,
        );
        set_icon_color(ctx, highlight_hover);
        toolbar_icons::draw_icon_highlight(
            ctx,
            x + (btn_size - icon_size) / 2.0,
            y + (btn_size - icon_size) / 2.0,
            icon_size,
        );
        hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active),
            kind: HitKind::Click,
            tooltip: Some("Click highlight"),
        });
        x += btn_size + gap;

        // Icons toggle checkbox
        let icons_w = 70.0;
        let icons_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, icons_w, btn_size))
            .unwrap_or(false);
        draw_checkbox(ctx, x, y, icons_w, btn_size, true, icons_hover, "Icons");
        hits.push(HitRegion {
            rect: (x, y, icons_w, btn_size),
            event: ToolbarEvent::ToggleIconMode(false),
            kind: HitKind::Click,
            tooltip: None,
        });
    } else {
        // Text mode: rectangular buttons with labels
        let btn_w = 60.0;
        let btn_h = 36.0;
        let y = (height - btn_h) / 2.0;

        for (tool, _icon_fn, label) in buttons {
            let is_active = snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
            let is_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
                .unwrap_or(false);
            draw_button(ctx, x, y, btn_w, btn_h, is_active, is_hover);
            draw_label_center(ctx, x, y, btn_w, btn_h, label);
            hits.push(HitRegion {
                rect: (x, y, btn_w, btn_h),
                event: ToolbarEvent::SelectTool(*tool),
                kind: HitKind::Click,
                tooltip: None,
            });
            x += btn_w + gap;
        }

        // Fill toggle checkbox
        let fill_w = 64.0;
        let fill_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, fill_w, btn_h))
            .unwrap_or(false);
        draw_checkbox(
            ctx,
            x,
            y,
            fill_w,
            btn_h,
            snapshot.fill_enabled,
            fill_hover,
            "Fill",
        );
        hits.push(HitRegion {
            rect: (x, y, fill_w, btn_h),
            event: ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
            kind: HitKind::Click,
            tooltip: None,
        });
        x += fill_w + gap;

        // Text mode button
        let is_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, btn_h))
            .unwrap_or(false);
        draw_button(ctx, x, y, btn_w, btn_h, snapshot.text_active, is_hover);
        draw_label_center(ctx, x, y, btn_w, btn_h, "Text");
        hits.push(HitRegion {
            rect: (x, y, btn_w, btn_h),
            event: ToolbarEvent::EnterTextMode,
            kind: HitKind::Click,
            tooltip: None,
        });
        x += btn_w + gap;

        // Icons toggle checkbox
        let icons_w = 70.0;
        let icons_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, icons_w, btn_h))
            .unwrap_or(false);
        draw_checkbox(ctx, x, y, icons_w, btn_h, false, icons_hover, "Icons");
        hits.push(HitRegion {
            rect: (x, y, icons_w, btn_h),
            event: ToolbarEvent::ToggleIconMode(true),
            kind: HitKind::Click,
            tooltip: None,
        });
    }

    // Pin and Close buttons on the right side
    // Align vertically with tool buttons: y=6 for icon mode (42px buttons), centered for text mode
    let btn_size = 24.0;
    let btn_gap = 6.0;
    let btn_y = if use_icons {
        // Center with 42px tool buttons at y=6: (6 + 42/2) - 24/2 = 27 - 12 = 15
        15.0
    } else {
        (height - btn_size) / 2.0
    };

    let pin_x = width - btn_size * 2.0 - btn_gap - 12.0;
    let pin_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, pin_x, btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_pin_button(ctx, pin_x, btn_y, btn_size, snapshot.top_pinned, pin_hover);
    hits.push(HitRegion {
        rect: (pin_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::PinTopToolbar(!snapshot.top_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.top_pinned { "Unpin" } else { "Pin" }),
    });

    let close_x = width - btn_size - 12.0;
    let close_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, close_x, btn_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_close_button(ctx, close_x, btn_y, btn_size, close_hover);
    hits.push(HitRegion {
        rect: (close_x, btn_y, btn_size, btn_size),
        event: ToolbarEvent::CloseTopToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close"),
    });

    // Draw tooltip for hovered icon button (below buttons, toolbar is tall enough)
    draw_tooltip(ctx, hits, hover, width, false);
}

fn point_in_rect(px: f64, py: f64, x: f64, y: f64, w: f64, h: f64) -> bool {
    px >= x && px <= x + w && py >= y && py <= y + h
}

/// Set icon glyph color based on hover state for better visual feedback
fn set_icon_color(ctx: &cairo::Context, hover: bool) {
    if hover {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0); // Bright white when hovered
    } else {
        ctx.set_source_rgba(0.95, 0.95, 0.95, 0.9); // Slightly dimmed when not hovered
    }
}

/// Draw tooltip near the hovered element
/// If `above` is true, tooltip appears above the button (for top toolbar)
fn draw_tooltip(
    ctx: &cairo::Context,
    hits: &[HitRegion],
    hover: Option<(f64, f64)>,
    panel_width: f64,
    above: bool,
) {
    let Some((hx, hy)) = hover else { return };

    // Find hovered region with tooltip
    for hit in hits {
        if hit.contains(hx, hy)
            && let Some(text) = hit.tooltip
        {
            ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            ctx.set_font_size(12.0);

            if let Ok(ext) = ctx.text_extents(text) {
                let pad = 6.0;
                let tooltip_w = ext.width() + pad * 2.0;
                let tooltip_h = ext.height() + pad * 2.0;

                // Position centered on button
                let btn_center_x = hit.rect.0 + hit.rect.2 / 2.0;
                let mut tooltip_x = btn_center_x - tooltip_w / 2.0;

                // Position above or below based on parameter (with increased gap)
                let gap = 6.0; // Increased from 4.0 for better spacing
                let tooltip_y = if above {
                    hit.rect.1 - tooltip_h - gap
                } else {
                    hit.rect.1 + hit.rect.3 + gap
                };

                // Clamp to panel bounds
                if tooltip_x < 4.0 {
                    tooltip_x = 4.0;
                }
                if tooltip_x + tooltip_w > panel_width - 4.0 {
                    tooltip_x = panel_width - tooltip_w - 4.0;
                }

                // Draw subtle drop shadow (offset darker rounded rect)
                let shadow_offset = 2.0;
                ctx.set_source_rgba(0.0, 0.0, 0.0, 0.3);
                draw_round_rect(
                    ctx,
                    tooltip_x + shadow_offset,
                    tooltip_y + shadow_offset,
                    tooltip_w,
                    tooltip_h,
                    4.0,
                );
                let _ = ctx.fill();

                // Draw tooltip background
                ctx.set_source_rgba(0.1, 0.1, 0.15, 0.95);
                draw_round_rect(ctx, tooltip_x, tooltip_y, tooltip_w, tooltip_h, 4.0);
                let _ = ctx.fill();

                // Draw border
                ctx.set_source_rgba(0.4, 0.4, 0.5, 0.8);
                ctx.set_line_width(1.0);
                draw_round_rect(ctx, tooltip_x, tooltip_y, tooltip_w, tooltip_h, 4.0);
                let _ = ctx.stroke();

                // Draw text
                ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
                ctx.move_to(
                    tooltip_x + pad - ext.x_bearing(),
                    tooltip_y + pad - ext.y_bearing(),
                );
                let _ = ctx.show_text(text);
            }
            break;
        }
    }
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
    let use_icons = snapshot.use_icons;
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(13.0);

    // Header row with Icons checkbox, More colors checkbox, pin and close buttons
    let btn_size = 22.0;
    let header_y = y;

    // Icons checkbox on the left
    let icons_w = 58.0;
    let icons_h = btn_size;
    let icons_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, header_y, icons_w, icons_h))
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        header_y,
        icons_w,
        icons_h,
        use_icons,
        icons_hover,
        "Icons",
    );
    hits.push(HitRegion {
        rect: (x, header_y, icons_w, icons_h),
        event: ToolbarEvent::ToggleIconMode(!use_icons),
        kind: HitKind::Click,
        tooltip: None,
    });

    // Pin button (toggles pinned state)
    let pin_x = width - btn_size * 2.0 - 20.0;
    let pin_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, pin_x, header_y, btn_size, btn_size))
        .unwrap_or(false);
    draw_pin_button(
        ctx,
        pin_x,
        header_y,
        btn_size,
        snapshot.side_pinned,
        pin_hover,
    );
    hits.push(HitRegion {
        rect: (pin_x, header_y, btn_size, btn_size),
        event: ToolbarEvent::PinSideToolbar(!snapshot.side_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.side_pinned { "Unpin" } else { "Pin" }),
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
        tooltip: Some("Close"),
    });

    y += btn_size + 6.0;

    let card_x = x - 6.0;
    let card_w = width - 2.0 * x + 12.0;
    let section_gap = 12.0;

    // ===== Colors Section =====
    // Basic colors (6) - always shown
    let basic_colors: &[(Color, &str)] = &[
        (RED, "Red"),
        (GREEN, "Green"),
        (BLUE, "Blue"),
        (YELLOW, "Yellow"),
        (WHITE, "White"),
        (BLACK, "Black"),
    ];
    // Extended colors - shown when "More colors" is checked
    let extended_colors: &[(Color, &str)] = &[
        (ORANGE, "Orange"),
        (PINK, "Pink"),
        (
            Color {
                r: 0.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            "Cyan",
        ),
        (
            Color {
                r: 0.6,
                g: 0.4,
                b: 0.8,
                a: 1.0,
            },
            "Purple",
        ),
        (
            Color {
                r: 0.4,
                g: 0.4,
                b: 0.4,
                a: 1.0,
            },
            "Gray",
        ),
    ];

    let swatch = 24.0;
    let swatch_gap = 6.0;
    let basic_rows = 1;
    let extended_rows = if snapshot.show_more_colors { 1 } else { 0 };
    // Color picker + swatches (checkbox moved to header)
    let picker_h = 24.0;
    let colors_card_h =
        28.0 + picker_h + 8.0 + (swatch + swatch_gap) * (basic_rows + extended_rows) as f64;

    draw_group_card(ctx, card_x, y, card_w, colors_card_h);
    draw_section_label(ctx, x, y + 12.0, "Colors");

    // Color picker gradient
    let picker_y = y + 24.0;
    let picker_w = card_w - 12.0;
    draw_color_picker(ctx, x, picker_y, picker_w, picker_h);
    hits.push(HitRegion {
        rect: (x, picker_y, picker_w, picker_h),
        event: ToolbarEvent::SetColor(snapshot.color),
        kind: HitKind::PickColor {
            x,
            y: picker_y,
            w: picker_w,
            h: picker_h,
        },
        tooltip: None,
    });

    // Basic colors row
    let mut cx = x;
    let mut row_y = picker_y + picker_h + 8.0;
    for (color, _name) in basic_colors {
        draw_swatch(ctx, cx, row_y, swatch, *color, *color == snapshot.color);
        hits.push(HitRegion {
            rect: (cx, row_y, swatch, swatch),
            event: ToolbarEvent::SetColor(*color),
            kind: HitKind::Click,
            tooltip: None,
        });
        cx += swatch + swatch_gap;
    }

    // "+" button to show more colors (when collapsed)
    if !snapshot.show_more_colors {
        let plus_btn_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, cx, row_y, swatch, swatch))
            .unwrap_or(false);
        draw_button(ctx, cx, row_y, swatch, swatch, false, plus_btn_hover);
        set_icon_color(ctx, plus_btn_hover);
        toolbar_icons::draw_icon_plus(
            ctx,
            cx + (swatch - 14.0) / 2.0,
            row_y + (swatch - 14.0) / 2.0,
            14.0,
        );
        hits.push(HitRegion {
            rect: (cx, row_y, swatch, swatch),
            event: ToolbarEvent::ToggleMoreColors(true),
            kind: HitKind::Click,
            tooltip: Some("More colors"),
        });
    }
    row_y += swatch + swatch_gap;

    // Extended colors row (if enabled)
    if snapshot.show_more_colors {
        cx = x;
        for (color, _name) in extended_colors {
            draw_swatch(ctx, cx, row_y, swatch, *color, *color == snapshot.color);
            hits.push(HitRegion {
                rect: (cx, row_y, swatch, swatch),
                event: ToolbarEvent::SetColor(*color),
                kind: HitKind::Click,
                tooltip: None,
            });
            cx += swatch + swatch_gap;
        }

        // "-" button to hide extended colors
        let minus_btn_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, cx, row_y, swatch, swatch))
            .unwrap_or(false);
        draw_button(ctx, cx, row_y, swatch, swatch, false, minus_btn_hover);
        set_icon_color(ctx, minus_btn_hover);
        toolbar_icons::draw_icon_minus(
            ctx,
            cx + (swatch - 14.0) / 2.0,
            row_y + (swatch - 14.0) / 2.0,
            14.0,
        );
        hits.push(HitRegion {
            rect: (cx, row_y, swatch, swatch),
            event: ToolbarEvent::ToggleMoreColors(false),
            kind: HitKind::Click,
            tooltip: Some("Hide colors"),
        });
    }

    y += colors_card_h + section_gap;

    // ===== Thickness Section =====
    // Layout: [-] [slider] [+] value - all on one row
    let slider_card_h = 52.0;
    draw_group_card(ctx, card_x, y, card_w, slider_card_h);
    let thickness_label = if snapshot.thickness_targets_eraser {
        "Eraser size"
    } else if snapshot.thickness_targets_marker {
        "Marker opacity"
    } else {
        "Thickness"
    };
    draw_section_label(ctx, x, y + 12.0, thickness_label);

    let btn_size = 24.0;
    let nudge_icon_size = 14.0;
    let value_w = 40.0;
    let slider_row_y = y + 26.0;
    let track_h = 8.0;
    let knob_r = 7.0;
    let (min_thick, max_thick, nudge_step) = if snapshot.thickness_targets_marker {
        (0.05, 0.9, 0.05)
    } else {
        (1.0, 50.0, 1.0)
    };

    // Minus button on left
    let minus_x = x;
    draw_button(ctx, minus_x, slider_row_y, btn_size, btn_size, false, false);
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    toolbar_icons::draw_icon_minus(
        ctx,
        minus_x + (btn_size - nudge_icon_size) / 2.0,
        slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (minus_x, slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::NudgeThickness(-nudge_step),
        kind: HitKind::Click,
        tooltip: None,
    });

    // Plus button on right (before value)
    let plus_x = width - x - btn_size - value_w - 4.0;
    draw_button(ctx, plus_x, slider_row_y, btn_size, btn_size, false, false);
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    toolbar_icons::draw_icon_plus(
        ctx,
        plus_x + (btn_size - nudge_icon_size) / 2.0,
        slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (plus_x, slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::NudgeThickness(nudge_step),
        kind: HitKind::Click,
        tooltip: None,
    });

    // Slider track between buttons
    let track_x = minus_x + btn_size + 6.0;
    let track_w = plus_x - track_x - 6.0;
    let track_y = slider_row_y + (btn_size - track_h) / 2.0;
    let t = ((snapshot.thickness - min_thick) / (max_thick - min_thick)).clamp(0.0, 1.0);
    let knob_x = track_x + t * (track_w - knob_r * 2.0) + knob_r;

    ctx.set_source_rgba(0.5, 0.5, 0.6, 0.6);
    draw_round_rect(ctx, track_x, track_y, track_w, track_h, 4.0);
    let _ = ctx.fill();
    ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
    ctx.arc(
        knob_x,
        track_y + track_h / 2.0,
        knob_r,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();

    hits.push(HitRegion {
        rect: (track_x, track_y - 6.0, track_w, track_h + 12.0),
        event: ToolbarEvent::SetThickness(snapshot.thickness),
        kind: HitKind::DragSetThickness {
            min: min_thick,
            max: max_thick,
        },
        tooltip: None,
    });

    // Value display on far right
    let thickness_text = if snapshot.thickness_targets_marker {
        format!("{:.0}%", snapshot.thickness * 100.0)
    } else {
        format!("{:.0}px", snapshot.thickness)
    };
    let value_x = width - x - value_w;
    draw_label_center(
        ctx,
        value_x,
        slider_row_y,
        value_w,
        btn_size,
        &thickness_text,
    );
    // Eraser brush shape toggle removed; default brush remains circle.
    y += slider_card_h + section_gap;

    // ===== Text Size Section =====
    // Layout: [-] [slider] [+] value - all on one row
    draw_group_card(ctx, card_x, y, card_w, slider_card_h);
    draw_section_label(ctx, x, y + 12.0, "Text size");

    let fs_min = 8.0;
    let fs_max = 72.0;
    let fs_slider_row_y = y + 26.0;

    // Minus button on left
    let fs_minus_x = x;
    draw_button(
        ctx,
        fs_minus_x,
        fs_slider_row_y,
        btn_size,
        btn_size,
        false,
        false,
    );
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    toolbar_icons::draw_icon_minus(
        ctx,
        fs_minus_x + (btn_size - nudge_icon_size) / 2.0,
        fs_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (fs_minus_x, fs_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::SetFontSize((snapshot.font_size - 2.0).max(fs_min)),
        kind: HitKind::Click,
        tooltip: None,
    });

    // Plus button on right (before value)
    let fs_plus_x = width - x - btn_size - value_w - 4.0;
    draw_button(
        ctx,
        fs_plus_x,
        fs_slider_row_y,
        btn_size,
        btn_size,
        false,
        false,
    );
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    toolbar_icons::draw_icon_plus(
        ctx,
        fs_plus_x + (btn_size - nudge_icon_size) / 2.0,
        fs_slider_row_y + (btn_size - nudge_icon_size) / 2.0,
        nudge_icon_size,
    );
    hits.push(HitRegion {
        rect: (fs_plus_x, fs_slider_row_y, btn_size, btn_size),
        event: ToolbarEvent::SetFontSize((snapshot.font_size + 2.0).min(fs_max)),
        kind: HitKind::Click,
        tooltip: None,
    });

    // Slider track between buttons
    let fs_track_x = fs_minus_x + btn_size + 6.0;
    let fs_track_w = fs_plus_x - fs_track_x - 6.0;
    let fs_track_y = fs_slider_row_y + (btn_size - track_h) / 2.0;
    let fs_t = ((snapshot.font_size - fs_min) / (fs_max - fs_min)).clamp(0.0, 1.0);
    let fs_knob_x = fs_track_x + fs_t * (fs_track_w - knob_r * 2.0) + knob_r;

    ctx.set_source_rgba(0.5, 0.5, 0.6, 0.6);
    draw_round_rect(ctx, fs_track_x, fs_track_y, fs_track_w, track_h, 4.0);
    let _ = ctx.fill();
    ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
    ctx.arc(
        fs_knob_x,
        fs_track_y + track_h / 2.0,
        knob_r,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();

    hits.push(HitRegion {
        rect: (fs_track_x, fs_track_y - 6.0, fs_track_w, track_h + 12.0),
        event: ToolbarEvent::SetFontSize(snapshot.font_size),
        kind: HitKind::DragSetFontSize,
        tooltip: None,
    });

    // Value display on far right
    let fs_text = format!("{:.0}pt", snapshot.font_size);
    draw_label_center(
        ctx,
        width - x - value_w,
        fs_slider_row_y,
        value_w,
        btn_size,
        &fs_text,
    );

    y += slider_card_h + section_gap;

    // ===== Font Section =====
    let font_card_h = 50.0;
    draw_group_card(ctx, card_x, y, card_w, font_card_h);
    draw_section_label(ctx, x, y + 14.0, "Font");

    let font_btn_h = 24.0;
    let font_gap = 8.0;
    let font_btn_w = (width - 2.0 * x - font_gap) / 2.0;
    let fonts = [
        FontDescriptor::new("Sans".to_string(), "bold".to_string(), "normal".to_string()),
        FontDescriptor::new(
            "Monospace".to_string(),
            "normal".to_string(),
            "normal".to_string(),
        ),
    ];
    let mut fx = x;
    let fy = y + 22.0;
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
            tooltip: None,
        });
        fx += font_btn_w + font_gap;
    }

    y += font_card_h + section_gap;

    // ===== Actions Section =====
    // Checkbox to toggle visibility + content when enabled
    let actions_checkbox_h = 24.0;
    let actions_content_h = if snapshot.show_actions_section {
        if use_icons {
            // 10 icon buttons (5 per row = 2 rows)
            let icon_btn_size = 42.0; // Unified with top toolbar
            let icon_gap = 6.0;
            let icon_rows = 2;
            (icon_btn_size + icon_gap) * icon_rows as f64
        } else {
            let action_h = 24.0;
            let action_gap = 5.0;
            let action_rows = 5; // 10 items / 2 cols = 5 rows
            (action_h + action_gap) * action_rows as f64
        }
    } else {
        0.0
    };
    let actions_card_h = 20.0 + actions_checkbox_h + actions_content_h;

    draw_group_card(ctx, card_x, y, card_w, actions_card_h);
    draw_section_label(ctx, x, y + 14.0, "Actions");

    // Actions toggle checkbox
    let actions_toggle_y = y + 22.0;
    let actions_toggle_w = card_w - 12.0;
    let actions_toggle_hover = hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                x,
                actions_toggle_y,
                actions_toggle_w,
                actions_checkbox_h,
            )
        })
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        actions_toggle_y,
        actions_toggle_w,
        actions_checkbox_h,
        snapshot.show_actions_section,
        actions_toggle_hover,
        "Show actions",
    );
    hits.push(HitRegion {
        rect: (x, actions_toggle_y, actions_toggle_w, actions_checkbox_h),
        event: ToolbarEvent::ToggleActionsSection(!snapshot.show_actions_section),
        kind: HitKind::Click,
        tooltip: None,
    });

    if snapshot.show_actions_section {
        let actions_start_y = actions_toggle_y + actions_checkbox_h + 6.0;

        // Action definitions with icons and labels (now includes delay actions as icons)
        // Row 1: Undo, Redo, UndoAll, RedoAll, UndoAllDelay
        // Row 2: Clear, Freeze, ConfigUI, ConfigFile, RedoAllDelay
        type IconFn = fn(&cairo::Context, f64, f64, f64);
        let all_actions: &[(ToolbarEvent, IconFn, &str, bool)] = &[
            // Row 1
            (
                ToolbarEvent::Undo,
                toolbar_icons::draw_icon_undo as IconFn,
                "Undo",
                snapshot.undo_available,
            ),
            (
                ToolbarEvent::Redo,
                toolbar_icons::draw_icon_redo as IconFn,
                "Redo",
                snapshot.redo_available,
            ),
            (
                ToolbarEvent::UndoAll,
                toolbar_icons::draw_icon_undo_all as IconFn,
                "Undo All",
                snapshot.undo_available,
            ),
            (
                ToolbarEvent::RedoAll,
                toolbar_icons::draw_icon_redo_all as IconFn,
                "Redo All",
                snapshot.redo_available,
            ),
            (
                ToolbarEvent::UndoAllDelayed,
                toolbar_icons::draw_icon_undo_all_delay as IconFn,
                "Undo All Delay",
                snapshot.undo_available,
            ),
            // Row 2
            (
                ToolbarEvent::ClearCanvas,
                toolbar_icons::draw_icon_clear as IconFn,
                "Clear",
                true,
            ),
            (
                ToolbarEvent::ToggleFreeze,
                if snapshot.frozen_active {
                    toolbar_icons::draw_icon_unfreeze as IconFn
                } else {
                    toolbar_icons::draw_icon_freeze as IconFn
                },
                if snapshot.frozen_active {
                    "Unfreeze"
                } else {
                    "Freeze"
                },
                true,
            ),
            (
                ToolbarEvent::OpenConfigurator,
                toolbar_icons::draw_icon_settings as IconFn,
                "Config UI",
                true,
            ),
            (
                ToolbarEvent::OpenConfigFile,
                toolbar_icons::draw_icon_file as IconFn,
                "Config file",
                true,
            ),
            (
                ToolbarEvent::RedoAllDelayed,
                toolbar_icons::draw_icon_redo_all_delay as IconFn,
                "Redo All Delay",
                snapshot.redo_available,
            ),
        ];

        if use_icons {
            // Icon mode: render icon buttons in a grid (5 per row, 2 rows)
            let icon_btn_size = 42.0; // Unified with top toolbar
            let icon_gap = 6.0;
            let icons_per_row = 5;
            let _icon_rows = all_actions.len().div_ceil(icons_per_row);
            let icon_size = 22.0;
            let total_icons_w =
                icons_per_row as f64 * icon_btn_size + (icons_per_row - 1) as f64 * icon_gap;
            let icons_start_x = x + (card_w - 12.0 - total_icons_w) / 2.0;

            for (idx, (evt, icon_fn, label, enabled)) in all_actions.iter().enumerate() {
                let row = idx / icons_per_row;
                let col = idx % icons_per_row;
                let bx = icons_start_x + (icon_btn_size + icon_gap) * col as f64;
                let by = actions_start_y + (icon_btn_size + icon_gap) * row as f64;
                let is_hover = hover
                    .map(|(hx, hy)| point_in_rect(hx, hy, bx, by, icon_btn_size, icon_btn_size))
                    .unwrap_or(false);

                if *enabled {
                    draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, is_hover);
                    set_icon_color(ctx, is_hover);
                } else {
                    draw_button(ctx, bx, by, icon_btn_size, icon_btn_size, false, false);
                    ctx.set_source_rgba(0.5, 0.5, 0.55, 0.5);
                }

                let icon_x = bx + (icon_btn_size - icon_size) / 2.0;
                let icon_y = by + (icon_btn_size - icon_size) / 2.0;
                icon_fn(ctx, icon_x, icon_y, icon_size);

                // Always add hit region for tooltip, even when disabled
                hits.push(HitRegion {
                    rect: (bx, by, icon_btn_size, icon_btn_size),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(*label),
                });
            }
        } else {
            // Text mode: render text buttons in a 2-column grid
            let action_h = 24.0;
            let action_gap = 5.0;
            let action_col_gap = 6.0;
            let action_w = ((width - 2.0 * x) - action_col_gap) / 2.0;

            for (idx, (evt, _icon, label, enabled)) in all_actions.iter().enumerate() {
                let row = idx / 2;
                let col = idx % 2;
                let bx = x + (action_w + action_col_gap) * col as f64;
                let by = actions_start_y + (action_h + action_gap) * row as f64;
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
                        tooltip: None,
                    });
                }
            }
        }
    }

    y += actions_card_h + section_gap;

    // ===== Step Undo/Redo Section =====
    let custom_toggle_h = 24.0;
    let custom_content_h = if snapshot.custom_section_enabled {
        120.0
    } else {
        0.0
    };
    let delay_sliders_h = if snapshot.show_delay_sliders {
        55.0
    } else {
        0.0
    };
    let custom_card_h = 20.0 + custom_toggle_h + custom_content_h + delay_sliders_h;
    draw_group_card(ctx, card_x, y, card_w, custom_card_h);
    draw_section_label(ctx, x, y + 14.0, "Step Undo/Redo");

    // Two checkboxes on one line: "Step controls" and "Delay sliders"
    let custom_toggle_y = y + 22.0;
    let half_w = (card_w - 12.0 - 6.0) / 2.0;

    let step_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, custom_toggle_y, half_w, custom_toggle_h))
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        custom_toggle_y,
        half_w,
        custom_toggle_h,
        snapshot.custom_section_enabled,
        step_hover,
        "Step controls",
    );
    hits.push(HitRegion {
        rect: (x, custom_toggle_y, half_w, custom_toggle_h),
        event: ToolbarEvent::ToggleCustomSection(!snapshot.custom_section_enabled),
        kind: HitKind::Click,
        tooltip: None,
    });

    let delay_hover = hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                x + half_w + 6.0,
                custom_toggle_y,
                half_w,
                custom_toggle_h,
            )
        })
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x + half_w + 6.0,
        custom_toggle_y,
        half_w,
        custom_toggle_h,
        snapshot.show_delay_sliders,
        delay_hover,
        "Delay sliders",
    );
    hits.push(HitRegion {
        rect: (x + half_w + 6.0, custom_toggle_y, half_w, custom_toggle_h),
        event: ToolbarEvent::ToggleDelaySliders(!snapshot.show_delay_sliders),
        kind: HitKind::Click,
        tooltip: None,
    });

    let mut custom_y = custom_toggle_y + custom_toggle_h + 6.0;

    if snapshot.custom_section_enabled {
        let render_custom_row = |ctx: &cairo::Context,
                                 hits: &mut Vec<HitRegion>,
                                 x: f64,
                                 y: f64,
                                 w: f64,
                                 snapshot: &ToolbarSnapshot,
                                 is_undo: bool,
                                 hover: Option<(f64, f64)>| {
            let row_h = 26.0;
            let btn_w = if snapshot.use_icons { 42.0 } else { 90.0 }; // Unified with top toolbar
            let steps_btn_w = 26.0;
            let gap = 6.0;
            let label = if is_undo { "Step Undo" } else { "Step Redo" };
            let steps = if is_undo {
                snapshot.custom_undo_steps
            } else {
                snapshot.custom_redo_steps
            };
            let delay_ms = if is_undo {
                snapshot.custom_undo_delay_ms
            } else {
                snapshot.custom_redo_delay_ms
            };

            let btn_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, row_h))
                .unwrap_or(false);

            if snapshot.use_icons {
                // Icon mode: square button with step icon
                let icon_size = 20.0;
                draw_button(ctx, x, y, btn_w, row_h, false, btn_hover);
                set_icon_color(ctx, btn_hover);
                if is_undo {
                    toolbar_icons::draw_icon_step_undo(
                        ctx,
                        x + (btn_w - icon_size) / 2.0,
                        y + (row_h - icon_size) / 2.0,
                        icon_size,
                    );
                } else {
                    toolbar_icons::draw_icon_step_redo(
                        ctx,
                        x + (btn_w - icon_size) / 2.0,
                        y + (row_h - icon_size) / 2.0,
                        icon_size,
                    );
                }
            } else {
                // Text mode: wider button with label
                draw_button(ctx, x, y, btn_w, row_h, false, btn_hover);
                draw_label_left(ctx, x + 10.0, y, btn_w - 20.0, row_h, label);
            }
            hits.push(HitRegion {
                rect: (x, y, btn_w, row_h),
                event: if is_undo {
                    ToolbarEvent::CustomUndo
                } else {
                    ToolbarEvent::CustomRedo
                },
                kind: HitKind::Click,
                tooltip: if snapshot.use_icons {
                    Some(if is_undo { "Step Undo" } else { "Step Redo" })
                } else {
                    None
                },
            });

            let steps_x = x + btn_w + gap;
            let minus_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, steps_x, y, steps_btn_w, row_h))
                .unwrap_or(false);
            draw_button(ctx, steps_x, y, steps_btn_w, row_h, false, minus_hover);
            set_icon_color(ctx, minus_hover);
            toolbar_icons::draw_icon_minus(
                ctx,
                steps_x + (steps_btn_w - 14.0) / 2.0,
                y + (row_h - 14.0) / 2.0,
                14.0,
            );
            hits.push(HitRegion {
                rect: (steps_x, y, steps_btn_w, row_h),
                event: if is_undo {
                    ToolbarEvent::SetCustomUndoSteps(steps.saturating_sub(1).max(1))
                } else {
                    ToolbarEvent::SetCustomRedoSteps(steps.saturating_sub(1).max(1))
                },
                kind: HitKind::Click,
                tooltip: None,
            });

            let steps_val_x = steps_x + steps_btn_w + 4.0;
            draw_label_center(
                ctx,
                steps_val_x,
                y,
                54.0,
                row_h,
                &format!("{} steps", steps),
            );

            let steps_plus_x = steps_val_x + 58.0;
            let plus_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, steps_plus_x, y, steps_btn_w, row_h))
                .unwrap_or(false);
            draw_button(ctx, steps_plus_x, y, steps_btn_w, row_h, false, plus_hover);
            set_icon_color(ctx, plus_hover);
            toolbar_icons::draw_icon_plus(
                ctx,
                steps_plus_x + (steps_btn_w - 14.0) / 2.0,
                y + (row_h - 14.0) / 2.0,
                14.0,
            );
            hits.push(HitRegion {
                rect: (steps_plus_x, y, steps_btn_w, row_h),
                event: if is_undo {
                    ToolbarEvent::SetCustomUndoSteps(steps.saturating_add(1))
                } else {
                    ToolbarEvent::SetCustomRedoSteps(steps.saturating_add(1))
                },
                kind: HitKind::Click,
                tooltip: None,
            });

            // Delay slider row
            let slider_y = y + row_h + 8.0;
            let slider_h = 6.0;
            let slider_r = 6.0;
            let slider_w = w - 12.0;
            ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
            draw_round_rect(ctx, x, slider_y, slider_w, slider_h, 3.0);
            let _ = ctx.fill();
            let t = delay_t_from_ms(delay_ms);
            let knob_x = x + t * (slider_w - slider_r * 2.0) + slider_r;
            ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
            ctx.arc(
                knob_x,
                slider_y + slider_h / 2.0,
                slider_r,
                0.0,
                std::f64::consts::PI * 2.0,
            );
            let _ = ctx.fill();
            hits.push(HitRegion {
                rect: (x, slider_y - 4.0, slider_w, slider_h + 8.0),
                event: if is_undo {
                    ToolbarEvent::SetCustomUndoDelay(delay_secs_from_t(t))
                } else {
                    ToolbarEvent::SetCustomRedoDelay(delay_secs_from_t(t))
                },
                kind: if is_undo {
                    HitKind::DragCustomUndoDelay
                } else {
                    HitKind::DragCustomRedoDelay
                },
                tooltip: None,
            });

            slider_y + slider_h + 10.0 - y
        };

        let undo_row_h = render_custom_row(ctx, hits, x, custom_y, card_w, snapshot, true, hover);
        custom_y += undo_row_h + 8.0;
        let _redo_row_h = render_custom_row(ctx, hits, x, custom_y, card_w, snapshot, false, hover);
    }

    // Delay sliders for undo-all/redo-all (immediately under step controls)
    if snapshot.show_delay_sliders {
        let sliders_w = card_w - 12.0;
        let slider_h = 6.0;
        let slider_knob_r = 6.0;
        let slider_start_y = y + 20.0 + custom_toggle_h + custom_content_h + 4.0;

        // Undo delay label and slider
        let undo_label = format!(
            "Undo delay: {:.1}s",
            snapshot.undo_all_delay_ms as f64 / 1000.0
        );
        ctx.set_source_rgba(0.7, 0.7, 0.75, 0.9);
        ctx.set_font_size(11.0);
        ctx.move_to(x, slider_start_y + 10.0);
        let _ = ctx.show_text(&undo_label);

        let undo_slider_y = slider_start_y + 16.0;
        ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
        draw_round_rect(ctx, x, undo_slider_y, sliders_w, slider_h, 3.0);
        let _ = ctx.fill();
        let undo_t = delay_t_from_ms(snapshot.undo_all_delay_ms);
        let undo_knob_x = x + undo_t * (sliders_w - slider_knob_r * 2.0) + slider_knob_r;
        ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
        ctx.arc(
            undo_knob_x,
            undo_slider_y + slider_h / 2.0,
            slider_knob_r,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        let _ = ctx.fill();
        hits.push(HitRegion {
            rect: (x, undo_slider_y - 4.0, sliders_w, slider_h + 8.0),
            event: ToolbarEvent::SetUndoDelay(delay_secs_from_t(undo_t)),
            kind: HitKind::DragUndoDelay,
            tooltip: None,
        });

        // Redo delay label and slider
        let redo_label = format!(
            "Redo delay: {:.1}s",
            snapshot.redo_all_delay_ms as f64 / 1000.0
        );
        ctx.set_source_rgba(0.7, 0.7, 0.75, 0.9);
        ctx.move_to(x + sliders_w / 2.0 + 10.0, slider_start_y + 10.0);
        let _ = ctx.show_text(&redo_label);

        let redo_slider_y = slider_start_y + 32.0;
        ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
        draw_round_rect(ctx, x, redo_slider_y, sliders_w, slider_h, 3.0);
        let _ = ctx.fill();
        let redo_t = delay_t_from_ms(snapshot.redo_all_delay_ms);
        let redo_knob_x = x + redo_t * (sliders_w - slider_knob_r * 2.0) + slider_knob_r;
        ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
        ctx.arc(
            redo_knob_x,
            redo_slider_y + slider_h / 2.0,
            slider_knob_r,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        let _ = ctx.fill();
        hits.push(HitRegion {
            rect: (x, redo_slider_y - 4.0, sliders_w, slider_h + 8.0),
            event: ToolbarEvent::SetRedoDelay(delay_secs_from_t(redo_t)),
            kind: HitKind::DragRedoDelay,
            tooltip: None,
        });
    }

    // Draw tooltip for hovered icon button (below for side toolbar)
    draw_tooltip(ctx, hits, hover, width, false);
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

fn draw_checkbox(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    checked: bool,
    hover: bool,
    label: &str,
) {
    let (r, g, b, a) = if hover {
        (0.32, 0.34, 0.4, 0.9)
    } else {
        (0.22, 0.24, 0.28, 0.75)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 4.0);
    let _ = ctx.fill();

    let box_size = h * 0.55;
    let box_x = x + 8.0;
    let box_y = y + (h - box_size) / 2.0;
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    ctx.rectangle(box_x, box_y, box_size, box_size);
    ctx.set_line_width(1.5);
    let _ = ctx.stroke();
    if checked {
        ctx.move_to(box_x + 3.0, box_y + box_size / 2.0);
        ctx.line_to(box_x + box_size / 2.0, box_y + box_size - 3.0);
        ctx.line_to(box_x + box_size - 3.0, box_y + 3.0);
        let _ = ctx.stroke();
    }

    let label_x = box_x + box_size + 8.0;
    draw_label_left(ctx, label_x, y, w - (label_x - x), h, label);
}

/// Draw a compact mini checkbox (used for fill toggle under shape buttons)
fn draw_mini_checkbox(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    checked: bool,
    hover: bool,
    label: &str,
) {
    let (r, g, b, a) = if checked {
        (0.25, 0.5, 0.35, 0.9) // Green tint when checked
    } else if hover {
        (0.32, 0.34, 0.4, 0.85)
    } else {
        (0.2, 0.22, 0.26, 0.7)
    };
    ctx.set_source_rgba(r, g, b, a);
    draw_round_rect(ctx, x, y, w, h, 3.0);
    let _ = ctx.fill();

    // Small checkbox square
    let box_size = h * 0.6;
    let box_x = x + 4.0;
    let box_y = y + (h - box_size) / 2.0;
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.85);
    ctx.rectangle(box_x, box_y, box_size, box_size);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();

    if checked {
        ctx.move_to(box_x + 2.0, box_y + box_size / 2.0);
        ctx.line_to(box_x + box_size / 2.0, box_y + box_size - 2.0);
        ctx.line_to(box_x + box_size - 2.0, box_y + 2.0);
        let _ = ctx.stroke();
    }

    // Centered label
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(10.0);
    if let Ok(ext) = ctx.text_extents(label) {
        let label_x = x + box_size + 8.0 + (w - box_size - 12.0 - ext.width()) / 2.0;
        let label_y = y + (h + ext.height()) / 2.0;
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        ctx.move_to(label_x, label_y);
        let _ = ctx.show_text(label);
    }
}

fn draw_color_picker(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64) {
    let hue_grad = cairo::LinearGradient::new(x, y, x + w, y);
    hue_grad.add_color_stop_rgba(0.0, 1.0, 0.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.17, 1.0, 1.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.33, 0.0, 1.0, 0.0, 1.0);
    hue_grad.add_color_stop_rgba(0.5, 0.0, 1.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(0.66, 0.0, 0.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(0.83, 1.0, 0.0, 1.0, 1.0);
    hue_grad.add_color_stop_rgba(1.0, 1.0, 0.0, 0.0, 1.0);

    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&hue_grad);
    let _ = ctx.fill();

    let val_grad = cairo::LinearGradient::new(x, y, x, y + h);
    val_grad.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.0);
    val_grad.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.65);
    ctx.rectangle(x, y, w, h);
    let _ = ctx.set_source(&val_grad);
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.4);
    ctx.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> Color {
    let h = (h - h.floor()).clamp(0.0, 1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color { r, g, b, a: 1.0 }
}

fn draw_round_rect(ctx: &cairo::Context, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    let r = radius.min(w / 2.0).min(h / 2.0);
    ctx.new_sub_path();
    ctx.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    ctx.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    ctx.arc(
        x + r,
        y + h - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    ctx.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        std::f64::consts::PI * 1.5,
    );
    ctx.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_and_side_close_independently() {
        let mut mgr = ToolbarSurfaceManager {
            visible: true,
            top_visible: true,
            side_visible: true,
            top: ToolbarSurface::new("test-top", Anchor::TOP, (0, 0, 0, 0)),
            side: ToolbarSurface::new("test-side", Anchor::LEFT, (0, 0, 0, 0)),
            top_hover: None,
            side_hover: None,
        };

        // Close top only -> side stays visible, any-visible stays true
        mgr.set_top_visible(false);
        assert!(!mgr.is_top_visible());
        assert!(mgr.is_side_visible());
        assert!(mgr.is_visible());

        // Close side only -> top stays visible, any-visible stays true
        mgr.set_side_visible(false);
        mgr.set_top_visible(true);
        assert!(mgr.is_top_visible());
        assert!(!mgr.is_side_visible());
        assert!(mgr.is_visible());

        // Close both -> everything hidden
        mgr.set_top_visible(false);
        mgr.set_side_visible(false);
        assert!(!mgr.is_top_visible());
        assert!(!mgr.is_side_visible());
        assert!(!mgr.is_visible());
    }

    #[test]
    fn hsv_to_rgb_matches_primary_points() {
        let red = hsv_to_rgb(0.0, 1.0, 1.0);
        assert!((red.r - 1.0).abs() < 1e-6 && red.g.abs() < 1e-6 && red.b.abs() < 1e-6);

        let green = hsv_to_rgb(1.0 / 3.0, 1.0, 1.0);
        assert!((green.g - 1.0).abs() < 1e-6 && green.r.abs() < 1e-6 && green.b.abs() < 1e-6);

        let blue = hsv_to_rgb(2.0 / 3.0, 1.0, 1.0);
        assert!((blue.b - 1.0).abs() < 1e-6 && blue.r.abs() < 1e-6 && blue.g.abs() < 1e-6);
    }

    #[test]
    fn thickness_drag_maps_to_expected_value() {
        let mut surface = ToolbarSurface::new("test", Anchor::TOP, (0, 0, 0, 0));
        surface.hit_regions.push(HitRegion {
            rect: (0.0, 0.0, 100.0, 10.0),
            event: ToolbarEvent::SetThickness(1.0),
            kind: HitKind::DragSetThickness {
                min: 1.0,
                max: 50.0,
            },
            tooltip: None,
        });

        let (evt, drag) = surface.hit_at(50.0, 5.0).expect("hit expected");
        assert!(drag, "drag flag should be true for thickness slider");
        match evt {
            ToolbarEvent::SetThickness(value) => {
                assert!((value - 25.5).abs() < 0.01, "expected mid-range thickness");
            }
            other => panic!("unexpected event from drag: {:?}", other),
        }
    }

    #[test]
    fn thickness_drag_respects_custom_range() {
        let mut surface = ToolbarSurface::new("test", Anchor::TOP, (0, 0, 0, 0));
        surface.hit_regions.push(HitRegion {
            rect: (0.0, 0.0, 200.0, 10.0),
            event: ToolbarEvent::SetThickness(0.05),
            kind: HitKind::DragSetThickness {
                min: 0.05,
                max: 0.9,
            },
            tooltip: None,
        });

        let (evt, drag) = surface.hit_at(100.0, 5.0).expect("hit expected");
        assert!(drag, "drag flag should be true for thickness slider");
        match evt {
            ToolbarEvent::SetThickness(value) => {
                assert!((value - 0.475).abs() < 0.01, "expected mid-range opacity");
            }
            other => panic!("unexpected event from drag: {:?}", other),
        }
    }

    #[test]
    fn color_picker_drag_emits_color_event() {
        let mut surface = ToolbarSurface::new("test", Anchor::TOP, (0, 0, 0, 0));
        surface.hit_regions.push(HitRegion {
            rect: (0.0, 0.0, 100.0, 100.0),
            event: ToolbarEvent::SetColor(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
            kind: HitKind::PickColor {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
            tooltip: None,
        });

        let (evt, drag) = surface.hit_at(50.0, 50.0).expect("hit expected");
        assert!(drag, "color picker should initiate drag");
        match evt {
            ToolbarEvent::SetColor(c) => {
                // Hue at midpoint with value 0.5 should yield equal green/blue components
                assert!((c.g - 0.5).abs() < 0.05 && (c.b - 0.5).abs() < 0.05);
            }
            other => panic!("unexpected event from color picker: {:?}", other),
        }
    }
}
