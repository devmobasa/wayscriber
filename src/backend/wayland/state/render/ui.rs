use super::tool_preview::{draw_stylus_hover_cursor, draw_tool_preview, mouse_tool_preview_redraw};
use super::*;
use crate::backend::wayland::state::region_capture::RegionPickerMeasurement;
use crate::backend::wayland::state::screen_image::{
    displayed_screen_image, image_point_for_screen_point, screen_source_is,
};

impl WaylandState {
    pub(super) fn render_ui_layer(
        &mut self,
        ctx: &cairo::Context,
        width: u32,
        height: u32,
        scale: i32,
        render_ui: bool,
    ) {
        let _ = ctx.save();
        if scale > 1 {
            ctx.scale(scale as f64, scale as f64);
        }
        self.render_ui_layers(ctx, width, height, render_ui);
        let _ = ctx.restore();
    }

    fn render_ui_layers(&mut self, ctx: &cairo::Context, width: u32, height: u32, render_ui: bool) {
        if !render_ui {
            self.input_state.clear_context_menu_layout();
            return;
        }
        let capture_picker = self.capture_picker_chrome_suppressed();
        if capture_picker {
            self.render_capture_picker(ctx, width, height);
        }
        self.render_cursor_chrome(ctx, width, height, capture_picker);
        self.render_mode_badges(ctx, width, height, capture_picker);
        self.render_status_surfaces(ctx, width, height, capture_picker);
        self.render_help_and_pickers(ctx, width, height, capture_picker);
        self.render_radial_menu_and_feedback(ctx, width, height, capture_picker);
        self.render_properties_and_context(ctx, width, height, capture_picker);
        self.render_inline_and_modal_ui(ctx, width, height, capture_picker);
    }

    fn render_cursor_chrome(
        &mut self,
        ctx: &cairo::Context,
        width: u32,
        height: u32,
        capture_picker: bool,
    ) {
        if !capture_picker && self.mouse_tool_preview_eligible() {
            let (cursor_x, cursor_y) = self.stylus_hover_cursor_position().unwrap_or_else(|| {
                let (x, y) = self.current_mouse();
                (x as f64, y as f64)
            });
            draw_tool_preview(
                ctx,
                self.input_state.active_tool(),
                self.input_state
                    .color_for_tool(self.input_state.active_tool()),
                self.input_state.thickness_for_active_tool(),
                cursor_x,
                cursor_y,
                width as f64,
                height as f64,
            );
        }
        if !capture_picker {
            self.render_shape_measure_badge(ctx, width, height);
        }
        if let Some((cursor_x, cursor_y)) = self.stylus_hover_cursor_position()
            && !capture_picker
            && !self.cursor_blocked_by_toolbar()
        {
            draw_stylus_hover_cursor(
                ctx,
                self.input_state.active_tool(),
                self.input_state
                    .color_for_tool(self.input_state.active_tool()),
                cursor_x,
                cursor_y,
            );
        }
    }

    fn render_mode_badges(
        &self,
        ctx: &cairo::Context,
        width: u32,
        height: u32,
        capture_picker: bool,
    ) {
        let fallback_visible = !capture_picker && self.input_state.fallback_mode_badges_visible();
        let status_visible = self.input_state.status_hud_effectively_visible();
        if self.input_state.frozen_active()
            && !self.zoom.active
            && self.config.ui.show_frozen_badge
            && !status_visible
            && fallback_visible
        {
            crate::ui::render_frozen_badge(ctx, width, height);
        }
        let mut offset = 0.0;
        if self.input_state.zoom_active()
            && !status_visible
            && !self.zoom_chip_visible()
            && fallback_visible
        {
            offset += crate::ui::render_zoom_badge(
                ctx,
                width,
                height,
                self.input_state.zoom_scale(),
                self.input_state.zoom_locked(),
            );
        }
        if self.input_state.boards.pan_enabled()
            && self.input_state.boards.show_pan_badge()
            && !self.input_state.board_is_transparent()
            && !status_visible
            && fallback_visible
        {
            offset += crate::ui::render_pan_badge(
                ctx,
                width,
                height,
                self.input_state.boards.active_frame().view_offset() != (0, 0),
                offset,
            );
        }
        if matches!(self.input_state.state, DrawingState::TextInput { .. })
            && self.input_state.text_editing.text_edit_target.is_some()
            && !status_visible
            && fallback_visible
        {
            crate::ui::render_editing_badge(ctx, width, height, offset);
        }
    }

