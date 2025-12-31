use crate::config::ToolbarLayoutMode;
use crate::draw::PageDeleteOutcome;
use crate::input::state::UiToastKind;
use crate::input::{InputState, ZoomAction};

use super::events::ToolbarEvent;

impl InputState {
    /// Applies a toolbar-originated event to the input state.
    ///
    /// Returns true if the event resulted in a state change.
    pub fn apply_toolbar_event(&mut self, event: ToolbarEvent) -> bool {
        match event {
            ToolbarEvent::SelectTool(tool) => {
                if matches!(self.state, crate::input::DrawingState::TextInput { .. }) {
                    self.cancel_text_input();
                }
                let mut changed = self.set_tool_override(Some(tool));
                if self.toolbar_layout_mode == ToolbarLayoutMode::Simple
                    && self.toolbar_shapes_expanded
                {
                    self.toolbar_shapes_expanded = false;
                    changed = true;
                }
                changed
            }
            ToolbarEvent::SetColor(color) => self.set_color(color),
            ToolbarEvent::SetThickness(value) => self.set_thickness_for_active_tool(value),
            ToolbarEvent::SetMarkerOpacity(value) => self.set_marker_opacity(value),
            ToolbarEvent::SetEraserMode(mode) => self.set_eraser_mode(mode),
            ToolbarEvent::SetFont(descriptor) => self.set_font_descriptor(descriptor),
            ToolbarEvent::SetFontSize(size) => self.set_font_size(size),
            ToolbarEvent::ToggleFill(enable) => self.set_fill_enabled(enable),
            ToolbarEvent::SetUndoDelay(delay_secs) => {
                let min_delay_s = 0.05;
                let clamped_ms = (delay_secs.clamp(min_delay_s, 5.0) * 1000.0).round();
                self.undo_all_delay_ms = clamped_ms as u64;
                true
            }
            ToolbarEvent::SetRedoDelay(delay_secs) => {
                let min_delay_s = 0.05;
                let clamped_ms = (delay_secs.clamp(min_delay_s, 5.0) * 1000.0).round();
                self.redo_all_delay_ms = clamped_ms as u64;
                true
            }
            ToolbarEvent::SetCustomUndoDelay(delay_secs) => {
                let min_delay_s = 0.05;
                let clamped_ms = (delay_secs.clamp(min_delay_s, 5.0) * 1000.0).round();
                self.custom_undo_delay_ms = clamped_ms as u64;
                true
            }
            ToolbarEvent::SetCustomRedoDelay(delay_secs) => {
                let min_delay_s = 0.05;
                let clamped_ms = (delay_secs.clamp(min_delay_s, 5.0) * 1000.0).round();
                self.custom_redo_delay_ms = clamped_ms as u64;
                true
            }
            ToolbarEvent::SetCustomUndoSteps(steps) => {
                let clamped = steps.clamp(1, 500);
                if self.custom_undo_steps != clamped {
                    self.custom_undo_steps = clamped;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::SetCustomRedoSteps(steps) => {
                let clamped = steps.clamp(1, 500);
                if self.custom_redo_steps != clamped {
                    self.custom_redo_steps = clamped;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::NudgeThickness(delta) => self.nudge_thickness_for_active_tool(delta),
            ToolbarEvent::NudgeMarkerOpacity(delta) => {
                self.set_marker_opacity(self.marker_opacity + delta)
            }
            ToolbarEvent::Undo => {
                self.toolbar_undo();
                true
            }
            ToolbarEvent::Redo => {
                self.toolbar_redo();
                true
            }
            ToolbarEvent::UndoAll => {
                self.undo_all_immediate();
                true
            }
            ToolbarEvent::RedoAll => {
                self.redo_all_immediate();
                true
            }
            ToolbarEvent::UndoAllDelayed => {
                self.start_undo_all_delayed(self.undo_all_delay_ms);
                true
            }
            ToolbarEvent::RedoAllDelayed => {
                self.start_redo_all_delayed(self.redo_all_delay_ms);
                true
            }
            ToolbarEvent::CustomUndo => {
                self.start_custom_undo(self.custom_undo_delay_ms, self.custom_undo_steps);
                true
            }
            ToolbarEvent::CustomRedo => {
                self.start_custom_redo(self.custom_redo_delay_ms, self.custom_redo_steps);
                true
            }
            ToolbarEvent::ClearCanvas => {
                self.toolbar_clear();
                true
            }
            ToolbarEvent::PagePrev => {
                if self.page_prev() {
                    true
                } else {
                    self.set_ui_toast(UiToastKind::Info, "Already on the first page.");
                    false
                }
            }
            ToolbarEvent::PageNext => {
                if self.page_next() {
                    true
                } else {
                    self.set_ui_toast(UiToastKind::Info, "Already on the last page.");
                    false
                }
            }
            ToolbarEvent::PageNew => {
                self.page_new();
                true
            }
            ToolbarEvent::PageDuplicate => {
                self.page_duplicate();
                true
            }
            ToolbarEvent::PageDelete => {
                if matches!(self.page_delete(), PageDeleteOutcome::Cleared) {
                    self.set_ui_toast(UiToastKind::Info, "Cleared the last page.");
                }
                true
            }
            ToolbarEvent::EnterTextMode => {
                let _ = self.set_tool_override(None);
                self.toolbar_enter_text_mode();
                true
            }
            ToolbarEvent::EnterStickyNoteMode => {
                let _ = self.set_tool_override(None);
                self.toolbar_enter_sticky_note_mode();
                true
            }
            ToolbarEvent::ToggleAllHighlight(enable) => {
                // set_highlight_tool already handles both highlight tool and click highlight
                let currently_active =
                    self.highlight_tool_active() || self.click_highlight_enabled();
                if currently_active != enable {
                    self.set_highlight_tool(enable);
                    self.needs_redraw = true;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleFreeze => {
                self.request_frozen_toggle();
                self.needs_redraw = true;
                true
            }
            ToolbarEvent::ZoomIn => {
                self.request_zoom_action(ZoomAction::In);
                true
            }
            ToolbarEvent::ZoomOut => {
                self.request_zoom_action(ZoomAction::Out);
                true
            }
            ToolbarEvent::ResetZoom => {
                self.request_zoom_action(ZoomAction::Reset);
                true
            }
            ToolbarEvent::ToggleZoomLock => {
                self.request_zoom_action(ZoomAction::ToggleLock);
                true
            }
            ToolbarEvent::RefreshZoomCapture => {
                self.request_zoom_action(ZoomAction::RefreshCapture);
                true
            }
            ToolbarEvent::ToggleCustomSection(enable) => {
                if self.custom_section_enabled != enable {
                    self.custom_section_enabled = enable;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleDelaySliders(show) => {
                if self.show_delay_sliders != show {
                    self.show_delay_sliders = show;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::OpenConfigurator => {
                self.launch_configurator();
                true
            }
            ToolbarEvent::OpenConfigFile => {
                self.open_config_file_default();
                true
            }
            ToolbarEvent::CloseTopToolbar => {
                self.toolbar_top_visible = false;
                self.toolbar_visible = self.toolbar_top_visible || self.toolbar_side_visible;
                true
            }
            ToolbarEvent::CloseSideToolbar => {
                self.toolbar_side_visible = false;
                self.toolbar_visible = self.toolbar_top_visible || self.toolbar_side_visible;
                true
            }
            ToolbarEvent::PinTopToolbar(pin) => {
                if self.toolbar_top_pinned != pin {
                    self.toolbar_top_pinned = pin;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::PinSideToolbar(pin) => {
                if self.toolbar_side_pinned != pin {
                    self.toolbar_side_pinned = pin;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleIconMode(use_icons) => {
                if self.toolbar_use_icons != use_icons {
                    self.toolbar_use_icons = use_icons;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleMoreColors(show) => {
                if self.show_more_colors != show {
                    self.show_more_colors = show;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleActionsSection(show) => {
                if self.show_actions_section != show {
                    self.show_actions_section = show;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleActionsAdvanced(show) => {
                if self.show_actions_advanced != show {
                    self.show_actions_advanced = show;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::TogglePresets(show) => {
                if self.show_presets != show {
                    self.show_presets = show;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleStepSection(show) => {
                if self.show_step_section != show {
                    self.show_step_section = show;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleTextControls(show) => {
                if self.show_text_controls != show {
                    self.show_text_controls = show;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::TogglePresetToasts(show) => {
                if self.show_preset_toasts != show {
                    self.show_preset_toasts = show;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleToolPreview(show) => {
                if self.show_tool_preview != show {
                    self.show_tool_preview = show;
                    self.needs_redraw = true;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::SetToolbarLayoutMode(mode) => {
                if self.toolbar_layout_mode != mode {
                    self.toolbar_layout_mode = mode;
                    self.apply_toolbar_mode_defaults(mode);
                    if mode != ToolbarLayoutMode::Simple {
                        self.toolbar_shapes_expanded = false;
                    }
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ToggleShapePicker(open) => {
                let allow = self.toolbar_layout_mode == ToolbarLayoutMode::Simple;
                let next = allow && open;
                if self.toolbar_shapes_expanded != next {
                    self.toolbar_shapes_expanded = next;
                    true
                } else {
                    false
                }
            }
            ToolbarEvent::ApplyPreset(slot) => self.apply_preset(slot),
            ToolbarEvent::SavePreset(slot) => self.save_preset(slot),
            ToolbarEvent::ClearPreset(slot) => self.clear_preset(slot),
            ToolbarEvent::MoveTopToolbar { .. } | ToolbarEvent::MoveSideToolbar { .. } => false,
        }
    }
}
