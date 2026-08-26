mod background;
mod overlays;
mod text;

use super::super::*;

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
        self.spotlight_magnifier_page_warned_source = Some(source);
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
        match arrived_magnification_warning(
            self.spotlight_magnifier_page_warned_source,
            source,
            has_magnified_region,
            show_toast,
        ) {
            ArrivedMagnificationWarning::Clear => {
                self.spotlight_magnifier_page_warned_source = None;
            }
            ArrivedMagnificationWarning::Skip => {}
            ArrivedMagnificationWarning::Warn => {
                self.push_spotlight_magnifier_toast(
                    "This page has magnified Spotlights. Freeze the screen to preview them.",
                );
                self.spotlight_magnifier_page_warned_source = Some(source);
            }
        }
    }

    /// Emits one warning per continuous run of failing frames.
    ///
    /// The dedup flag is armed only when a toast was actually shown. Frames
    /// that suppress transients — every frame while the capture picker is
    /// open — must not arm it, or the next real warning would be swallowed as
    /// a duplicate of a toast the user never saw.
    fn push_spotlight_magnifier_warning(&mut self, message: &str, show_toast: bool) {
        if !spotlight_magnifier_warning_is_due(self.spotlight_magnifier_warning_active, show_toast)
        {
            return;
        }
        self.push_spotlight_magnifier_toast(message);
        self.spotlight_magnifier_warning_active = true;
    }

    fn push_spotlight_magnifier_toast(&mut self, message: &str) {
        self.input_state.push_toast(
            crate::input::state::ToastPriority::Critical,
            SPOTLIGHT_MAGNIFIER_TOAST_SOURCE,
            crate::input::state::Toast::warning(message),
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_canvas_layer(
        &mut self,
        ctx: &cairo::Context,
        width: u32,
        height: u32,
        scale: i32,
        phys_width: u32,
        phys_height: u32,
        now: Instant,
        damage_world: &[crate::util::Rect],
        render_transients: bool,
        mut perf: Option<&mut PerfRenderBreakdown>,
    ) -> Result<()> {
        let capture_picker_active = self.capture_picker_chrome_suppressed();
        let capture_picker_draws_committed = capture_picker_draws_committed(
            capture_picker_active,
            self.region_picker_include_drawings(),
        );
        let render_transients = render_transients && !capture_picker_active;
        let canvas_transform_active = self.canvas_transform_active();
        let (canvas_origin_x, canvas_origin_y) = self.canvas_view_origin();
        let shapes_total = self.input_state.boards.active_frame().shapes.len();
        let text_halo_enabled = self.config.drawing.text_halo_enabled;

        // For pure pan transforms, serve the board background and committed
        // shapes from the baked layer cache: pan frames force full damage, so
        // this turns an O(shapes) Cairo replay into a single aligned blit.
        let layer_cache_start = perf.as_ref().map(|_| Instant::now());
        let layer_cache_ready = if !capture_picker_active && self.canvas_layer_cache_usable() {
            self.ensure_canvas_layer_cache(width, height, scale)
        } else {
            self.canvas_layer_cache.clear();
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
        if !capture_picker_draws_committed {
            self.spotlight_dimmed_last_frame = false;
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
            if self.zoom.active {
                ctx.scale(self.zoom.scale, self.zoom.scale);
            }
            ctx.translate(-canvas_origin_x, -canvas_origin_y);
        }

        let replay_ctx = eraser_ctx.replay_context();

        let completed_shapes_start = perf.as_ref().map(|_| Instant::now());
        if layer_cache_ready && self.canvas_layer_cache.blit(ctx) {
            // Board background and committed shapes came from the baked layer.
            debug!("Rendered committed shapes from layer cache");
            if let Some(perf) = perf.as_mut() {
                perf.shapes_total = shapes_total;
                perf.canvas_layer_cache_used = true;
            }
        } else {
            // Render all completed shapes from active frame
            debug!("Rendering {} completed shapes", shapes_total);
            let shapes = &self.input_state.boards.active_frame().shapes;
            if let Some(perf) = perf.as_mut() {
                perf.shapes_total = shapes.len();
            }

            // Manual Culling: Only render shapes that intersect with the damage regions.
            // Cairo's internal clipping is efficient for rasterization, but sending
            // thousands of shapes to Cairo still incurs overhead for geometry processing.
            // A simple bounding box check here eliminates that overhead.
            let render_drawn_shape = |drawn_shape: &crate::draw::DrawnShape| {
                super::super::canvas_layer::render_committed_shape(
                    ctx,
                    drawn_shape,
                    &replay_ctx,
                    text_halo_enabled,
                )
            };

            // Compute bounding box of all damage regions for fast rejection
            // (Union of all dirty rects). These bounds are in world coordinates.
            let damage_bounds =
                damage_world
                    .iter()
                    .fold(None, |acc: Option<crate::util::Rect>, r| match acc {
                        None => Some(*r),
                        Some(u) => {
                            // Manual union to avoid extra allocations.
                            let min_x = u.x.min(r.x);
                            let min_y = u.y.min(r.y);
                            let max_x =
                                u.x.saturating_add(u.width).max(r.x.saturating_add(r.width));
                            let max_y =
                                u.y.saturating_add(u.height)
                                    .max(r.y.saturating_add(r.height));
                            Some(crate::util::Rect {
                                x: min_x,
                                y: min_y,
                                width: max_x - min_x,
                                height: max_y - min_y,
                            })
                        }
                    });

            if let Some(bounds) = damage_bounds {
                // Expand bounds slightly to account for line width/glow that might extend outside
                // the logical shape bounds (though Shape::bounding_box should theoretically cover it,
                // safety margin is good).
                let margin = 2;
                let safe_x = bounds.x.saturating_sub(margin);
                let safe_y = bounds.y.saturating_sub(margin);
                let safe_width = bounds.width.saturating_add(margin * 2);
                let safe_height = bounds.height.saturating_add(margin * 2);
                let safe_bounds = if canvas_transform_active {
                    crate::util::Rect::new(safe_x, safe_y, safe_width, safe_height)
                } else {
                    // Clamp to logical surface bounds to avoid negative coords or overflow.
                    let logical_width = width as i32;
                    let logical_height = height as i32;
                    let clamped_x = safe_x.max(0);
                    let clamped_y = safe_y.max(0);
                    let max_width = logical_width.saturating_sub(clamped_x);
                    let max_height = logical_height.saturating_sub(clamped_y);
                    crate::util::Rect::new(
                        clamped_x,
                        clamped_y,
                        safe_width.min(max_width),
                        safe_height.min(max_height),
                    )
                };

                if let Some(safe_bounds) = safe_bounds {
                    let mut shapes_tested = 0usize;
                    let mut shapes_rendered = 0usize;
                    for drawn_shape in shapes {
                        shapes_tested += 1;
                        // If shape has no bounding box (e.g. empty freehand), skip it.
                        // If it has one, check intersection. Uses the per-shape
                        // memoized bounds to avoid O(points) recomputation per frame.
                        if let Some(bbox) = drawn_shape.bounding_box() {
                            // Check intersection:
                            // !(bbox.left > safe.right || bbox.right < safe.left || ...)
                            let bbox_right = bbox.x.saturating_add(bbox.width);
                            let bbox_bottom = bbox.y.saturating_add(bbox.height);
                            let safe_right = safe_bounds.x.saturating_add(safe_bounds.width);
                            let safe_bottom = safe_bounds.y.saturating_add(safe_bounds.height);

                            let intersects = !(bbox.x >= safe_right
                                || bbox_right <= safe_bounds.x
                                || bbox.y >= safe_bottom
                                || bbox_bottom <= safe_bounds.y);

                            if intersects {
                                render_drawn_shape(drawn_shape);
                                shapes_rendered += 1;
                            }
                        }
                    }
                    if let Some(perf) = perf.as_mut() {
                        perf.shapes_tested = shapes_tested;
                        perf.shapes_rendered = shapes_rendered;
                    }
                }
            } else {
                // If we don't have damage bounds, render everything to stay correct.
                let mut shapes_rendered = 0usize;
                for drawn_shape in shapes {
                    render_drawn_shape(drawn_shape);
                    shapes_rendered += 1;
                }
                if let Some(perf) = perf.as_mut() {
                    perf.shapes_tested = shapes.len();
                    perf.shapes_rendered = shapes_rendered;
                }
            }
        }
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
            let (screen_x, screen_y) = self.current_mouse();
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
            self.input_state.spotlight_feather,
            magnifier_source,
            Some((phys_width, phys_height)),
            &mut self.spotlight_magnifier_scratch,
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
                self.spotlight_magnifier_warning_active = false;
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
                self.spotlight_magnifier_warning_active = false;
            }
            Ok(crate::draw::SpotlightMagnifierOutcome::NotNeeded) => {
                self.spotlight_magnifier_warning_active = false;
            }
        }
        // Remember for the next frame's damage decision: once the last spotlight
        // is gone this buffer still carries its dim until a full repaint.
        self.spotlight_dimmed_last_frame = !spotlight_regions.is_empty();
        crate::draw::render_spotlight_pass(
            ctx,
            &spotlight_regions,
            crate::draw::SpotlightPass {
                dim_opacity: self.input_state.spotlight_dim_opacity,
                feather: self.input_state.spotlight_feather,
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

        let (mx, my) =
            self.canvas_world_coords(self.current_mouse().0 as f64, self.current_mouse().1 as f64);
        let (hover_mx, hover_my) = self
            .stylus_hover_cursor_position()
            .map(|(x, y)| self.canvas_world_coords(x, y))
            .unwrap_or((mx, my));

        self.render_eraser_hover_halos(ctx, hover_mx, hover_my);

        let provisional = self.input_state.provisional_tool_stroke(mx, my);
        let provisional_points = provisional_point_count(&provisional);
        let provisional_start = perf.as_ref().map(|_| Instant::now());
        let rendered_provisional = match provisional {
            crate::input::tool::ProvisionalToolStroke::BlurReplayPreview(params) => {
                crate::draw::render_blur_rect(ctx, params, &replay_ctx);
                true
            }
            _ => self.input_state.render_provisional_shape_for_damage(
                ctx,
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
        self.render_text_input_preview(ctx);

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

/// What to do about magnified Spotlights the user arrived at rather than made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArrivedMagnificationWarning {
    /// Availability changed for the better: forget the warning already shown,
    /// so losing the source again is heard.
    Clear,
    /// Already warned for this availability, nothing magnified here, or this
    /// frame cannot show toasts.
    Skip,
    /// Show the warning and remember the availability it was shown for.
    Warn,
}

/// Decides the arrival warning from the four facts it depends on.
///
/// Keyed on availability and nothing else: the spec asks for at most one
/// deduplicated warning *until availability changes*, so walking through
/// several unavailable pages is one warning, and freezing then unfreezing
/// earns a new one.
///
/// Only a change in availability releases the memory. A page that happens to
/// hold no magnified Spotlight is not such a change, so passing through one on
/// the way back to an unavailable page must not re-arm the warning.
fn arrived_magnification_warning(
    already_warned_for: Option<crate::draw::SpotlightMagnifierSource>,
    source: crate::draw::SpotlightMagnifierSource,
    has_magnified_region: bool,
    show_toast: bool,
) -> ArrivedMagnificationWarning {
    if source.is_complete() {
        return ArrivedMagnificationWarning::Clear;
    }
    if already_warned_for == Some(source) || !has_magnified_region {
        return ArrivedMagnificationWarning::Skip;
    }
    // A suppressed frame skips without recording: the warning is still owed
    // once the user can actually see it.
    if !show_toast {
        return ArrivedMagnificationWarning::Skip;
    }
    ArrivedMagnificationWarning::Warn
}

/// Whether a Spotlight magnifier warning should be emitted right now.
///
/// A frame that cannot show transients cannot show a toast either, and it must
/// not count as "already warned": every frame with the capture picker open is
/// such a frame, and arming the flag there would swallow the next real warning
/// as a duplicate of a toast nobody saw.
const fn spotlight_magnifier_warning_is_due(already_warned: bool, show_toast: bool) -> bool {
    show_toast && !already_warned
}

const fn capture_picker_draws_committed(
    capture_picker_active: bool,
    include_drawings: bool,
) -> bool {
    !capture_picker_active || include_drawings
}

#[cfg(test)]
mod tests {
    use super::{
        ArrivedMagnificationWarning, arrived_magnification_warning, capture_picker_draws_committed,
        spotlight_magnifier_warning_is_due,
    };
    use crate::draw::SpotlightMagnifierSource;

    #[test]
    fn arriving_at_a_page_of_unavailable_loupes_warns_once_not_every_frame() {
        let unavailable = SpotlightMagnifierSource::IncompleteTransparent;

        // The page switch lands: nothing has been warned for yet.
        assert_eq!(
            arrived_magnification_warning(None, unavailable, true, true),
            ArrivedMagnificationWarning::Warn
        );
        // Every following frame draws the same unavailable page in silence.
        assert_eq!(
            arrived_magnification_warning(Some(unavailable), unavailable, true, true),
            ArrivedMagnificationWarning::Skip
        );
    }

    #[test]
    fn only_a_change_in_availability_releases_the_warning() {
        let unavailable = SpotlightMagnifierSource::IncompleteTransparent;

        // Gaining a source is the change the spec keys on: losing it again
        // must be heard.
        assert_eq!(
            arrived_magnification_warning(
                Some(unavailable),
                SpotlightMagnifierSource::CompleteSolid,
                true,
                true
            ),
            ArrivedMagnificationWarning::Clear
        );

        // Passing through a page with nothing magnified is not such a change,
        // so the memory survives and returning to the unavailable page is
        // silent rather than a second warning for the same availability.
        assert_eq!(
            arrived_magnification_warning(Some(unavailable), unavailable, false, true),
            ArrivedMagnificationWarning::Skip
        );
        assert_eq!(
            arrived_magnification_warning(Some(unavailable), unavailable, true, true),
            ArrivedMagnificationWarning::Skip
        );
    }

    #[test]
    fn a_page_with_nothing_magnified_neither_warns_nor_arms() {
        // Never warned yet, nothing magnified here: stay silent, and stay
        // owing the warning for the next page that does hold one.
        assert_eq!(
            arrived_magnification_warning(
                None,
                SpotlightMagnifierSource::IncompleteTransparent,
                false,
                true
            ),
            ArrivedMagnificationWarning::Skip
        );
        assert_eq!(
            arrived_magnification_warning(
                None,
                SpotlightMagnifierSource::IncompleteTransparent,
                true,
                true
            ),
            ArrivedMagnificationWarning::Warn
        );
    }

    #[test]
    fn a_suppressed_frame_still_owes_the_arrival_warning() {
        // The capture picker is open, so no toast can be seen. Skipping must
        // not count as having warned, or the user never learns why the loupes
        // are flat.
        assert_eq!(
            arrived_magnification_warning(
                None,
                SpotlightMagnifierSource::IncompleteTransparent,
                true,
                false
            ),
            ArrivedMagnificationWarning::Skip
        );
        assert_eq!(
            arrived_magnification_warning(
                None,
                SpotlightMagnifierSource::IncompleteTransparent,
                true,
                true
            ),
            ArrivedMagnificationWarning::Warn
        );
    }

    #[test]
    fn a_suppressed_frame_neither_warns_nor_counts_as_having_warned() {
        // The picker suppresses transients, so no toast can be shown...
        assert!(!spotlight_magnifier_warning_is_due(false, false));
        // ...and because nothing was shown, the flag stays clear and the next
        // frame that *can* warn still does.
        assert!(spotlight_magnifier_warning_is_due(false, true));
    }

    #[test]
    fn a_standing_warning_is_not_repeated_every_frame() {
        assert!(!spotlight_magnifier_warning_is_due(true, true));
    }

    #[test]
    fn picker_preview_follows_the_annotated_export_choice() {
        assert!(capture_picker_draws_committed(false, false));
        assert!(capture_picker_draws_committed(false, true));
        assert!(capture_picker_draws_committed(true, true));
        assert!(!capture_picker_draws_committed(true, false));
    }
}