    fn render_status_surfaces(
        &self,
        ctx: &cairo::Context,
        width: u32,
        height: u32,
        capture_picker: bool,
    ) {
        if !capture_picker && self.input_state.floating_badge_visible() {
            crate::ui::render_page_badge(
                ctx,
                width,
                height,
                self.input_state.boards.active_index(),
                self.input_state.boards.board_count(),
                self.input_state.board_name(),
                self.input_state.boards.active_page_index(),
                self.input_state.boards.page_count(),
            );
        }
        if self.input_state.ui_visibility.show_status_bar {
            crate::ui::render_status_bar(
                ctx,
                &self.input_state,
                &self.config.ui.status_bar_style,
                width,
                height,
            );
        }
        if !capture_picker && self.zoom_chip_visible() {
            crate::ui::render_zoom_chip(
                ctx,
                &self.input_state,
                &self.config.ui.status_bar_style,
                width,
                height,
            );
        }
        if !capture_picker && self.input_state.input_hud_visible() {
            crate::ui::render_input_hud(
                ctx,
                &self.input_state,
                &self.config.ui.status_bar_style,
                width,
                height,
            );
        }
    }

    fn render_help_and_pickers(
        &mut self,
        ctx: &cairo::Context,
        width: u32,
        height: u32,
        capture_picker: bool,
    ) {
        if !capture_picker && self.input_state.help_overlay.is_visible() {
            let bindings = crate::ui::HelpOverlayBindings::from_input_state(&self.input_state);
            let scroll_max = crate::ui::render_help_overlay(
                ctx,
                &self.config.ui.help_overlay_style,
                width,
                height,
                self.frozen_enabled(),
                self.input_state.help_overlay.page(),
                &bindings,
                self.input_state.help_overlay.query(),
                self.config.ui.help_overlay_context_filter,
                self.input_state.boards.board_count() > 1,
                self.config.capture.enabled,
                self.input_state.help_overlay.scroll(),
                self.input_state.help_overlay.is_quick_mode(),
            );
            self.input_state
                .help_overlay
                .update_scroll_extent(scroll_max);
        }
        if !capture_picker && self.input_state.is_board_picker_open() {
            self.input_state
                .update_board_picker_layout(ctx, width, height);
            crate::ui::render_board_picker_with_halo(
                ctx,
                &self.input_state,
                width,
                height,
                self.config.drawing.text_halo_enabled,
            );
        } else {
            self.input_state.clear_board_picker_layout();
        }
        if !capture_picker && self.input_state.is_color_picker_popup_open() {
            self.input_state
                .update_color_picker_popup_layout(width, height);
            crate::ui::render_color_picker_popup(ctx, &self.input_state, width, height);
        } else {
            self.input_state.clear_color_picker_popup_layout();
        }
        if !capture_picker && self.input_state.is_font_picker_open() {
            crate::ui::render_font_picker(ctx, &self.input_state, width, height);
        }
        if !capture_picker && self.input_state.is_precision_entry_open() {
            self.render_precision_entry(ctx, width, height);
        }
        if !capture_picker {
            self.render_eyedropper_loupe(ctx, width, height);
        }
        self.render_ocr_selection(ctx, width, height);
    }

    fn render_precision_entry(&mut self, ctx: &cairo::Context, width: u32, height: u32) {
        let snapshot = self.toolbar_snapshot();
        let (_, top_h) = crate::backend::wayland::toolbar::top_size(&snapshot);
        let anchor = (
            self.inline_top_base_x() + self.data.toolbar_top_offset,
            self.inline_top_base_y() + self.data.toolbar_top_offset_y + top_h as f64 + 8.0,
        );
        crate::ui::render_precision_entry_popup(ctx, &self.input_state, width, height, anchor);
    }

