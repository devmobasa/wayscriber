use super::*;
use crate::{
    input::InputState,
    onboarding::OnboardingState,
    ui::toolbar::model::{
        ToolbarBackendRoute, ToolbarEventPolicy, ToolbarPersistence, ToolbarPersistenceTarget,
        ToolbarPreApplyEffect, ToolbarRuntimeUiPersistenceTarget,
    },
};
use wayland_client::{Connection, QueueHandle};

mod persistence;
mod session;
pub(in crate::backend::wayland::state) use session::SessionFileDialogController;

#[cfg(test)]
use crate::ui::toolbar::model::{ToolbarConfigPersistenceTarget, ToolbarUiPersistenceTarget};
#[cfg(test)]
use persistence::{
    ToolbarPositions, apply_toolbar_config_target, apply_toolbar_ui_config_target,
    persisted_tool_preview_value, persisted_top_display_mode_value,
};
use session::populate_session_snapshot;

fn record_drawer_hint_shown(state: &mut OnboardingState) -> bool {
    if state.drawer_hint_count >= crate::onboarding::DRAWER_HINT_MAX {
        return false;
    }

    state.drawer_hint_count = state.drawer_hint_count.saturating_add(1);
    state.drawer_hint_shown = state.drawer_hint_count >= crate::onboarding::DRAWER_HINT_MAX;
    true
}

fn toolbar_event_blocked_by_modal(input_state: &InputState) -> bool {
    input_state.command_palette_is_engaged()
}

/// The top overflow menu is a plain flyout: any event other than the two menu
/// toggles dismisses it (selecting a dropped tool, an unrelated keybinding, etc.).
fn event_dismisses_top_overflow(event: &ToolbarEvent) -> bool {
    !matches!(
        event,
        ToolbarEvent::ToggleShapePicker(_) | ToolbarEvent::ToggleTopOverflow(_)
    )
}

/// The precise-entry popup dismisses on any toolbar interaction other
/// than its own open/commit/cancel events (mirroring the overflow flyout).
fn event_dismisses_precision_entry(event: &ToolbarEvent) -> bool {
    !matches!(
        event,
        ToolbarEvent::OpenPrecisionEntry(_)
            | ToolbarEvent::CommitPrecisionEntry { .. }
            | ToolbarEvent::CancelPrecisionEntry
    )
}

/// The Shapes popover dismisses on everything the overflow does *except* its own
/// inline options: the Fill checkbox and the polygon-sides stepper live inside
/// the popover, so using them must not close it out from under the pointer.
fn event_dismisses_shape_picker(event: &ToolbarEvent) -> bool {
    event_dismisses_top_overflow(event)
        && !matches!(
            event,
            ToolbarEvent::ToggleFill(_) | ToolbarEvent::NudgePolygonSides(_)
        )
}

/// Events shared by the overflow-anchored popovers that never dismiss them:
/// their own open/close toggles (mutual exclusion lives in the apply layer)
/// and the internal scrollbar.
fn event_spared_by_top_menu_popovers(event: &ToolbarEvent) -> bool {
    matches!(
        event,
        ToolbarEvent::ToggleSessionPopover(_)
            | ToolbarEvent::ToggleSettingsPopover(_)
            | ToolbarEvent::ToggleCanvasPopover(_)
            | ToolbarEvent::ScrollTopPopover(_)
    )
}

/// The Canvas popover hosts the board/page/zoom/advanced/step controls, so
/// those events must not close it out from under the pointer; everything
/// else dismisses it like the overflow flyout.
fn event_dismisses_canvas_popover(event: &ToolbarEvent) -> bool {
    !event_spared_by_top_menu_popovers(event)
        && !matches!(
            event,
            ToolbarEvent::BoardPrev
                | ToolbarEvent::BoardNext
                | ToolbarEvent::BoardNew
                | ToolbarEvent::BoardDuplicate
                | ToolbarEvent::BoardDelete
                | ToolbarEvent::PagePrev
                | ToolbarEvent::PageNext
                | ToolbarEvent::PageNew
                | ToolbarEvent::PageDuplicate
                | ToolbarEvent::PageDelete
                | ToolbarEvent::ZoomIn
                | ToolbarEvent::ZoomOut
                | ToolbarEvent::ResetZoom
                | ToolbarEvent::ToggleZoomLock
                | ToolbarEvent::UndoAll
                | ToolbarEvent::RedoAll
                | ToolbarEvent::UndoAllDelayed
                | ToolbarEvent::RedoAllDelayed
                | ToolbarEvent::ToggleFreeze
                | ToolbarEvent::ToggleCustomSection(_)
                | ToolbarEvent::ToggleDelaySliders(_)
                | ToolbarEvent::SetCustomUndoSteps(_)
                | ToolbarEvent::SetCustomRedoSteps(_)
                | ToolbarEvent::CustomUndo
                | ToolbarEvent::CustomRedo
                | ToolbarEvent::SetCustomUndoDelay(_)
                | ToolbarEvent::SetCustomRedoDelay(_)
                | ToolbarEvent::SetUndoDelay(_)
                | ToolbarEvent::SetRedoDelay(_)
        )
}

