use super::base::{DrawingState, InputState};
use crate::input::tool::Tool;
use cairo::Context as CairoContext;
use std::time::Instant;

impl InputState {
    /// Returns whether the click highlight feature is currently enabled.
    pub fn click_highlight_enabled(&self) -> bool {
        self.click_highlight.enabled()
    }

    /// Toggle the click highlight feature and mark the frame for redraw.
    pub fn toggle_click_highlight(&mut self) -> bool {
        let enabled = self.click_highlight.toggle(&mut self.dirty_tracker);
        self.needs_redraw = true;
        enabled
    }

    /// Clears any active highlights without changing the enabled flag.
    pub fn clear_click_highlights(&mut self) {
        if self.click_highlight.has_active() {
            self.click_highlight.clear_all(&mut self.dirty_tracker);
            self.needs_redraw = true;
        }
    }

    /// Spawns a highlight at the given position if the feature is enabled.
    pub fn trigger_click_highlight(&mut self, x: i32, y: i32) {
        if self.click_highlight.spawn(x, y, &mut self.dirty_tracker) {
            self.needs_redraw = true;
        }
    }

    pub fn sync_highlight_color(&mut self) {
        if self.click_highlight.apply_pen_color(self.current_color) {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Advances highlight animations; returns true if highlights remain active.
    pub fn advance_click_highlights(&mut self, now: Instant) -> bool {
        self.click_highlight.advance(now, &mut self.dirty_tracker)
    }

    /// Render active highlights to the cairo context.
    pub fn render_click_highlights(&self, ctx: &CairoContext, now: Instant) {
        self.click_highlight.render(ctx, now);
    }

    /// Returns the active tool considering overrides and drawing state.
    pub fn active_tool(&self) -> Tool {
        if let DrawingState::Drawing { tool, .. } = &self.state {
            *tool
        } else if let Some(tool) = self.tool_override {
            tool
        } else {
            self.modifiers.current_tool()
        }
    }

    /// Returns whether the highlight tool is currently selected.
    pub fn highlight_tool_active(&self) -> bool {
        matches!(self.tool_override, Some(Tool::Highlight))
            || matches!(
                self.state,
                DrawingState::Drawing {
                    tool: Tool::Highlight,
                    ..
                }
            )
    }

    /// Toggles highlight-only tool mode.
    pub fn toggle_highlight_tool(&mut self) -> bool {
        let enable = !self.highlight_tool_active();

        if enable {
            self.tool_override = Some(Tool::Highlight);
            // Ensure we are not mid-drawing with another tool
            if !matches!(
                self.state,
                DrawingState::Idle | DrawingState::TextInput { .. }
            ) {
                self.state = DrawingState::Idle;
            }
        } else {
            self.tool_override = None;
        }

        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        enable
    }
}
