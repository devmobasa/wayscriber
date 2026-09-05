mod background;
mod context;
mod overlays;
mod text;

use super::super::*;
pub(super) use context::CanvasRenderCtx;

const SPOTLIGHT_MAGNIFIER_TOAST_SOURCE: &str = "spotlight-magnifier";

impl WaylandState {
    /// Answers one user action that asked for magnification the current
    /// surface cannot supply.
    ///
    /// Deliberately not deduped against the render loop's flag: the request is
    /// already coalesced to one per drained batch of input events, and the
    /// spec asks for a warning per *action*. Sharing the render flag would
    /// silence every action after the first for as long as the source stayed
    /// unavailable — which, on a transparent board, is the whole session.
    pub(in crate::backend::wayland) fn show_spotlight_magnifier_feedback_if_unavailable(&mut self) {
        let source = self.current_spotlight_magnifier_source();
        if source.is_complete() {
            return;
        }
        self.push_spotlight_magnifier_toast(
            "Freeze the screen to preview Spotlight magnification.",
        );
        // This toast already says what the standing page warning would, so
        // adopt its dedup key: the next frame must not repeat it.
        self.spotlight.note_page_source(source, true, true);
    }

    /// Warns once for magnified Spotlights the user arrived at rather than
    /// made — a page switch, a board switch, a restored session, an undo.
    ///
    /// Those have no originating action to hang a warning on, so the state is
    /// noticed here instead. The dedup key is the availability itself, so the
    /// warning repeats only once that changes, and a frame that cannot show
    /// transients does not record one the user never saw.
    fn warn_once_for_arrived_unavailable_magnification(
        &mut self,
        has_magnified_region: bool,
        source: crate::draw::SpotlightMagnifierSource,
        show_toast: bool,
    ) {
        if self
            .spotlight
            .note_page_source(source, has_magnified_region, show_toast)
        {
            self.push_spotlight_magnifier_toast(
                "This page has magnified Spotlights. Freeze the screen to preview them.",
            );
        }
    }

    /// Emits one warning per continuous run of failing frames.
    ///
    /// The dedup flag is armed only when a toast was actually shown. Frames
    /// that suppress transients — every frame while the capture picker is
    /// open — must not arm it, or the next real warning would be swallowed as
    /// a duplicate of a toast the user never saw.
    fn push_spotlight_magnifier_warning(&mut self, message: &str, show_toast: bool) {
        if self.spotlight.render_warning_due(show_toast) {
            self.push_spotlight_magnifier_toast(message);
        }
    }

    fn push_spotlight_magnifier_toast(&mut self, message: &str) {
        self.input_state.push_toast(
            crate::input::state::ToastPriority::Critical,
            SPOTLIGHT_MAGNIFIER_TOAST_SOURCE,
            crate::input::state::Toast::warning(message),
        );
    }

