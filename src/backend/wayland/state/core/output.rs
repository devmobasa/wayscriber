use log::{info, warn};
use smithay_client_toolkit::shell::{
    WaylandSurface,
    wlr_layer::{Anchor, Layer},
};

use super::super::*;
use crate::input::state::{OutputFocusAction, UiToastKind};

const OUTPUT_BADGE_MAX_LEN: usize = 28;

impl WaylandState {
    pub(in crate::backend::wayland) fn preferred_fullscreen_output(
        &self,
    ) -> Option<wl_output::WlOutput> {
        if let Some(preferred) = self.preferred_output_identity()
            && let Some(output) = self.output_state.outputs().find(|output| {
                self.output_identity_for(output)
                    .map(|id| id.eq_ignore_ascii_case(preferred))
                    .unwrap_or(false)
            })
        {
            return Some(output);
        }

        self.surface
            .current_output()
            .or_else(|| self.output_state.outputs().next())
    }

    pub(in crate::backend::wayland) fn output_identity_for(
        &self,
        output: &wl_output::WlOutput,
    ) -> Option<String> {
        let info = self.output_state.info(output)?;

        let mut components: Vec<String> = Vec::new();

        if let Some(name) = info.name.filter(|s| !s.is_empty()) {
            components.push(name);
        }

        if !info.make.is_empty() {
            components.push(info.make);
        }

        if !info.model.is_empty() {
            components.push(info.model);
        }

        if components.is_empty() {
            components.push(format!("id{}", info.id));
        }

        Some(components.join("-"))
    }

    fn sorted_known_outputs(&self) -> Vec<wl_output::WlOutput> {
        let mut outputs: Vec<(u32, wl_output::WlOutput)> = self
            .output_state
            .outputs()
            .filter_map(|output| {
                self.output_state
                    .info(&output)
                    .map(|info| (info.id, output))
            })
            .collect();

        outputs.sort_by_key(|(id, _)| *id);
        outputs.into_iter().map(|(_, output)| output).collect()
    }

    fn output_badge_label_for(&self, output: &wl_output::WlOutput) -> Option<String> {
        let info = self.output_state.info(output)?;

        if let Some(name) = info.name.as_deref().filter(|name| !name.is_empty()) {
            return Some(crate::util::truncate_with_ellipsis(
                name,
                OUTPUT_BADGE_MAX_LEN,
            ));
        }

        let label = match (info.make.trim(), info.model.trim()) {
            ("", "") => format!("Output {}", info.id),
            (make, "") => make.to_string(),
            ("", model) => model.to_string(),
            (make, model) => format!("{make} {model}"),
        };

        Some(crate::util::truncate_with_ellipsis(
            &label,
            OUTPUT_BADGE_MAX_LEN,
        ))
    }

    pub(in crate::backend::wayland) fn refresh_active_output_label(&mut self) {
        let next_label = self
            .surface
            .current_output()
            .as_ref()
            .and_then(|output| self.output_badge_label_for(output))
            .or_else(|| {
                self.sorted_known_outputs()
                    .first()
                    .and_then(|output| self.output_badge_label_for(output))
            });

        if self.input_state.active_output_label != next_label {
            self.input_state.active_output_label = next_label;
            self.input_state.needs_redraw = true;
        }
    }

    pub(in crate::backend::wayland) fn handle_output_focus_action(
        &mut self,
        qh: &QueueHandle<Self>,
        action: OutputFocusAction,
    ) {
        if !self.config.ui.multi_monitor_enabled {
            self.input_state.set_ui_toast(
                UiToastKind::Info,
                "Multi-monitor focus is disabled (ui.multi_monitor_enabled=false)",
            );
            self.input_state.trigger_blocked_feedback();
            return;
        }
        if self.capture.is_in_progress()
            || self.frozen.is_in_progress()
            || self.zoom.is_in_progress()
        {
            self.input_state.set_ui_toast(
                UiToastKind::Info,
                "Cannot switch outputs while capture, frozen mode, or zoom capture is active",
            );
            self.input_state.trigger_blocked_feedback();
            return;
        }

        let outputs = self.sorted_known_outputs();
        if outputs.len() <= 1 {
            self.input_state
                .set_ui_toast(UiToastKind::Info, "Only one output is available");
            self.input_state.trigger_blocked_feedback();
            return;
        }

        let current_output = self
            .surface
            .current_output()
            .or_else(|| self.preferred_fullscreen_output());
        let current_index = current_output
            .as_ref()
            .and_then(|current| outputs.iter().position(|output| output == current))
            .unwrap_or(0);
        let target_index = match action {
            OutputFocusAction::Next => (current_index + 1) % outputs.len(),
            OutputFocusAction::Prev => {
                if current_index == 0 {
                    outputs.len() - 1
                } else {
                    current_index - 1
                }
            }
        };
        let target_output = outputs[target_index].clone();
        let target_label = self
            .output_badge_label_for(&target_output)
            .unwrap_or_else(|| format!("Output {}", target_index + 1));

        if self.surface.is_xdg_window() {
            if !self.xdg_fullscreen() {
                self.input_state.set_ui_toast(
                    UiToastKind::Info,
                    "Enable ui.xdg_fullscreen to switch outputs on xdg fallback",
                );
                self.input_state.trigger_blocked_feedback();
                return;
            }
            let Some(window) = self.surface.xdg_window().cloned() else {
                warn!("Output switch requested in xdg mode, but no xdg window is active");
                return;
            };
            info!("Switching xdg overlay to {}", target_label);
            window.set_fullscreen(Some(&target_output));
            window.commit();
            self.surface.set_current_output(target_output);
            self.refresh_active_output_label();
            self.request_xdg_activation(qh);
            self.input_state.needs_redraw = true;
            return;
        }

        if self.layer_shell.is_none() {
            warn!("Output switch requested, but no supported shell is active");
            self.input_state.trigger_blocked_feedback();
            return;
        }

        info!("Switching layer overlay to {}", target_label);
        self.recreate_layer_surface_for_output(qh, &target_output);
        self.surface.set_current_output(target_output);
        self.refresh_active_output_label();
        self.set_keyboard_focus(false);
        self.set_overlay_ready(false);
        self.input_state.needs_redraw = true;
        self.sync_toolbar_visibility(qh);
    }

    fn recreate_layer_surface_for_output(
        &mut self,
        qh: &QueueHandle<Self>,
        output: &wl_output::WlOutput,
    ) {
        let Some(layer_shell) = self.layer_shell.as_ref() else {
            return;
        };

        let wl_surface = self.compositor_state.create_surface(qh);
        wl_surface.set_buffer_scale(self.surface.scale().max(1));
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            wl_surface,
            Layer::Top,
            Some("wayscriber"),
            Some(output),
        );

        layer_surface.set_anchor(Anchor::all());
        let desired_keyboard_mode = self.desired_keyboard_interactivity();
        layer_surface.set_keyboard_interactivity(desired_keyboard_mode);
        layer_surface.set_size(0, 0);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.commit();

        self.surface.set_layer_surface(layer_surface);
        self.set_current_keyboard_interactivity(Some(desired_keyboard_mode));
        self.buffer_damage.mark_all_full();
        self.set_toolbar_needs_recreate(true);
    }
}
