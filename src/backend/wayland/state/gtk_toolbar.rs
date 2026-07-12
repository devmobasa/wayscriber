//! Backend-side glue for the GTK toolbar frontend: spawn decision,
//! feedback draining, and state pushes. See `crate::toolbar_gtk` for the
//! threading model.

use std::time::Duration;

use wayland_client::{Connection, QueueHandle};

use super::WaylandState;
use crate::toolbar_gtk::select::{
    GtkPreconditions, ToolbarFrontend, requested_backend, resolve_frontend,
};
use crate::toolbar_gtk::{GtkToolbarBridge, GtkToolbarFeedback, GtkToolbarUpdate};

impl WaylandState {
    /// Bounds the main poll while the GTK thread can produce feedback,
    /// since nothing it sends wakes the Wayland fd.
    const GTK_TOOLBAR_WAKE_INTERVAL: Duration = Duration::from_millis(25);

    /// True while the GTK frontend owns the toolbars (built-in bars stay
    /// unmapped).
    pub(in crate::backend::wayland) fn gtk_toolbars_active(&self) -> bool {
        self.gtk_toolbar.is_some()
    }

    /// Spawns the GTK toolbar thread when the resolved frontend is GTK.
    pub(in crate::backend::wayland) fn spawn_gtk_toolbar_if_selected(&mut self) {
        let request = requested_backend(&self.config);
        let preconditions = GtkPreconditions {
            feature_compiled: cfg!(feature = "toolbar-gtk"),
            layer_shell: self.layer_shell.is_some(),
            force_inline: super::force_inline_toolbars_requested(&self.config),
            main_surface_uses_overlay_layer: self.data.main_surface_uses_overlay_layer,
        };
        match resolve_frontend(request, preconditions) {
            ToolbarFrontend::Gtk => {
                self.gtk_toolbar = GtkToolbarBridge::spawn();
                if self.gtk_toolbar.is_some() {
                    log::info!("GTK toolbars enabled; built-in toolbar surfaces stay unmapped");
                } else {
                    log::warn!("GTK toolbar thread failed to start; using built-in toolbars");
                }
            }
            ToolbarFrontend::Builtin(blocker) => {
                if let Some(reason) = blocker {
                    log::warn!(
                        "GTK toolbars requested but unavailable ({}); using built-in toolbars",
                        reason.describe()
                    );
                }
            }
        }
    }

    /// Wake interval for the event-loop timeout chain; `None` when the GTK
    /// frontend is inactive.
    pub(in crate::backend::wayland) fn gtk_toolbar_wake_timeout(&self) -> Option<Duration> {
        self.gtk_toolbar
            .as_ref()
            .map(|_| Self::GTK_TOOLBAR_WAKE_INTERVAL)
    }

    /// Drains pending GTK toolbar feedback into the shared toolbar-event
    /// path, and falls back to the built-in bars if the GTK thread died.
    pub(in crate::backend::wayland) fn process_gtk_toolbar(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if self
            .gtk_toolbar
            .as_ref()
            .is_some_and(GtkToolbarBridge::failed)
        {
            log::warn!("GTK toolbar thread failed; falling back to built-in toolbars");
            self.gtk_toolbar = None;
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            return;
        }
        let Some(bridge) = self.gtk_toolbar.as_ref() else {
            return;
        };
        let mut pending = Vec::new();
        while let Some(feedback) = bridge.try_recv_feedback() {
            pending.push(feedback);
        }
        for feedback in pending {
            match feedback {
                GtkToolbarFeedback::Event {
                    event,
                    rebind_requested,
                } => {
                    self.handle_toolbar_event_with_rebind(
                        event,
                        rebind_requested,
                        Some(conn),
                        Some(qh),
                    );
                }
                GtkToolbarFeedback::SetTopOffset { x, y, seq, done } => {
                    self.data.gtk_top_offset_seq = seq;
                    self.apply_gtk_top_offset(x, y, done);
                }
                GtkToolbarFeedback::SetSideOffset { x, y, seq, done } => {
                    self.data.gtk_side_offset_seq = seq;
                    self.apply_gtk_side_offset(x, y, done);
                }
            }
        }
    }

    /// Pushes the current toolbar state to the GTK thread; the bridge
    /// deduplicates unchanged updates.
    pub(in crate::backend::wayland) fn push_gtk_toolbar_update(&mut self) {
        if self.gtk_toolbar.is_none() {
            return;
        }
        let snapshot = self.toolbar_snapshot();
        // Mirror the built-in suppression behavior: while the overlay is
        // suppressed (capture, freeze, zoom, external dialog) or in light
        // passthrough, the bars must unmap so they neither appear in
        // captures nor swallow clicks.
        let suppressed = self.toolbar.is_suppressed() || self.overlay_passthrough_requested();
        let update = GtkToolbarUpdate {
            top_visible: self.input_state.toolbar_top_visible() && !suppressed,
            side_visible: self.input_state.toolbar_side_visible() && !suppressed,
            top_offset: (self.data.toolbar_top_offset, self.data.toolbar_top_offset_y),
            side_offset: (
                self.data.toolbar_side_offset_x,
                self.data.toolbar_side_offset,
            ),
            top_offset_seq: self.data.gtk_top_offset_seq,
            side_offset_seq: self.data.gtk_side_offset_seq,
            top_base_x: self.gtk_top_base_x(&snapshot),
            output_name: self
                .surface
                .current_output()
                .and_then(|output| self.output_state.info(&output))
                .and_then(|info| info.name),
            rebind_modifier: self.config.ui.toolbar.rebind_modifier,
            rebind_modifier_active: self.config.ui.toolbar.rebind_modifier.matches(
                self.input_state.modifiers.ctrl,
                self.input_state.modifiers.shift,
                self.input_state.modifiers.alt,
            ),
            snapshot,
        };
        if let Some(bridge) = self.gtk_toolbar.as_mut() {
            bridge.maybe_send(update);
        }
    }
}
