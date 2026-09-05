use super::*;
use crate::env_vars::{XDG_CURRENT_DESKTOP_ENV, XDG_SESSION_DESKTOP_ENV};
use smithay_client_toolkit::shell::WaylandSurface;

fn toolbar_visibility_for_frontend(
    requested: bool,
    gtk_active: bool,
    gtk_drag_preview: Option<crate::toolbar_gtk::GtkToolbarKind>,
    capture_picker_suppressed: bool,
) -> bool {
    if capture_picker_suppressed {
        return false;
    }
    if !gtk_active {
        return requested;
    }
    requested && gtk_drag_preview == Some(crate::toolbar_gtk::GtkToolbarKind::Top)
}

impl WaylandState {
    pub(in crate::backend::wayland) fn desired_keyboard_interactivity(
        &self,
    ) -> KeyboardInteractivity {
        // GTK bars count as visible layer toolbars: the canvas must drop
        // from Exclusive to OnDemand while they are mapped, or compositors
        // that honor exclusivity (Hyprland) lock all input to the canvas
        // and the bars become click-through.
        let toolbar_visible = !self.capture_picker_chrome_suppressed()
            && (self.toolbar.is_visible()
                || (self.gtk_toolbars_active() && self.input_state.toolbar_top_visible()));
        keyboard_interactivity_for(KeyboardInteractivityPolicyInput {
            keyboard_release_requested: self.overlay_keyboard_passthrough_requested(),
            main_layer_focus_acquiring: self.focus.main_layer_acquiring(),
            layer_shell_available: self.protocol.layer_shell().is_some(),
            separate_toolbar_visible: toolbar_visible,
            inline_toolbars_active: self.toolbar_chrome.inline_toolbars(),
            canvas_modal_active: self.input_state.is_color_picker_popup_open(),
        })
    }

    fn log_toolbar_layer_shell_missing_once(&mut self) {
        if !self.toolbar_chrome.note_layer_shell_missing() {
            return;
        }

        let desktop_env =
            std::env::var(XDG_CURRENT_DESKTOP_ENV).unwrap_or_else(|_| "unknown".into());
        let session_env =
            std::env::var(XDG_SESSION_DESKTOP_ENV).unwrap_or_else(|_| "unknown".into());
        log::info!(
            "Layer-shell protocol unavailable; toolbar surfaces will not appear (desktop='{}', session='{}'). Overlay may be limited to the work area on compositors like GNOME.",
            desktop_env,
            session_env
        );
    }

    /// Applies and commits keyboard interactivity when the desired mode changes.
    pub(in crate::backend::wayland) fn refresh_keyboard_interactivity(&mut self) {
        let desired = self.desired_keyboard_interactivity();
        let current = self.focus.current_keyboard_interactivity();

        let updated = if let Some(layer) = self.surface.layer_surface_mut() {
            if current != Some(desired) {
                layer.set_keyboard_interactivity(desired);
                layer.commit();
                true
            } else {
                false
            }
        } else {
            self.focus.set_keyboard_interactivity(None);
            return;
        };

        if updated {
            self.focus.set_keyboard_interactivity(Some(desired));
        }
    }

