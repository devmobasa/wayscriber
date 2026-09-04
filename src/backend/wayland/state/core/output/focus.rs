use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn handle_output_focus_action(
        &mut self,
        qh: &QueueHandle<Self>,
        action: OutputFocusAction,
    ) {
        if !self.config.ui.multi_monitor_enabled {
            self.input_state.push_toast(
                ToastPriority::Info,
                "output",
                Toast::info("Multi-monitor focus is disabled (ui.multi_monitor_enabled=false)"),
            );
            self.input_state.trigger_blocked_feedback();
            return;
        }
        if self.capture.is_in_progress()
            || self.frozen.is_in_progress()
            || self.zoom.is_in_progress()
            || self.input_state.frozen_active()
            || self.input_state.zoom_active()
        {
            self.input_state.push_toast(
                ToastPriority::Info,
                "output",
                Toast::info(
                    "Cannot switch outputs while capture, frozen mode, or zoom mode is active",
                ),
            );
            self.input_state.trigger_blocked_feedback();
            return;
        }

        let outputs = self.sorted_known_outputs();
        if outputs.len() <= 1 {
            self.input_state.push_toast(
                ToastPriority::Info,
                "output",
                Toast::info("Only one output is available"),
            );
            self.input_state.trigger_blocked_feedback();
            return;
        }

        let surface_current_output = self.surface.current_output();
        let current_output = surface_current_output.or_else(|| self.preferred_fullscreen_output());
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
        let target_identity = self.output_identity_for(&target_output);

        if self.surface.is_xdg_window() {
            if !self.xdg_fullscreen() {
                self.input_state.push_toast(
                    ToastPriority::Info,
                    "output",
                    Toast::info("Enable fullscreen mode before switching outputs in this session."),
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
            self.focus.clear_surface_enter();
            self.refresh_active_output_label();
            self.begin_session_output_transition(target_identity, "output switch");
            self.request_xdg_activation(qh);
            self.input_state.needs_redraw = true;
            return;
        }

        if self.protocol.layer_shell().is_none() {
            warn!("Output switch requested, but no supported shell is active");
            self.input_state.trigger_blocked_feedback();
            return;
        }

        info!("Switching layer overlay to {}", target_label);
        self.teardown_keyboard_focus();
        self.recreate_layer_surface_for_output(qh, &target_output);
        self.surface.set_current_output(target_output);
        self.focus.clear_surface_enter();
        self.refresh_active_output_label();
        self.begin_session_output_transition(target_identity, "output switch");
        self.input_state.needs_redraw = true;
        self.sync_toolbar_visibility(qh);
    }

    fn recreate_layer_surface_for_output(
        &mut self,
        qh: &QueueHandle<Self>,
        output: &wl_output::WlOutput,
    ) {
        self.focus.begin_main_layer_acquisition();
        let Some(layer_shell) = self.protocol.layer_shell() else {
            return;
        };

        let wl_surface = self.protocol.compositor().create_surface(qh);
        wl_surface.set_buffer_scale(self.surface.scale().max(1));
        let layer = self.main_surface_layer();
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            wl_surface,
            layer,
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
        self.focus
            .set_keyboard_interactivity(Some(desired_keyboard_mode));
        self.force_sync_overlay_interactivity();
        self.buffer_damage
            .mark_all_full(FullDamageReason::LayerSurfaceRecreated);
        self.toolbar_chrome.set_needs_recreate(true);
    }
}