    fn render_radial_menu_and_feedback(
        &mut self,
        ctx: &cairo::Context,
        width: u32,
        height: u32,
        capture_picker: bool,
    ) {
        if !capture_picker && self.input_state.is_radial_menu_open() {
            self.input_state.update_radial_menu_layout(width, height);
            if self
                .input_state
                .radial_menu_mark_painted_if_due(std::time::Instant::now())
            {
                crate::ui::render_radial_menu(ctx, &self.input_state, width, height);
            }
        } else {
            self.input_state.clear_radial_menu_layout();
        }
        let toast_geometry = crate::ui::render_ui_toast(ctx, &self.input_state, width, height);
        self.input_state.ui_toast_bounds = toast_geometry.map(|geometry| geometry.0);
        self.input_state.ui_toast_action_bounds = toast_geometry
            .map(|geometry| geometry.1)
            .unwrap_or([None, None]);
        crate::ui::render_preset_toast(ctx, &self.input_state, width, height);
        crate::ui::render_blocked_feedback(ctx, &self.input_state, width, height);
    }

    fn render_properties_and_context(
        &mut self,
        ctx: &cairo::Context,
        width: u32,
        height: u32,
        capture_picker: bool,
    ) {
        if capture_picker || self.zoom.active || self.input_state.is_board_picker_open() {
            self.input_state.clear_context_menu_layout();
            self.input_state.clear_properties_panel_layout();
            return;
        }
        if self.input_state.is_properties_panel_open() {
            self.input_state
                .update_properties_panel_layout(ctx, width, height);
        } else {
            self.input_state.clear_properties_panel_layout();
        }
        crate::ui::render_properties_panel(ctx, &self.input_state, width, height);
        if self.input_state.is_context_menu_open() {
            self.input_state
                .update_context_menu_layout(ctx, width, height);
        } else {
            self.input_state.clear_context_menu_layout();
        }
        crate::ui::render_context_menu(ctx, &self.input_state, width, height);
    }

    fn render_inline_and_modal_ui(
        &mut self,
        ctx: &cairo::Context,
        width: u32,
        height: u32,
        capture_picker: bool,
    ) {
        if !capture_picker && self.toolbar.is_visible() && self.inline_toolbars_render_active() {
            let snapshot = self.toolbar_snapshot();
            if self.toolbar.update_snapshot(&snapshot) {
                self.toolbar.mark_dirty();
            }
            self.render_inline_toolbars(ctx, &snapshot);
        }
        if self.input_state.region_state().purpose()
            == Some(crate::input::state::RegionPurposeTag::Measure)
        {
            self.render_capture_picker(ctx, width, height);
        }
        self.render_ocr_scan(ctx, width, height);
        if capture_picker {
            return;
        }
        if let Some(card) = self.first_run_onboarding_card() {
            crate::ui::render_onboarding_card(ctx, width, height, &card);
        }
        crate::ui::render_command_palette(ctx, &self.input_state, width, height);
        crate::ui::render_tour(ctx, &self.input_state, width, height);
    }

    /// The scan band while recognition runs, then the outcome card. The card
    /// reports what happened, never the recognized text: keeping screen
    /// contents out of application state is an invariant of `src/ocr`.
    fn render_ocr_scan(&self, ctx: &cairo::Context, width: u32, height: u32) {
        let Some(scan) = self.input_state.ocr_scan() else {
            return;
        };
        let now = std::time::Instant::now();
        if let Some((outcome, shown)) = scan.result(now) {
            crate::ui::render_ocr_scan_result(ctx, scan.region(), outcome, shown, (width, height));
        } else if let Some(progress) = scan.sweep_progress(now) {
            crate::ui::render_ocr_scan_sweep(ctx, scan.region(), progress);
        } else if scan.is_scanning() {
            // Reduced motion: the region is still marked as being read, just
            // without a band travelling across it.
            crate::ui::render_ocr_scan_still(ctx, scan.region());
        }
    }