    pub(super) fn render_canvas_layer(
        &mut self,
        canvas: &CanvasRenderCtx<'_>,
        mut perf: Option<&mut PerfRenderBreakdown>,
    ) -> Result<()> {
        let ctx = canvas.cairo;
        let width = canvas.geometry.width;
        let height = canvas.geometry.height;
        let scale = canvas.geometry.scale;
        let phys_width = canvas.geometry.physical_width;
        let phys_height = canvas.geometry.physical_height;
        let now = canvas.now;
        let damage_world = canvas.damage_world;
        let render_transients = canvas.canvas.render_transients;
        let canvas_transform_active = canvas.canvas.transform_active;
        let (canvas_origin_x, canvas_origin_y) = canvas.canvas.origin;
        let shapes_total = self.input_state.boards.active_frame().shapes.len();
        let text_halo_enabled = canvas.canvas.text_halo_enabled;

        // For pure pan transforms, serve the board background and committed
        // shapes from the baked layer cache: pan frames force full damage, so
        // this turns an O(shapes) Cairo replay into a single aligned blit.
        let layer_cache_start = perf.as_ref().map(|_| Instant::now());
        let layer_cache_ready = if canvas.canvas.layer_cache_eligible {
            self.ensure_canvas_layer_cache(width, height, scale)
        } else {
            self.render.canvas_layer_cache_mut().clear();
            false
        };
        if let (Some(perf), Some(layer_cache_start)) = (perf.as_mut(), layer_cache_start) {
            perf.stages.completed_shapes = perf
                .stages
                .completed_shapes
                .saturating_add(Instant::now().saturating_duration_since(layer_cache_start));
        }

        let background_start = perf.as_ref().map(|_| Instant::now());
        let eraser_ctx = self.render_canvas_background(ctx, scale, phys_width, phys_height)?;
        if let (Some(perf), Some(background_start)) = (perf.as_mut(), background_start) {
            perf.stages.background = perf
                .stages
                .background
                .saturating_add(Instant::now().saturating_duration_since(background_start));
        }

        // A capture picker always selects against the frozen desktop. When its
        // captured intent includes drawings, replay committed annotations over
        // that backdrop so the preview matches the exported PNG. The layer
        // cache stays disabled because its baked board background is not the
        // frozen capture. Turning the Review toggle off keeps only the raw
        // backdrop. Transient handles, provisional strokes, text previews,
        // hover effects, and click highlights remain suppressed in both cases.
        if !canvas.canvas.draw_committed {
            self.spotlight.note_frame(false);
            if let Some(perf) = perf.as_mut() {
                perf.shapes_total = shapes_total;
                perf.shapes_tested = 0;
                perf.shapes_rendered = 0;
                perf.canvas_layer_cache_used = false;
            }
            return Ok(());
        }

        // Scale subsequent drawing to logical coordinates
        let _ = ctx.save();
        if scale > 1 {
            ctx.scale(scale as f64, scale as f64);
        }

        if canvas_transform_active {
            let _ = ctx.save();
            if let Some(zoom_scale) = canvas.canvas.zoom_scale {
                ctx.scale(zoom_scale, zoom_scale);
            }
            ctx.translate(-canvas_origin_x, -canvas_origin_y);
        }

        let replay_ctx = eraser_ctx.replay_context();

        let completed_shapes_start = perf.as_ref().map(|_| Instant::now());
        let (layer_cache, draw_caches, measurer) = self.render.canvas_draw_parts_mut();
        render_committed_canvas_shapes(
            measurer,
            &self.input_state.boards.active_frame().shapes,
            layer_cache,
            draw_caches,
            canvas,
            layer_cache_ready,
            &replay_ctx,
            perf.as_deref_mut(),
        );
        if let (Some(perf), Some(completed_shapes_start)) = (perf.as_mut(), completed_shapes_start)
        {
            perf.stages.completed_shapes = perf
                .stages
                .completed_shapes
                .saturating_add(Instant::now().saturating_duration_since(completed_shapes_start));
        }

        // Spotlights dim everything around themselves, so they cannot be drawn in
        // z-order like other shapes: one pass covering every region at once.
        //
        // It has to run *after* the committed shapes, not before them. Eraser
        // strokes clear their path and replay the original backdrop into it, so a
        // dim layer painted earlier would be punched away and every past erasure
        // would show as a bright trail outside the openings.
        //
        // One collection serves the dim pass, the magnifier pass, and the
        // arrival warning. The warning reads `committed_magnified` rather than
        // the region list, because a drag still under the pointer describes
        // nothing the page holds: cancelling it leaves nothing behind, and
        // completing it warns through its own action instead.
        let spotlight_cursor = render_transients.then(|| {
            let (screen_x, screen_y) = self.pointer.position();
            self.canvas_world_coords(screen_x as f64, screen_y as f64)
        });
        let crate::input::state::SpotlightFrameRegions {
            regions: spotlight_regions,
            committed_magnified,
        } = self.input_state.spotlight_frame_regions(spotlight_cursor);
        let magnifier_source = eraser_ctx.magnifier_source();
        self.warn_once_for_arrived_unavailable_magnification(
            committed_magnified,
            magnifier_source,
            render_transients,
        );
        match crate::draw::render_spotlight_magnification_pass(
            ctx,
            &spotlight_regions,
            self.input_state.style.spotlight_feather,
            magnifier_source,
            Some((phys_width, phys_height)),
            self.spotlight.scratch_mut(),
        ) {
            // A missing pixel source is a standing condition, not an event:
            // the toolbar carries the inline unavailable state, and the one
            // warning toast belongs to the user action that asked for
            // magnification (see `show_spotlight_magnifier_feedback_if_unavailable`).
            // Warning again from the render loop would fire on every frame.
            //
            // It also ends any run of render failures: the flag below tracks
            // failing *renders*, and a frame that never attempted one must not
            // leave it armed to swallow the next real failure.
            Ok(crate::draw::SpotlightMagnifierOutcome::SourceUnavailable) => {
                self.spotlight.clear_render_warning();
            }
            Ok(crate::draw::SpotlightMagnifierOutcome::AllocationFailed) => {
                self.push_spotlight_magnifier_warning(
                    "Spotlight magnification could not allocate its render buffer.",
                    render_transients,
                );
            }
            Err(error) => {
                log::warn!("Spotlight magnifier render failed: {error}");
                self.push_spotlight_magnifier_warning(
                    "Spotlight magnification could not be rendered.",
                    render_transients,
                );
            }
            Ok(crate::draw::SpotlightMagnifierOutcome::Rendered(metrics)) => {
                if let Some(perf) = perf.as_mut() {
                    perf.stages.spotlight_snapshot = perf
                        .stages
                        .spotlight_snapshot
                        .saturating_add(metrics.snapshot_time);
                    perf.stages.spotlight_paint = perf
                        .stages
                        .spotlight_paint
                        .saturating_add(metrics.paint_time);
                    perf.spotlight_regions = metrics.regions;
                    perf.spotlight_copied_pixels = metrics.copied_pixels;
                    perf.spotlight_strategy = Some(metrics.strategy);
                }
                self.spotlight.clear_render_warning();
            }
            Ok(crate::draw::SpotlightMagnifierOutcome::NotNeeded) => {
                self.spotlight.clear_render_warning();
            }
        }
        // Remember for the next frame's damage decision: once the last spotlight
        // is gone this buffer still carries its dim until a full repaint.
        self.spotlight.note_frame(!spotlight_regions.is_empty());
        crate::draw::render_spotlight_pass(
            ctx,
            &spotlight_regions,
            crate::draw::SpotlightPass {
                dim_opacity: self.input_state.style.spotlight_dim_opacity,
                feather: self.input_state.style.spotlight_feather,
            },
        );

        if !render_transients {
            if canvas_transform_active {
                let _ = ctx.restore();
            }
            let _ = ctx.restore();
            return Ok(());
        }

        self.render_selection_overlays(ctx);

        let (mx, my) = self.canvas_world_coords(
            self.pointer.position().0 as f64,
            self.pointer.position().1 as f64,
        );
        let (hover_mx, hover_my) = self
            .stylus_hover_cursor_position()
            .map(|(x, y)| self.canvas_world_coords(x, y))
            .unwrap_or((mx, my));

        self.render_eraser_hover_halos(ctx, hover_mx, hover_my);

        let provisional = self.input_state.provisional_tool_stroke(mx, my);
        let provisional_points = provisional_point_count(&provisional);
        let provisional_start = perf.as_ref().map(|_| Instant::now());
        let (caches, measurer) = self.render.draw_text_parts_mut();
        let mut render = crate::draw::RenderCtx::new(ctx, caches);
        let rendered_provisional = match provisional {
            crate::input::tool::ProvisionalToolStroke::BlurReplayPreview(params) => {
                render.render_blur_rect(params, &replay_ctx);
                true
            }
            _ => self.input_state.render_provisional_shape_for_damage(
                measurer,
                &mut render,
                mx,
                my,
                damage_world,
                text_halo_enabled,
            ),
        };
        if let (Some(perf), Some(provisional_start)) = (perf.as_mut(), provisional_start) {
            perf.provisional_points = provisional_points;
            perf.stages.provisional = perf
                .stages
                .provisional
                .saturating_add(Instant::now().saturating_duration_since(provisional_start));
        }
        if rendered_provisional {
            debug!("Rendered provisional shape");
        }

        // Render text cursor/buffer if in text mode
        self.render_text_input_preview(canvas);

        self.input_state.render_highlight_tool_ring(ctx, mx, my);

        // Render click highlight overlays before UI so status/help remain legible
        self.input_state.render_click_highlights(ctx, now);

        if canvas_transform_active {
            let _ = ctx.restore();
        }

        let _ = ctx.restore();

        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn render_committed_canvas_shapes(
    measurer: &crate::draw::TextMeasurer,
    shapes: &[crate::draw::DrawnShape],
    layer_cache: &super::super::canvas_layer::CanvasLayerCache,
    draw_caches: &mut crate::draw::RenderCaches,
    canvas: &CanvasRenderCtx<'_>,
    layer_cache_ready: bool,
    replay_ctx: &crate::draw::EraserReplayContext<'_>,
    mut perf: Option<&mut PerfRenderBreakdown>,
) {
    let ctx = canvas.cairo;
    let width = canvas.geometry.width;
    let height = canvas.geometry.height;
    let damage_world = canvas.damage_world;
    let canvas_transform_active = canvas.canvas.transform_active;
    let text_halo_enabled = canvas.canvas.text_halo_enabled;
    if layer_cache_ready && layer_cache.blit(ctx) {
        debug!("Rendered committed shapes from layer cache");
        if let Some(perf) = perf.as_mut() {
            perf.shapes_total = shapes.len();
            perf.canvas_layer_cache_used = true;
        }
        return;
    }
    debug!("Rendering {} completed shapes", shapes.len());
    if let Some(perf) = perf.as_mut() {
        perf.shapes_total = shapes.len();
    }
    let mut render = crate::draw::RenderCtx {
        cairo: ctx,
        caches: draw_caches,
    };
    let mut render_shape = |shape: &crate::draw::DrawnShape| {
        super::super::canvas_layer::render_committed_shape(
            measurer,
            &mut render,
            shape,
            replay_ctx,
            text_halo_enabled,
        )
    };
    let Some(bounds) = union_damage_bounds(damage_world) else {
        for shape in shapes {
            render_shape(shape);
        }
        if let Some(perf) = perf.as_mut() {
            perf.shapes_tested = shapes.len();
            perf.shapes_rendered = shapes.len();
        }
        return;
    };
    let Some(safe_bounds) =
        safe_shape_damage_bounds(bounds, width, height, canvas_transform_active)
    else {
        return;
    };
    let mut shapes_rendered = 0usize;
    for shape in shapes {
        if shape
            .bounding_box_with(measurer)
            .is_some_and(|bounds| rects_intersect(bounds, safe_bounds))
        {
            render_shape(shape);
            shapes_rendered += 1;
        }
    }
    if let Some(perf) = perf.as_mut() {
        perf.shapes_tested = shapes.len();
        perf.shapes_rendered = shapes_rendered;
    }
}

fn union_damage_bounds(regions: &[crate::util::Rect]) -> Option<crate::util::Rect> {
    regions.iter().copied().reduce(|union, region| {
        let min_x = union.x.min(region.x);
        let min_y = union.y.min(region.y);
        let max_x = union
            .x
            .saturating_add(union.width)
            .max(region.x.saturating_add(region.width));
        let max_y = union
            .y
            .saturating_add(union.height)
            .max(region.y.saturating_add(region.height));
        crate::util::Rect {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    })
}

fn safe_shape_damage_bounds(
    bounds: crate::util::Rect,
    width: u32,
    height: u32,
    canvas_transform_active: bool,
) -> Option<crate::util::Rect> {
    let margin = 2;
    let x = bounds.x.saturating_sub(margin);
    let y = bounds.y.saturating_sub(margin);
    let width_with_margin = bounds.width.saturating_add(margin * 2);
    let height_with_margin = bounds.height.saturating_add(margin * 2);
    if canvas_transform_active {
        return crate::util::Rect::new(x, y, width_with_margin, height_with_margin);
    }
    let x = x.max(0);
    let y = y.max(0);
    crate::util::Rect::new(
        x,
        y,
        width_with_margin.min((width as i32).saturating_sub(x)),
        height_with_margin.min((height as i32).saturating_sub(y)),
    )
}

fn rects_intersect(a: crate::util::Rect, b: crate::util::Rect) -> bool {
    let a_right = a.x.saturating_add(a.width);
    let a_bottom = a.y.saturating_add(a.height);
    let b_right = b.x.saturating_add(b.width);
    let b_bottom = b.y.saturating_add(b.height);
    !(a.x >= b_right || a_right <= b.x || a.y >= b_bottom || a_bottom <= b.y)
}

fn provisional_point_count(stroke: &crate::input::tool::ProvisionalToolStroke<'_>) -> usize {
    match stroke {
        crate::input::tool::ProvisionalToolStroke::BorrowedFreehand { points, .. }
        | crate::input::tool::ProvisionalToolStroke::BorrowedPressureFreehand { points, .. }
        | crate::input::tool::ProvisionalToolStroke::BorrowedMarker { points, .. }
        | crate::input::tool::ProvisionalToolStroke::EraserPreview { points, .. } => points.len(),
        crate::input::tool::ProvisionalToolStroke::Shape(_)
        | crate::input::tool::ProvisionalToolStroke::BlurReplayPreview(_)
        | crate::input::tool::ProvisionalToolStroke::None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{rects_intersect, safe_shape_damage_bounds, union_damage_bounds};
    use crate::util::Rect;

    #[test]
    fn damage_union_covers_every_region() {
        assert_eq!(
            union_damage_bounds(&[
                Rect::new(10, 20, 30, 40).unwrap(),
                Rect::new(-5, 50, 20, 10).unwrap(),
            ]),
            Rect::new(-5, 20, 45, 40)
        );
    }

    #[test]
    fn shape_damage_margin_clamps_only_in_screen_space() {
        let bounds = Rect::new(0, 0, 10, 10).unwrap();
        assert_eq!(
            safe_shape_damage_bounds(bounds, 100, 100, false),
            Rect::new(0, 0, 14, 14)
        );
        assert_eq!(
            safe_shape_damage_bounds(bounds, 100, 100, true),
            Rect::new(-2, -2, 14, 14)
        );
    }

    #[test]
    fn edge_touching_shape_is_outside_damage() {
        let damage = Rect::new(10, 10, 20, 20).unwrap();
        assert!(!rects_intersect(Rect::new(0, 10, 10, 5).unwrap(), damage));
        assert!(rects_intersect(Rect::new(9, 10, 2, 5).unwrap(), damage));
    }
}

#[cfg(test)]
mod resource_tests;
