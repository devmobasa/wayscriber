use super::tool_preview::{draw_stylus_hover_cursor, draw_tool_preview, mouse_tool_preview_redraw};
use super::*;

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
        if render_ui {
            if self.mouse_tool_preview_eligible() {
                let (cursor_x, cursor_y) =
                    self.stylus_hover_cursor_position().unwrap_or_else(|| {
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
            if let Some((cursor_x, cursor_y)) = self.stylus_hover_cursor_position()
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
            // Mode badges: when the status HUD is visible they render as
            // pills stacked on the HUD (see render_status_bar); the floating
            // top-corner badges below cover the hidden-status-bar case only.
            if self.input_state.frozen_active()
                && !self.zoom.active
                && self.config.ui.show_frozen_badge
                && !self.input_state.show_status_bar
            {
                crate::ui::render_frozen_badge(ctx, width, height);
            }
            // Render a zoom badge when the status bar is hidden.
            // Badge renderers return the vertical space they consume (measured
            // height plus stacking gap) so stacked badges never overlap.
            //
            // Reconciliation (M8): the bottom-right zoom chip is the canonical
            // zoom indicator/control whenever `show_zoom_actions` is on, so the
            // passive top-corner zoom badge is suppressed then — otherwise the
            // chip and this badge would both show the zoom percentage. When
            // zoom actions are off (chip absent) this passive badge remains the
            // hidden-status-bar zoom indicator, exactly as before M8.
            let mut top_badge_offset = 0.0;
            if self.input_state.zoom_active()
                && !self.input_state.show_status_bar
                && !self.input_state.show_zoom_actions
            {
                top_badge_offset += crate::ui::render_zoom_badge(
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
                && !self.input_state.show_status_bar
            {
                top_badge_offset += crate::ui::render_pan_badge(
                    ctx,
                    width,
                    height,
                    self.input_state.boards.active_frame().view_offset() != (0, 0),
                    top_badge_offset,
                );
            }
            // Render editing badge when in text edit mode
            if matches!(self.input_state.state, DrawingState::TextInput { .. })
                && self.input_state.text_edit_target.is_some()
                && !self.input_state.show_status_bar
            {
                crate::ui::render_editing_badge(ctx, width, height, top_badge_offset);
            }
            if !self.input_state.show_status_bar || self.input_state.show_floating_badge_always {
                let board_count = self.input_state.boards.board_count();
                let page_count = self.input_state.boards.page_count();
                if board_count > 1 || page_count > 1 {
                    let board_index = self.input_state.boards.active_index();
                    let board_name = self.input_state.board_name();
                    let page_index = self.input_state.boards.active_page_index();
                    crate::ui::render_page_badge(
                        ctx,
                        width,
                        height,
                        board_index,
                        board_count,
                        board_name,
                        page_index,
                        page_count,
                    );
                }
            }

            // Render the status HUD if enabled (layout was cached for this
            // frame by collect_ui_effect_damage).
            if self.input_state.show_status_bar {
                crate::ui::render_status_bar(
                    ctx,
                    &self.input_state,
                    &self.config.ui.status_bar_style,
                    width,
                    height,
                );
            }

            // Render the interactive bottom-right zoom chip (layout cached for
            // this frame by collect_ui_effect_damage). Reuses the status-bar
            // style tokens so it reads as the same chrome family.
            if self.zoom_chip_visible() {
                crate::ui::render_zoom_chip(
                    ctx,
                    &self.input_state,
                    &self.config.ui.status_bar_style,
                    width,
                    height,
                );
            }

            // Render help overlay if toggled
            if self.input_state.show_help {
                let bindings = crate::ui::HelpOverlayBindings::from_input_state(&self.input_state);
                let scroll_max = crate::ui::render_help_overlay(
                    ctx,
                    &self.config.ui.help_overlay_style,
                    width,
                    height,
                    self.frozen_enabled(),
                    self.input_state.help_overlay_page,
                    &bindings,
                    self.input_state.help_overlay_search.as_str(),
                    self.config.ui.help_overlay_context_filter,
                    self.input_state.boards.board_count() > 1,
                    self.config.capture.enabled,
                    self.input_state.help_overlay_scroll,
                    self.input_state.help_overlay_quick_mode,
                );
                self.input_state.help_overlay_scroll_max = scroll_max;
                self.input_state.help_overlay_scroll =
                    self.input_state.help_overlay_scroll.clamp(0.0, scroll_max);
            }

            if self.input_state.is_board_picker_open() {
                self.input_state
                    .update_board_picker_layout(ctx, width, height);
                crate::ui::render_board_picker(ctx, &self.input_state, width, height);
            } else {
                self.input_state.clear_board_picker_layout();
            }

            if self.input_state.is_color_picker_popup_open() {
                self.input_state
                    .update_color_picker_popup_layout(width, height);
                crate::ui::render_color_picker_popup(ctx, &self.input_state, width, height);
            } else {
                self.input_state.clear_color_picker_popup_layout();
            }

            if self.input_state.is_precision_entry_open() {
                // Anchor under the top strip (the pill is its bottom row):
                // the same base position both the inline fallback and the
                // layer-shell margins derive from.
                let snapshot = self.toolbar_snapshot();
                let (_, top_h) = crate::backend::wayland::toolbar::top_size(&snapshot);
                let anchor = (
                    self.inline_top_base_x(&snapshot) + self.data.toolbar_top_offset,
                    self.inline_top_base_y() + self.data.toolbar_top_offset_y + top_h as f64 + 8.0,
                );
                crate::ui::render_precision_entry_popup(
                    ctx,
                    &self.input_state,
                    width,
                    height,
                    anchor,
                );
            }

            self.render_eyedropper_loupe(ctx, width, height);

            if self.input_state.is_radial_menu_open() {
                // Layout (and with it hit-testing) is live from the moment
                // of opening so pre-paint flicks resolve correctly; painting
                // waits out the flick window (RADIAL_PAINT_DELAY).
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

            self.input_state.ui_toast_bounds =
                crate::ui::render_ui_toast(ctx, &self.input_state, width, height);
            crate::ui::render_preset_toast(ctx, &self.input_state, width, height);
            crate::ui::render_blocked_feedback(ctx, &self.input_state, width, height);

            if !self.zoom.active && !self.input_state.is_board_picker_open() {
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

                // Render context menu if open
                crate::ui::render_context_menu(ctx, &self.input_state, width, height);
            } else {
                self.input_state.clear_context_menu_layout();
                self.input_state.clear_properties_panel_layout();
            }

            // Inline toolbars (xdg fallback or drag preview) render directly into main surface.
            if self.toolbar.is_visible() && self.inline_toolbars_render_active() {
                let snapshot = self.toolbar_snapshot();
                if self.toolbar.update_snapshot(&snapshot) {
                    self.toolbar.mark_dirty();
                }
                self.render_inline_toolbars(ctx, &snapshot);
            }

            // Modal overlays render last (on top of everything including toolbars)
            if let Some(card) = self.first_run_onboarding_card() {
                crate::ui::render_onboarding_card(ctx, width, height, &card);
            }
            crate::ui::render_command_palette(ctx, &self.input_state, width, height);
            crate::ui::render_tour(ctx, &self.input_state, width, height);
        } else {
            self.input_state.clear_context_menu_layout();
        }
    }

    /// The render-time gate for the mouse-anchored tool-preview bubble: it is
    /// drawn only for eligible idle states, with cursor focus and no toolbar
    /// blocking. Shared by the render pass (above) and the pointer handler so
    /// idle-motion damage and the actual draw can never disagree about whether
    /// the bubble is visible.
    pub(in crate::backend::wayland) fn mouse_tool_preview_eligible(&self) -> bool {
        self.input_state.show_tool_preview
            && self.has_cursor_focus()
            && !self.cursor_blocked_by_toolbar()
            && matches!(
                self.input_state.state,
                DrawingState::Idle | DrawingState::PendingTextClick { .. }
            )
    }

    /// The render/damage/hit-test gate for the interactive bottom-right zoom
    /// chip: shown whenever zoom actions are enabled (the existing
    /// `show_zoom_actions` toggle — no new config key). Like the status bar,
    /// the chip is a PERSISTENT fixed-corner control, not a cursor-follower, so
    /// it is deliberately NOT gated on cursor focus or toolbar blocking: gating
    /// it that way regressed the zoom readout to nothing whenever the pointer
    /// sat over the toolbar or off-surface (e.g. while clicking the Canvas
    /// popover's Zoom buttons), because the fallback badges are suppressed for
    /// the whole `show_zoom_actions` window. Keeping the chip visible for that
    /// entire window makes chip-shown ⟺ fallbacks-suppressed, so exactly one
    /// zoom indicator shows in every state — never zero, never two. Shared by
    /// the render pass, the damage collector (which caches the layout), and the
    /// pointer/touch press guards, so all three agree on whether the chip
    /// exists this frame.
    pub(in crate::backend::wayland) fn zoom_chip_visible(&self) -> bool {
        self.input_state.show_zoom_actions
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
