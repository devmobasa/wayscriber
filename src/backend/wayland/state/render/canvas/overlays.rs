use super::super::super::*;
use std::collections::HashSet;

impl WaylandState {
    pub(super) fn render_selection_overlays(&mut self, ctx: &cairo::Context) {
        if self.input_state.has_selection() {
            let selected: HashSet<_> = self
                .input_state
                .selected_shape_ids()
                .iter()
                .copied()
                .collect();
            let frame = self.input_state.canvas_set.active_frame();
            for drawn in &frame.shapes {
                if selected.contains(&drawn.id) {
                    crate::draw::render_selection_halo(ctx, drawn);
                }
            }
        }

        if matches!(
            self.input_state.state,
            DrawingState::Idle | DrawingState::ResizingText { .. }
        ) && let Some((_shape_id, handle)) = self.input_state.selected_text_resize_handle()
        {
            let _ = ctx.save();
            ctx.rectangle(
                handle.x as f64,
                handle.y as f64,
                handle.width as f64,
                handle.height as f64,
            );
            ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
            let _ = ctx.fill_preserve();
            ctx.set_source_rgba(0.2, 0.45, 1.0, 0.9);
            ctx.set_line_width(1.5);
            let _ = ctx.stroke();
            let _ = ctx.restore();
        }
    }

    pub(super) fn render_eraser_hover_halos(&mut self, ctx: &cairo::Context, mx: i32, my: i32) {
        let eraser_drawing = matches!(
            self.input_state.state,
            DrawingState::Drawing {
                tool: Tool::Eraser,
                ..
            }
        );
        let eraser_stroke = self.input_state.eraser_mode == EraserMode::Stroke;
        let eraser_hover = eraser_stroke
            && !eraser_drawing
            && self.input_state.active_tool() == Tool::Eraser
            && matches!(self.input_state.state, DrawingState::Idle)
            && self.has_pointer_focus()
            && !self.pointer_over_toolbar();
        if eraser_stroke && (eraser_drawing || eraser_hover) {
            self.input_state.ensure_spatial_index_for_active_frame();
            let ids = if eraser_drawing {
                if let DrawingState::Drawing {
                    tool: Tool::Eraser,
                    points,
                    ..
                } = &self.input_state.state
                {
                    let sampled = self.input_state.sample_eraser_path_points(points);
                    self.input_state.hit_test_all_for_points_cached(
                        &sampled,
                        self.input_state.eraser_hit_radius(),
                    )
                } else {
                    Vec::new()
                }
            } else {
                let point = [(mx, my)];
                self.input_state
                    .hit_test_all_for_points_cached(&point, self.input_state.eraser_hit_radius())
            };
            if !ids.is_empty() {
                let frame = self.input_state.canvas_set.active_frame();
                match ids.len() {
                    1 => {
                        if let Some(drawn) = frame.shape(ids[0]) {
                            crate::draw::render_selection_halo(ctx, drawn);
                        }
                    }
                    2..=4 => {
                        let mut indices = Vec::with_capacity(ids.len());
                        for id in &ids {
                            if let Some(index) = frame.find_index(*id) {
                                indices.push(index);
                            }
                        }
                        if !indices.is_empty() {
                            indices.sort_unstable();
                            indices.dedup();
                            for index in indices {
                                if let Some(drawn) = frame.shapes.get(index) {
                                    crate::draw::render_selection_halo(ctx, drawn);
                                }
                            }
                        }
                    }
                    _ => {
                        let hover_ids: HashSet<_> = ids.into_iter().collect();
                        for drawn in &frame.shapes {
                            if hover_ids.contains(&drawn.id) {
                                crate::draw::render_selection_halo(ctx, drawn);
                            }
                        }
                    }
                }
            }
        }
    }
}