/// The Session popover hosts the session controls, so those events must not
/// close it out from under the pointer; everything else dismisses it like
/// the overflow flyout.
fn event_dismisses_session_popover(event: &ToolbarEvent) -> bool {
    !event_spared_by_top_menu_popovers(event)
        && !matches!(
            event,
            ToolbarEvent::OpenSession
                | ToolbarEvent::OpenRecentSession(_)
                | ToolbarEvent::SaveSessionAs
                | ToolbarEvent::SaveSessionAsConfirm(_)
                | ToolbarEvent::SaveSessionAsCancel
                | ToolbarEvent::SessionInfo
                | ToolbarEvent::ClearSession
                | ToolbarEvent::OpenConfigurator
        )
}

/// The Settings popover hosts the full Settings-pane control set (layout
/// mode, toggles, buttons, and the customization sub-panel), so all of
/// those events keep it open; everything else dismisses it.
fn event_dismisses_settings_popover(event: &ToolbarEvent) -> bool {
    !event_spared_by_top_menu_popovers(event)
        && !matches!(
            event,
            ToolbarEvent::SetToolbarLayoutMode(_)
                | ToolbarEvent::ToggleContextAwareUi(_)
                | ToolbarEvent::ToggleIconMode(_)
                | ToolbarEvent::ToggleTextControls(_)
                | ToolbarEvent::ToggleStatusBar(_)
                | ToolbarEvent::ToggleStatusBoardBadge(_)
                | ToolbarEvent::ToggleStatusPageBadge(_)
                | ToolbarEvent::ToggleFloatingBadgeAlways(_)
                | ToolbarEvent::TogglePresetToasts(_)
                | ToolbarEvent::TogglePresets(_)
                | ToolbarEvent::ToggleActionsSection(_)
                | ToolbarEvent::ToggleZoomActions(_)
                | ToolbarEvent::ToggleActionsAdvanced(_)
                | ToolbarEvent::ToggleBoardsSection(_)
                | ToolbarEvent::TogglePagesSection(_)
                | ToolbarEvent::ToggleStepSection(_)
                | ToolbarEvent::SetToolbarItemCustomizationOpen(_)
                | ToolbarEvent::SetToolbarItemCustomizationGroup(_)
                | ToolbarEvent::SetToolbarItemHidden(_, _)
                | ToolbarEvent::MoveToolbarItem { .. }
                | ToolbarEvent::StartToolbarItemDrag { .. }
                | ToolbarEvent::DragToolbarItemOver { .. }
                | ToolbarEvent::ResetToolbarItemOrder(_)
                | ToolbarEvent::ResetToolbarItemHiddenOverrides
                | ToolbarEvent::OpenCommandPalette
                | ToolbarEvent::OpenConfigurator
                | ToolbarEvent::OpenConfigFile
        )
}

impl WaylandState {
    /// Returns a snapshot of the current input state for toolbar UI consumption.
    pub(in crate::backend::wayland) fn toolbar_snapshot(&self) -> ToolbarSnapshot {
        let hints = ToolbarBindingHints::from_input_state(&self.input_state);
        let hint_max = crate::onboarding::DRAWER_HINT_MAX;
        let show_drawer_hint = self.onboarding.state().drawer_hint_count < hint_max
            && self.input_state.toolbar_side_pane == crate::ui::toolbar::SidePane::Draw;
        let mut snapshot =
            ToolbarSnapshot::from_input_with_options(&self.input_state, hints, show_drawer_hint);
        populate_session_snapshot(&mut snapshot, self.session.options());
        snapshot.side_viewport_max = self.side_pane_viewport_max(&snapshot);
        snapshot.top_viewport_max = self.top_strip_viewport_max(&snapshot);
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
        let base_x = self.inline_top_base_x(snapshot);
        super::geometry::remaining_top_width(screen_width, base_x, Self::TOP_MARGIN_RIGHT, scale)
    }