    fn render_capture_picker(&self, ctx: &cairo::Context, width: u32, height: u32) {
        let purpose = self.input_state.region_state().purpose();
        if !self.input_state.region_is_active()
            || !purpose.is_some_and(|purpose| {
                purpose.is_capture() || purpose == crate::input::state::RegionPurposeTag::Measure
            })
        {
            return;
        }

        let measure_mode = purpose == Some(crate::input::state::RegionPurposeTag::Measure);

        let options = match self.capture.region_phase() {
            crate::backend::wayland::capture::RegionCapturePhase::Reserved(intent) => {
                Some(intent.options())
            }
            crate::backend::wayland::capture::RegionCapturePhase::Idle
            | crate::backend::wayland::capture::RegionCapturePhase::Submitting(_)
            | crate::backend::wayland::capture::RegionCapturePhase::Accepted => None,
        };
        let geometry = self.region_selection_geometry();
        let selection = if measure_mode {
            self.region_measure_selection()
        } else {
            geometry.map(|geometry| geometry.display_selection())
        };
        let (pointer_x, pointer_y) = self.current_mouse();
        let pointer = (f64::from(pointer_x), f64::from(pointer_y));
        let measurement = (measure_mode
            || options.is_some_and(|options| options.show_size_readout()))
        .then(|| self.region_picker_measurement(pointer))
        .flatten()
        .map(|measurement| match measurement {
            RegionPickerMeasurement::Point { x, y } => format!("{x}, {y}"),
            RegionPickerMeasurement::Size { width, height } => {
                crate::ui::capture_size_text((width, height))
            }
        });
        let region_state = self.input_state.region_state();
        let action_bar = if region_state.is_review() {
            geometry.map(|geometry| {
                crate::ui::RegionActionBar::place(geometry.display_selection(), (width, height))
            })
        } else {
            None
        };
        let hovered_action: Option<crate::ui::RegionAction> =
            action_bar.and_then(|bar| bar.hit(pointer));
        // Grips belong to the reviewed rectangle, not the window-mode
        // candidate, so they follow `region_selection_geometry` rather than
        // the effective selection the scrim cuts out.
        let resize_handles = (region_state.is_review()
            && !self.region_review_crop_locked()
            && !self.region_cut_mode_armed())
        .then(|| {
            geometry
                .map(|geometry| crate::ui::RegionResizeHandles::place(geometry.display_selection()))
        });
        let resize_handles = resize_handles.flatten();
        // A grip sitting under the action bar is painted over by it, so the
        // bar's hover wins the pointer.
        let hovered_handle = (hovered_action.is_none()
            && !action_bar.is_some_and(|bar| bar.contains(pointer)))
        .then(|| resize_handles.and_then(|handles| handles.hit(pointer)))
        .flatten();
        let loupe_enabled = options.is_some_and(|options| options.show_loupe())
            && (region_state.is_selecting() || region_state.is_review())
            && !self.region_review_loupe_suppressed();
        let loupe_source = if loupe_enabled {
            self.region_picker_source_token().and_then(|token| {
                let source = displayed_screen_image(
                    &self.zoom,
                    &self.frozen,
                    self.input_state.board_is_transparent(),
                )?;
                screen_source_is(&token, &source, &self.zoom, &self.frozen, (width, height))
                    .then_some((source, token))
            })
        } else {
            None
        };
        let loupe = loupe_source.as_ref().and_then(|(_source, token)| {
            let image_point = image_point_for_screen_point(token, pointer);
            crate::ui::RegionCaptureLoupeVisual::when_enabled(
                loupe_enabled,
                pointer,
                (image_point.x, image_point.y),
            )
        });
        let window = crate::ui::RegionCaptureWindowVisual {
            available: self.region_window_snap_available(),
            active: self.region_window_snap_active(),
            targets: self.region_window_snap_display_selections(),
            highlighted_target: self.region_window_snap_highlighted_index(),
        };

        crate::ui::render_region_capture_picker(
            ctx,
            width,
            height,
            &crate::ui::RegionCapturePickerVisual {
                selection,
                pointer,
                measurement: measurement.as_deref(),
                show_scrim: !measure_mode,
                review: region_state.is_review(),
                resize_handles,
                hovered_handle,
                show_legend: options.is_some_and(|options| options.show_legend())
                    && !self.region_picker_legend_dismissed(),
                loupe,
                action_bar,
                hovered_action,
                include_drawings: self.region_picker_include_drawings(),
                cut: crate::ui::RegionCaptureCutVisual {
                    preview: self.region_cut_preview_pixels().and_then(|pixels| {
                        Some(crate::ui::RegionCutPreviewVisual {
                            pixels,
                            display: self.region_cut_displayed_selection()?,
                        })
                    }),
                    drag: self
                        .region_cut_drag_overlay()
                        .map(|(axis, band)| crate::ui::RegionCutDragVisual { axis, band }),
                    availability: self.region_cut_availability(),
                    cut_armed: self.region_cut_mode_armed(),
                    status: self.region_cut_status(),
                },
                window,
            },
            |image_x, image_y| {
                loupe_source.as_ref().and_then(|(source, _token)| {
                    crate::backend::wayland::state::eyedropper::sample_at(
                        source.image,
                        image_x,
                        image_y,
                    )
                })
            },
        );
    }

