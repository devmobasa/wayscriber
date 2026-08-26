use super::*;
use crate::{
    backend::wayland::runtime_ui_state::ToolbarRuntimeFinish,
    input::InputState,
    ui::toolbar::model::{
        ToolbarBackendRoute, ToolbarEventPolicy, ToolbarPersistence, ToolbarPopover,
        ToolbarRuntimeUiPersistenceTarget, popovers_for_event,
    },
};
use wayland_client::{Connection, QueueHandle};

mod feedback;
mod presets;
pub(in crate::backend::wayland) use presets::queue_preset_action;
mod quick_colors;
pub(in crate::backend::wayland) use quick_colors::queue_quick_color_edit;
mod session;
pub(in crate::backend::wayland::state) use session::SessionFileDialogController;

use feedback::{ToolbarPinChange, pin_durability};
use session::populate_session_snapshot;

fn toolbar_event_blocked_by_modal(input_state: &InputState) -> bool {
    input_state.command_palette_is_engaged()
}

fn finalize_pointer_gestures_before_toolbar_dispatch(
    input_state: &mut InputState,
    spotlight_wheel_idle_deadline: &mut Option<std::time::Instant>,
) {
    // Several backend-owned toolbar routes return before
    // `InputState::apply_toolbar_event`, including session operations. Close
    // the burst at the shared dispatch boundary so a save/open persistence
    // barrier sees both the changed factor and its undo history, and so no
    // already-finished gesture leaves an idle wake behind.
    input_state.flush_spotlight_magnification_gesture();
    *spotlight_wheel_idle_deadline = None;
    // A held bend handle closes here too, and for the sharper version of the
    // same reason: a session open or clear replaces the frame the gesture's
    // snapshot belongs to, and shape ids restart per frame, so a bend flushed
    // afterwards would attach to an unrelated shape on the new page.
    input_state.finish_active_arrow_bend();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolbarEventPreflight {
    Continue,
    RebindCaptured,
}

fn handle_toolbar_event_preflight(
    input_state: &mut InputState,
    spotlight_wheel_idle_deadline: &mut Option<std::time::Instant>,
    event: &ToolbarEvent,
    rebind_requested: bool,
) -> ToolbarEventPreflight {
    // Rebind capture returns before every later backend and InputState route,
    // so gesture finalization belongs ahead of that branch. This also covers
    // GTK and secondary-device clicks, which enter with the rebind decision
    // already resolved.
    finalize_pointer_gestures_before_toolbar_dispatch(input_state, spotlight_wheel_idle_deadline);

    if rebind_requested && let Some(action) = crate::ui::toolbar::model::action_for_event(event) {
        input_state.begin_keybinding_capture(action);
        return ToolbarEventPreflight::RebindCaptured;
    }

    ToolbarEventPreflight::Continue
}

/// Whether `event` dismisses `popover`.
///
/// Every popover here is a flyout: anything that is not part of it closes it.
/// "Part of it" is declared once, as the event's owning popover in
/// `ToolbarEventPolicy`, so a new control cannot be added to a popover and
/// forgotten in that popover's dismissal rule - which used to close it out
/// from under the pointer the first time the control was used.
///
/// The three overflow-anchored menus additionally spare each other's toggles
/// and the shared scrollbar: switching between them is one gesture, and the
/// switch itself is what closes the previous one.
fn event_dismisses_popover(event: &ToolbarEvent, popover: ToolbarPopover) -> bool {
    !popovers_for_event(event).contains(&popover)
}

impl WaylandState {
    /// Returns a snapshot of the current input state for toolbar UI consumption.
    pub(in crate::backend::wayland) fn toolbar_snapshot(&self) -> ToolbarSnapshot {
        let hints = ToolbarBindingHints::from_input_state(&self.input_state);
        let mut snapshot = ToolbarSnapshot::from_input_with_bindings(&self.input_state, hints);
        // Resolved here rather than read from the last rendered frame: a
        // toolbar snapshot is built between canvas renders, and before the
        // first one, so a published value would lag or not exist yet.
        snapshot.spotlight_magnifier_source = Some(self.current_spotlight_magnifier_source());
        populate_session_snapshot(&mut snapshot, self.session.options());
        snapshot.runtime_ui_persistence = self
            .runtime_ui
            .as_ref()
            .map(|runtime| runtime.persistence_snapshot())
            .or_else(|| self.runtime_ui_unavailable.clone());
        snapshot.top_viewport_max = self.top_strip_viewport_max(&snapshot);
        snapshot.top_available_height = self.top_popover_available_height(&snapshot);
        snapshot.top_fade = self.data.top_strip_fade.value();
        snapshot
    }

    /// Width available to the top strip in pre-scale spec units; content
    /// past this degrades into the overflow menu instead of clipping off
    /// the screen. Both inline and layer-shell placement use the pushed top
    /// base X, so budgeting must subtract that same position.
    fn top_strip_viewport_max(&self, snapshot: &ToolbarSnapshot) -> Option<f64> {
        let screen_width = self.surface.width() as f64;
        let scale = if snapshot.toolbar_scale.is_finite() {
            snapshot.toolbar_scale.clamp(0.5, 3.0)
        } else {
            1.0
        };
        let base_x = self.inline_top_base_x();
        super::geometry::remaining_top_width(screen_width, base_x, Self::TOP_MARGIN_RIGHT, scale)
    }

    /// Height available from the top toolbar surface origin to the output
    /// bottom, in pre-scale spec units, for sizing the open
    /// Canvas/Session/Settings popover (see
    /// `ToolbarSnapshot::top_available_height`). The surface origin includes
    /// the live vertical drag offset, so opening a tall popover does not make
    /// the toolbar clamp back up the screen.
    fn top_popover_available_height(&self, snapshot: &ToolbarSnapshot) -> Option<f64> {
        let screen_height = self.surface.height() as f64;
        let scale = if snapshot.toolbar_scale.is_finite() {
            snapshot.toolbar_scale.clamp(0.5, 3.0)
        } else {
            1.0
        };
        let surface_y = self.inline_top_base_y() + self.data.toolbar_top_offset_y;
        super::geometry::remaining_top_height(screen_height, surface_y, scale)
    }

    /// Applies an incoming toolbar event and schedules redraws as needed.
    pub(in crate::backend::wayland) fn handle_toolbar_event(
        &mut self,
        event: ToolbarEvent,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) {
        let rebind_requested = self.config.ui.toolbar.rebind_modifier.matches(
            self.input_state.modifiers.ctrl,
            self.input_state.modifiers.shift,
            self.input_state.modifiers.alt,
        );
        // Built-in press resolution: Shift+click on Clear skips the undo
        // toast. (GTK resolves the same upgrade from its own click-time
        // modifier capture before the event reaches the bridge.)
        let event = match event {
            ToolbarEvent::ClearCanvas { instant } => ToolbarEvent::ClearCanvas {
                instant: instant || self.input_state.modifiers.shift,
            },
            other => other,
        };
        // Built-in toolbar events mutate visuals on the main surface when the
        // toolbars are inline. Refresh every SHM slot even for early-returning
        // event paths (popover dismissal, rebind capture, session actions) so
        // slot rotation cannot restore stale toolbar pixels.
        if self.inline_toolbars_active() {
            self.mark_inline_toolbar_full_damage();
        }
        self.handle_toolbar_event_with_rebind(event, rebind_requested, conn, qh);
    }

    pub(in crate::backend::wayland) fn handle_toolbar_event_with_rebind(
        &mut self,
        event: ToolbarEvent,
        rebind_requested: bool,
        conn: Option<&Connection>,
        qh: Option<&QueueHandle<Self>>,
    ) {
        // GTK toolbar feedback bypasses the built-in pointer modal gate, so
        // enforce the same rule in the shared event path as well.
        if toolbar_event_blocked_by_modal(&self.input_state) {
            return;
        }
        // A toolbar interaction replaces the modal sampler. Do this before
        // shortcut capture so the capture modal owns subsequent keys.
        self.cancel_eyedropper();
        self.cancel_region_for_toolbar_interaction();
        if handle_toolbar_event_preflight(
            &mut self.input_state,
            &mut self.spotlight_wheel_idle_deadline,
            &event,
            rebind_requested,
        ) == ToolbarEventPreflight::RebindCaptured
        {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            return;
        }
        // Toolbar actions win over the modal sampler: cancel without sampling,
        // then apply the requested toolbar event normally.
        if self.input_state.is_precision_entry_open()
            && event_dismisses_popover(&event, ToolbarPopover::PrecisionEntry)
            && self.input_state.cancel_precision_entry()
        {
            self.toolbar.mark_dirty();
        }
        let dismiss_overflow = self.input_state.toolbar_top_overflow_open
            && event_dismisses_popover(&event, ToolbarPopover::TopOverflow);
        let dismiss_shapes = self.input_state.toolbar_shapes_expanded
            && event_dismisses_popover(&event, ToolbarPopover::ShapePicker);
        let dismiss_session = self.input_state.toolbar_session_popover_open
            && event_dismisses_popover(&event, ToolbarPopover::Session);
        let dismiss_settings = self.input_state.toolbar_settings_popover_open
            && event_dismisses_popover(&event, ToolbarPopover::Settings);
        let dismiss_canvas = self.input_state.toolbar_canvas_popover_open
            && event_dismisses_popover(&event, ToolbarPopover::Canvas);
        if dismiss_overflow
            || dismiss_shapes
            || dismiss_session
            || dismiss_settings
            || dismiss_canvas
        {
            if dismiss_overflow {
                self.input_state.toolbar_top_overflow_open = false;
            }
            if dismiss_shapes {
                self.input_state.toolbar_shapes_expanded = false;
            }
            if dismiss_session {
                self.input_state.toolbar_session_popover_open = false;
            }
            if dismiss_settings {
                self.input_state.toolbar_settings_popover_open = false;
            }
            if dismiss_canvas {
                self.input_state.toolbar_canvas_popover_open = false;
            }
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
        }
        if self.handle_toolbar_session_event(&event, conn, qh) {
            return;
        }
        let persistence_lifecycle_handled = self
            .runtime_ui
            .as_mut()
            .is_some_and(|runtime| runtime.handle_persistence_lifecycle_event(&event));
        if persistence_lifecycle_handled {
            // Read-only recovery cancellation can terminalize synchronously
            // and install a seed reload that was staged behind the barrier.
            // Consume that rebuild in this dispatch instead of waiting for a
            // writer wake that may never arrive.
            self.drain_runtime_ui_completions();
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            return;
        }

        let policy = ToolbarEventPolicy::for_event(&event);

        match (&policy.backend_route, &event) {
            (ToolbarBackendRoute::MoveTopToolbar, ToolbarEvent::MoveTopToolbar { x, y }) => {
                let inline_active = self.inline_toolbars_active();
                let coord_is_screen = inline_active;
                drag_log(|| {
                    format!(
                        "toolbar move event: kind=Top, coord=({:.3}, {:.3}), coord_is_screen={}, inline_active={}",
                        *x, *y, coord_is_screen, inline_active
                    )
                });
                if !self.begin_toolbar_move_drag(MoveDragKind::Top, (*x, *y), coord_is_screen) {
                    return;
                }
                if coord_is_screen {
                    self.handle_toolbar_move_screen(MoveDragKind::Top, (*x, *y));
                } else {
                    self.handle_toolbar_move(MoveDragKind::Top, (*x, *y));
                }
                return;
            }
            (ToolbarBackendRoute::ApplyToInput, _) | (ToolbarBackendRoute::MoveTopToolbar, _) => {}
        }

        #[cfg(feature = "tablet-input")]
        let prev_thickness = self.input_state.current_thickness;
        #[cfg(feature = "tablet-input")]
        let thickness_event = policy.tablet_thickness_sensitive;

        let starts_item_drag = matches!(event, ToolbarEvent::StartToolbarItemDrag { .. });
        if matches!(event, ToolbarEvent::DragToolbarItemOver { .. })
            && !self.toolbar_item_drag_update_allowed()
        {
            // Keep the active preview unchanged. Its real release/cancel still
            // flows through the barrier-aware finish path exactly once, while
            // a same-authority barrier failure may resume this untouched drag.
            return;
        }
        let runtime_target = match policy.persistence {
            ToolbarPersistence::RuntimeUi(target) => Some(target),
            ToolbarPersistence::Ephemeral => None,
        };
        // Classified before the apply consumes the event; the effective config
        // is updated from the runtime state the apply leaves behind.
        if starts_item_drag {
            // The pairing lives in `persistence_for_event`, so a drag-start
            // whose policy stopped naming an order group is metadata drift,
            // not an impossible state. Refusing the drag keeps the toolbar
            // usable; panicking here took the whole overlay down with it.
            let Some(ToolbarRuntimeUiPersistenceTarget::ItemOrder(group)) = runtime_target else {
                log::error!(
                    "Ignoring a toolbar item drag: {event:?} starts one but its policy names no \
                     order group to persist it under"
                );
                return;
            };
            if !self.begin_toolbar_item_drag_preview(group) {
                return;
            }
        }
        let pin_change = ToolbarPinChange::from_event(&event);
        let prepared_runtime = if starts_item_drag {
            None
        } else if let Some(target) = runtime_target {
            match self.runtime_ui.as_ref() {
                Some(runtime) => match runtime.begin_toolbar_mutation(target, &self.input_state) {
                    Some(prepared) => Some(prepared),
                    None => return,
                },
                None => None,
            }
        } else {
            None
        };
        let pin_durability = pin_durability(prepared_runtime.as_ref());

        let applied = self.input_state.apply_toolbar_event(event);
        if applied {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;

            #[cfg(feature = "tablet-input")]
            if thickness_event && self.sync_stylus_thickness_cache(prev_thickness) {
                if self.stylus_tip_down {
                    self.record_stylus_peak(self.input_state.current_thickness);
                } else {
                    self.stylus_peak_thickness = None;
                }
            }

            // The reader thread follows the HUD's live state, whatever moved it.
            if runtime_target == Some(ToolbarRuntimeUiPersistenceTarget::InputHud) {
                self.sync_input_monitor();
            }
        }
        if starts_item_drag && !applied {
            self.finish_toolbar_item_drag(false);
        }
        let mut pin_confirmation_allowed = applied && prepared_runtime.is_none();
        if let Some(prepared) = prepared_runtime
            && let Some(runtime) = self.runtime_ui.as_mut()
        {
            let finish = runtime.finish_toolbar_mutation(prepared, applied, &self.input_state);
            pin_confirmation_allowed =
                applied && matches!(finish, ToolbarRuntimeFinish::KeepPreview);
            self.apply_toolbar_runtime_finish(finish);
        }
        if pin_confirmation_allowed && let Some(pin_change) = pin_change {
            pin_change.notify(&mut self.input_state, pin_durability);
        }
        if let Some(action) = self.input_state.take_pending_preset_action() {
            self.handle_preset_action(action);
        }
        if let Some(edit) = self.input_state.take_pending_quick_color_edit() {
            self.handle_quick_color_edit(edit);
        }
        if let Some(color) = self.input_state.take_pending_copy_hex_request() {
            self.handle_copy_hex_color(color);
        }
        if let Some(target) = self.input_state.take_pending_paste_hex_request() {
            self.handle_paste_hex_color(target);
        }
        self.drain_clipboard_requests();
        self.refresh_keyboard_interactivity();
    }

    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn sync_stylus_thickness_cache(&mut self, prev: f64) -> bool {
        let cur = self.input_state.current_thickness;
        if (cur - prev).abs() <= f64::EPSILON {
            return false;
        }

        self.stylus_base_thickness = Some(cur);
        if self.stylus_tip_down {
            self.stylus_pressure_thickness = Some(cur);
        } else {
            self.stylus_pressure_thickness = None;
        }
        true
    }

    /// Records the maximum stylus thickness seen during the current stroke.
    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn record_stylus_peak(&mut self, thickness: f64) {
        self.stylus_peak_thickness = Some(
            self.stylus_peak_thickness
                .map_or(thickness, |p| p.max(thickness)),
        );
    }
}

#[cfg(test)]
mod tests;