    /// Height available to the side palette in pre-scale spec units: the
    /// screen space below the palette's top edge, floored so a pathological
    /// drag offset cannot collapse the pane entirely.
    fn side_pane_viewport_max(&self, snapshot: &ToolbarSnapshot) -> Option<f64> {
        const MIN_SIDE_VIEWPORT: f64 = 180.0;
        let screen_height = self.surface.height() as f64;
        if screen_height <= 0.0 {
            return None;
        }
        let scale = if snapshot.toolbar_scale.is_finite() {
            snapshot.toolbar_scale.clamp(0.5, 3.0)
        } else {
            1.0
        };
        let side_top = Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset;
        let available = screen_height - side_top - Self::SIDE_MARGIN_BOTTOM;
        Some((available / scale).max(MIN_SIDE_VIEWPORT))
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
        if rebind_requested
            && let Some(action) = crate::ui::toolbar::model::action_for_event(&event)
        {
            self.input_state.begin_keybinding_capture(action);
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            return;
        }
        // Toolbar actions win over the modal sampler: cancel without sampling,
        // then apply the requested toolbar event normally.
        if self.input_state.is_precision_entry_open()
            && event_dismisses_precision_entry(&event)
            && self.input_state.cancel_precision_entry()
        {
            self.toolbar.mark_dirty();
        }
        let dismiss_overflow =
            self.input_state.toolbar_top_overflow_open && event_dismisses_top_overflow(&event);
        let dismiss_shapes =
            self.input_state.toolbar_shapes_expanded && event_dismisses_shape_picker(&event);
        let dismiss_session = self.input_state.toolbar_session_popover_open
            && event_dismisses_session_popover(&event);
        let dismiss_settings = self.input_state.toolbar_settings_popover_open
            && event_dismisses_settings_popover(&event);
        let dismiss_canvas =
            self.input_state.toolbar_canvas_popover_open && event_dismisses_canvas_popover(&event);
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

        let policy = ToolbarEventPolicy::for_event(&event);
        for effect in &policy.pre_apply_effects {
            match effect {
                ToolbarPreApplyEffect::RecordDrawerHintShown => {
                    if record_drawer_hint_shown(self.onboarding.state_mut()) {
                        self.onboarding.save();
                    }
                }
            }
        }

        match (&policy.backend_route, &event) {
            (ToolbarBackendRoute::MoveTopToolbar, ToolbarEvent::MoveTopToolbar { x, y }) => {
                let inline_active = self.inline_toolbars_active();
                let coord_is_screen = inline_active;
                drag_log(format!(
                    "toolbar move event: kind=Top, coord=({:.3}, {:.3}), coord_is_screen={}, inline_active={}",
                    *x, *y, coord_is_screen, inline_active
                ));
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
            (ToolbarBackendRoute::MoveSideToolbar, ToolbarEvent::MoveSideToolbar { x, y }) => {
                let inline_active = self.inline_toolbars_active();
                let coord_is_screen = inline_active;
                drag_log(format!(
                    "toolbar move event: kind=Side, coord=({:.3}, {:.3}), coord_is_screen={}, inline_active={}",
                    *x, *y, coord_is_screen, inline_active
                ));
                if !self.begin_toolbar_move_drag(MoveDragKind::Side, (*x, *y), coord_is_screen) {
                    return;
                }
                if coord_is_screen {
                    self.handle_toolbar_move_screen(MoveDragKind::Side, (*x, *y));
                } else {
                    self.handle_toolbar_move(MoveDragKind::Side, (*x, *y));
                }
                return;
            }
            (ToolbarBackendRoute::ApplyToInput, _)
            | (ToolbarBackendRoute::MoveTopToolbar, _)
            | (ToolbarBackendRoute::MoveSideToolbar, _) => {}
        }

        #[cfg(feature = "tablet-input")]
        let prev_thickness = self.input_state.current_thickness;
        #[cfg(feature = "tablet-input")]
        let thickness_event = policy.tablet_thickness_sensitive;

        let pane_switch = matches!(event, ToolbarEvent::SetSidePane(_));
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
            ToolbarPersistence::Ephemeral | ToolbarPersistence::Config(_) => None,
        };
        if starts_item_drag {
            let Some(ToolbarRuntimeUiPersistenceTarget::ItemOrder(group)) = runtime_target else {
                unreachable!("item drag start must carry its order-group runtime target");
            };
            if self
                .runtime_ui
                .as_mut()
                .is_some_and(|runtime| !runtime.begin_item_drag(group, &self.input_state))
            {
                return;
            }
        }
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

        let applied = self.input_state.apply_toolbar_event(event);
        if applied {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            if pane_switch {
                self.reset_side_toolbar_focus();
            }

            #[cfg(feature = "tablet-input")]
            if thickness_event && self.sync_stylus_thickness_cache(prev_thickness) {
                if self.stylus_tip_down {
                    self.record_stylus_peak(self.input_state.current_thickness);
                } else {
                    self.stylus_peak_thickness = None;
                }
            }

            match policy.persistence {
                ToolbarPersistence::Ephemeral | ToolbarPersistence::RuntimeUi(_) => {}
                ToolbarPersistence::Config(ToolbarPersistenceTarget::Toolbar(target)) => {
                    self.save_toolbar_config(target);
                }
                ToolbarPersistence::Config(ToolbarPersistenceTarget::Ui(target)) => {
                    self.save_toolbar_ui_config(target);
                }
                ToolbarPersistence::Config(ToolbarPersistenceTarget::History) => {
                    self.save_toolbar_history_config();
                }
                ToolbarPersistence::Config(ToolbarPersistenceTarget::ClickHighlight) => {
                    self.save_click_highlight_preferences();
                }
            }
        }
        if starts_item_drag && !applied {
            self.finish_toolbar_item_drag(false);
        }
        if let Some(prepared) = prepared_runtime
            && let Some(runtime) = self.runtime_ui.as_mut()
        {
            let finish = runtime.finish_toolbar_mutation(prepared, applied, &self.input_state);
            self.apply_toolbar_runtime_finish(finish);
        }
        if let Some(action) = self.input_state.take_pending_preset_action() {
            self.handle_preset_action(action);
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