    /// The render-time gate for the mouse-anchored tool-preview bubble: it is
    /// drawn only for eligible idle states, with cursor focus and no toolbar
    /// blocking. Shared by the render pass (above) and the pointer handler so
    /// idle-motion damage and the actual draw can never disagree about whether
    /// the bubble is visible.
    pub(in crate::backend::wayland) fn mouse_tool_preview_eligible(&self) -> bool {
        self.input_state.ui_visibility.show_tool_preview
            && self.has_cursor_focus()
            && !self.cursor_blocked_by_toolbar()
            && matches!(
                self.input_state.state,
                DrawingState::Idle | DrawingState::PendingTextClick { .. }
            )
    }

    /// The render/damage/hit-test gate for the interactive bottom-right zoom
    /// chip: shown whenever zoom actions and the persisted `show_zoom_chip`
    /// master preference are enabled and the `zoom_chip_display` policy allows
    /// it. Like the status bar,
    /// the chip is a PERSISTENT fixed-corner control, not a cursor-follower, so
    /// it is deliberately NOT gated on cursor focus or toolbar blocking: gating
    /// it that way regressed the zoom readout to nothing whenever the pointer
    /// sat over the toolbar or off-surface (e.g. while clicking the Canvas
    /// popover's Zoom buttons). Effective chip visibility is also the fallback
    /// suppression gate, so outside Focus Mode exactly one zoom indicator shows
    /// in every state — never zero, never two. Focus Mode intentionally hides
    /// both the chip and its fallback badges. Otherwise, hiding the chip via
    /// `ToggleZoomChip` hands the readout to the status-HUD badge when the bar is
    /// visible, or the passive top-corner badge when it is hidden. Shared by the
    /// render pass, the damage collector (which caches the layout), and the
    /// pointer/touch press guards, so all three agree on whether the chip exists
    /// this frame.
    pub(in crate::backend::wayland) fn zoom_chip_visible(&self) -> bool {
        self.input_state.zoom_chip_enabled()
    }

    /// Damage the previous and current preview-bubble footprints and request a
    /// redraw so the bubble tracks idle pointer motion from `prev` to `next`
    /// (screen-space, matching [`Self::current_mouse`]).
    ///
    /// Only the mouse-anchored bubble is handled here: when a stylus is
    /// hovering the preview follows the stylus position instead, and the tablet
    /// frame handler owns that damage via `mark_stylus_hover_cursor_dirty`.
    pub(in crate::backend::wayland) fn mark_mouse_tool_preview_dirty(
        &mut self,
        prev: (i32, i32),
        next: (i32, i32),
    ) {
        if self.stylus_hover_cursor_position().is_some() {
            return;
        }
        let redraw = mouse_tool_preview_redraw(
            self.mouse_tool_preview_eligible(),
            self.input_state.thickness_for_active_tool(),
            (prev.0 as f64, prev.1 as f64),
            (next.0 as f64, next.1 as f64),
            self.surface.width(),
            self.surface.height(),
        );
        if !redraw.redraw {
            return;
        }
        if redraw.rects.is_empty() {
            self.input_state.dirty_tracker.mark_full();
        } else {
            for rect in redraw.rects {
                self.input_state.dirty_tracker.mark_rect(rect);
            }
        }
        self.input_state.needs_redraw = true;
    }
}