    /// Syncs toolbar visibility from the input state, ensures surfaces exist, and adjusts keyboard interactivity.
    pub(in crate::backend::wayland) fn sync_toolbar_visibility(&mut self, qh: &QueueHandle<Self>) {
        // Sync individual toolbar visibility. While the GTK frontend owns
        // the toolbars, the built-in surfaces stay unmapped.
        let gtk_active = self.gtk_toolbars_active();
        let top_visible = toolbar_visibility_for_frontend(
            self.input_state.toolbar_top_visible(),
            gtk_active,
            self.toolbar_drag.gtk_preview_kind(),
            self.capture_picker_chrome_suppressed(),
        );
        let inline_active = self.toolbar_chrome.inline_toolbars();
        let drag_preview =
            self.toolbar_drag.preview_active() || self.toolbar_drag.gtk_preview_kind().is_some();

        if top_visible != self.toolbar.is_top_visible() {
            self.toolbar.set_top_visible(top_visible);
            self.input_state.needs_redraw = true;
        }

        let any_visible = self.toolbar.is_visible();
        if !any_visible {
            self.toolbar_chrome.set_pointer_over_toolbar(false);
            self.toolbar_chrome.reset_configure_misses();
            self.toolbar_chrome.reset_margins();
            self.clear_toolbar_focus();
        }

        if any_visible {
            log::debug!(
                "Toolbar visibility sync: top_visible={}, layer_shell_available={}, inline_active={}, top_created={}, needs_recreate={}, scale={}",
                top_visible,
                self.protocol.layer_shell().is_some(),
                inline_active,
                self.toolbar.top_created(),
                self.toolbar_chrome.needs_recreate(),
                self.surface.scale()
            );
            drag_log(|| {
                format!(
                    "toolbar sync: top_offset=({}, {}), inline_active={}, layer_shell={}, needs_recreate={}",
                    self.toolbar_chrome.top_offset().0,
                    self.toolbar_chrome.top_offset().1,
                    inline_active,
                    self.protocol.layer_shell().is_some(),
                    self.toolbar_chrome.needs_recreate()
                )
            });
        }

        // Warn the user when layer-shell is unavailable and we're forced to inline fallback.
        if any_visible && self.protocol.layer_shell().is_none() {
            self.log_toolbar_layer_shell_missing_once();
        }

        if any_visible && inline_active {
            // If we forced inline while layer surfaces already existed, tear them down to avoid
            // focus/input conflicts on compositors that support layer-shell.
            if self.toolbar.top_created() {
                self.toolbar.destroy_all();
                self.toolbar_chrome.set_needs_recreate(true);
                self.toolbar_chrome.reset_margins();
            }
            self.toolbar_chrome.reset_configure_misses();
        }

        if any_visible && self.protocol.layer_shell().is_some() && !inline_active && !drag_preview {
            // Detect compositors ignoring or failing to configure toolbar layer surfaces; if they
            // never configure after repeated attempts, fall back to inline toolbars automatically.
            let top_configured = self.toolbar.top_configured();
            let expected_top = self.toolbar.is_top_visible();
            let verdict = self
                .toolbar_chrome
                .note_configure_result(!expected_top || top_configured);
            if verdict == ConfigureVerdict::StillWaiting {
                let misses = self.toolbar_chrome.configure_miss_count();
                if debug_toolbar_drag_logging_enabled() && misses.is_multiple_of(60) {
                    debug!(
                        "Toolbar configure pending: count={}, expected_top={}, configured_top={}",
                        misses, expected_top, top_configured
                    );
                }
            }

            if verdict == ConfigureVerdict::FallBackToInline {
                warn!(
                    "Toolbar layer surface did not configure after repeated frames; falling back to the inline toolbar"
                );
                self.toolbar.destroy_all();
                self.toolbar_chrome.reset_margins();
                self.toolbar_chrome.set_needs_recreate(true);
                // Re-run visibility sync with inline mode enabled.
                self.sync_toolbar_visibility(qh);
                return;
            }

            if self.toolbar_chrome.needs_recreate() {
                self.toolbar.destroy_all();
                self.toolbar_chrome.set_needs_recreate(false);
                self.toolbar_chrome.reset_margins();
            }
            let snapshot = self.toolbar_snapshot();
            if !self.toolbar_drag.is_moving() {
                let _ = self.apply_toolbar_offsets(&snapshot);
            }
            if let Some(layer_shell) = self.protocol.layer_shell() {
                let scale = self.surface.scale();
                let output = self.surface.current_output();
                self.toolbar.ensure_created(
                    self.render.ui_text(),
                    qh,
                    self.protocol.compositor(),
                    layer_shell,
                    scale,
                    output.as_ref(),
                    &snapshot,
                );
            }
        }

        if !any_visible {
            self.toolbar_chrome.clear_inline_hits();
            self.toolbar_chrome.clear_inline_hover();
        }

        self.refresh_keyboard_interactivity();
    }

    pub(in crate::backend::wayland) fn render_toolbars(&mut self, snapshot: &ToolbarSnapshot) {
        if !self.toolbar.is_visible() {
            return;
        }

        // No hover tracking yet; pass None. Can be updated when we record pointer positions per surface.
        let render_profile = self.input_state.active_ui_render_profile().cloned();
        self.toolbar.render(
            self.render.ui_text(),
            self.protocol.shm(),
            snapshot,
            None,
            render_profile.as_ref(),
        );
        self.toolbar.apply_input_regions(self.protocol.compositor());
    }

    pub(in crate::backend::wayland) fn render_layer_toolbars_if_needed(&mut self) {
        if !self.toolbar.is_visible() {
            return;
        }
        if self.inline_toolbars_render_active() && !self.toolbar.is_suppressed() {
            return;
        }

        let snapshot = self.toolbar_snapshot();
        let changed = self.toolbar.update_snapshot(&snapshot);
        if changed {
            self.toolbar.mark_dirty();
        }
        if changed || self.toolbar.needs_render() {
            self.render_toolbars(&snapshot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolbar_gtk::GtkToolbarKind;

    #[test]
    fn gtk_frontend_maps_only_the_inline_preview_target() {
        assert!(!toolbar_visibility_for_frontend(true, true, None, false));
        assert!(toolbar_visibility_for_frontend(
            true,
            true,
            Some(GtkToolbarKind::Top),
            false,
        ));
    }

    #[test]
    fn builtin_frontend_ignores_gtk_preview_state() {
        assert!(toolbar_visibility_for_frontend(true, false, None, false));
    }

    #[test]
    fn capture_picker_suppresses_every_toolbar_frontend_without_changing_request() {
        assert!(!toolbar_visibility_for_frontend(true, false, None, true));
        assert!(!toolbar_visibility_for_frontend(
            true,
            true,
            Some(GtkToolbarKind::Top),
            true,
        ));
        assert!(toolbar_visibility_for_frontend(true, false, None, false));
    }
}
