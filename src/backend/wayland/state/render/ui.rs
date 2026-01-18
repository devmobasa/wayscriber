use super::tool_preview::draw_tool_preview;
use super::*;

impl WaylandState {
    pub(super) fn render_ui_layers(
        &mut self,
        ctx: &cairo::Context,
        width: u32,
        height: u32,
        render_ui: bool,
    ) {
        if render_ui {
            if self.input_state.show_tool_preview
                && self.has_pointer_focus()
                && !self.pointer_over_toolbar()
                && matches!(
                    self.input_state.state,
                    DrawingState::Idle | DrawingState::PendingTextClick { .. }
                )
            {
                let (cursor_x, cursor_y) = self.current_mouse();
                draw_tool_preview(
                    ctx,
                    self.input_state.active_tool(),
                    self.input_state.current_color,
                    cursor_x as f64,
                    cursor_y as f64,
                    width as f64,
                    height as f64,
                );
            }
            // Render frozen badge even if status bar is hidden
            if self.input_state.frozen_active()
                && !self.zoom.active
                && self.config.ui.show_frozen_badge
            {
                crate::ui::render_frozen_badge(ctx, width, height);
            }
            // Render a zoom badge when the status bar is hidden or zoom is locked.
            if self.input_state.zoom_active()
                && (!self.input_state.show_status_bar || self.input_state.zoom_locked())
            {
                crate::ui::render_zoom_badge(
                    ctx,
                    width,
                    height,
                    self.input_state.zoom_scale(),
                    self.input_state.zoom_locked(),
                );
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

            // Render status bar if enabled
            if self.input_state.show_status_bar {
                crate::ui::render_status_bar(
                    ctx,
                    &self.input_state,
                    self.config.ui.status_bar_position,
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
            crate::ui::render_command_palette(ctx, &self.input_state, width, height);
            crate::ui::render_tour(ctx, &self.input_state, width, height);
        } else {
            self.input_state.clear_context_menu_layout();
        }
    }
}
