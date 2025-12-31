use super::super::events::{HitKind, delay_secs_from_t, delay_t_from_ms};
use super::super::format_binding_label;
use super::super::hit::HitRegion;
use super::spec::ToolbarLayoutSpec;
use crate::config::ToolbarLayoutMode;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

/// Populate hit regions for the side toolbar.
#[allow(dead_code)]
pub fn build_side_hits(
    width: f64,
    _height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
) {
    let spec = ToolbarLayoutSpec::new(snapshot);
    let use_icons = spec.use_icons();
    let x = ToolbarLayoutSpec::SIDE_START_X;
    let (pin_x, close_x, header_y) = spec.side_header_button_positions(width);
    let header_btn = ToolbarLayoutSpec::SIDE_HEADER_BUTTON_SIZE;
    let content_width = spec.side_content_width(width);
    let section_gap = ToolbarLayoutSpec::SIDE_SECTION_GAP;
    let show_text_controls =
        snapshot.text_active || snapshot.note_active || snapshot.show_text_controls;
    let icons_w = ToolbarLayoutSpec::SIDE_HEADER_TOGGLE_WIDTH;
    hits.push(HitRegion {
        rect: (x, header_y, icons_w, header_btn),
        event: ToolbarEvent::ToggleIconMode(!snapshot.use_icons),
        kind: HitKind::Click,
        tooltip: None,
    });
    let mode_w = ToolbarLayoutSpec::SIDE_HEADER_MODE_WIDTH;
    let mode_x = x + icons_w + ToolbarLayoutSpec::SIDE_HEADER_MODE_GAP;
    let mode_tooltip = format!(
        "Mode: S/R/A = {}/{}/{}",
        ToolbarLayoutMode::Simple.label(),
        ToolbarLayoutMode::Regular.label(),
        ToolbarLayoutMode::Advanced.label(),
    );
    hits.push(HitRegion {
        rect: (mode_x, header_y, mode_w, header_btn),
        event: ToolbarEvent::SetToolbarLayoutMode(snapshot.layout_mode.next()),
        kind: HitKind::Click,
        tooltip: Some(mode_tooltip),
    });
    hits.push(HitRegion {
        rect: (close_x, header_y, header_btn, header_btn),
        event: ToolbarEvent::CloseSideToolbar,
        kind: HitKind::Click,
        tooltip: Some("Close".to_string()),
    });

    hits.push(HitRegion {
        rect: (pin_x, header_y, header_btn, header_btn),
        event: ToolbarEvent::PinSideToolbar(!snapshot.side_pinned),
        kind: HitKind::Click,
        tooltip: Some(if snapshot.side_pinned {
            "Unpin".to_string()
        } else {
            "Pin".to_string()
        }),
    });

    let mut y = spec.side_content_start_y();

    // Color picker hit region
    let picker_y = y + ToolbarLayoutSpec::SIDE_COLOR_PICKER_OFFSET_Y;
    let picker_h = spec.side_color_picker_height(snapshot);
    hits.push(HitRegion {
        rect: (x, picker_y, content_width, picker_h),
        event: ToolbarEvent::SetColor(snapshot.color),
        kind: HitKind::PickColor {
            x,
            y: picker_y,
            w: content_width,
            h: picker_h,
        },
        tooltip: None,
    });
    y += spec.side_colors_height(snapshot) + section_gap;

    // Preset slots
    let presets_card_h = ToolbarLayoutSpec::SIDE_PRESET_CARD_HEIGHT;
    let slot_count = snapshot.preset_slot_count.min(snapshot.presets.len());
    if snapshot.show_presets && slot_count > 0 {
        let slot_size = ToolbarLayoutSpec::SIDE_PRESET_SLOT_SIZE;
        let slot_gap = ToolbarLayoutSpec::SIDE_PRESET_SLOT_GAP;
        let slot_row_y = y + ToolbarLayoutSpec::SIDE_PRESET_ROW_OFFSET_Y;
        let action_row_y = slot_row_y + slot_size + ToolbarLayoutSpec::SIDE_PRESET_ACTION_GAP;
        let action_gap = ToolbarLayoutSpec::SIDE_PRESET_ACTION_BUTTON_GAP;
        let action_w = (slot_size - action_gap) / 2.0;
        for slot_index in 0..slot_count {
            let slot = slot_index + 1;
            let slot_x = x + slot_index as f64 * (slot_size + slot_gap);
            let preset_exists = snapshot
                .presets
                .get(slot_index)
                .and_then(|preset| preset.as_ref())
                .is_some();
            if preset_exists {
                hits.push(HitRegion {
                    rect: (slot_x, slot_row_y, slot_size, slot_size),
                    event: ToolbarEvent::ApplyPreset(slot),
                    kind: HitKind::Click,
                    tooltip: Some(format!("Apply preset {}", slot)),
                });
            }
            hits.push(HitRegion {
                rect: (
                    slot_x,
                    action_row_y,
                    action_w,
                    ToolbarLayoutSpec::SIDE_PRESET_ACTION_HEIGHT,
                ),
                event: ToolbarEvent::SavePreset(slot),
                kind: HitKind::Click,
                tooltip: Some(format!("Save preset {}", slot)),
            });
            if preset_exists {
                hits.push(HitRegion {
                    rect: (
                        slot_x + action_w + action_gap,
                        action_row_y,
                        action_w,
                        ToolbarLayoutSpec::SIDE_PRESET_ACTION_HEIGHT,
                    ),
                    event: ToolbarEvent::ClearPreset(slot),
                    kind: HitKind::Click,
                    tooltip: Some(format!("Clear preset {}", slot)),
                });
            }
        }
        y += presets_card_h + section_gap;
    }

    // Thickness slider
    let slider_row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
    let slider_hit_h = ToolbarLayoutSpec::SIDE_NUDGE_SIZE;
    hits.push(HitRegion {
        rect: (x, slider_row_y, content_width, slider_hit_h),
        event: ToolbarEvent::SetThickness(snapshot.thickness),
        kind: HitKind::DragSetThickness {
            min: 1.0,
            max: 50.0,
        },
        tooltip: None,
    });
    hits.push(HitRegion {
        rect: (
            x,
            slider_row_y,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
        ),
        event: ToolbarEvent::NudgeThickness(-1.0),
        kind: HitKind::Click,
        tooltip: None,
    });
    hits.push(HitRegion {
        rect: (
            x + content_width - ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
            slider_row_y,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
        ),
        event: ToolbarEvent::NudgeThickness(1.0),
        kind: HitKind::Click,
        tooltip: None,
    });
    y += ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT + section_gap;

    if snapshot.thickness_targets_eraser {
        y += ToolbarLayoutSpec::SIDE_ERASER_MODE_CARD_HEIGHT + section_gap;
    }

    let show_marker_opacity =
        snapshot.show_marker_opacity_section || snapshot.thickness_targets_marker;
    if show_marker_opacity {
        y += ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT + section_gap;
    }

    // Text size slider
    if show_text_controls {
        let text_slider_row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
        hits.push(HitRegion {
            rect: (x, text_slider_row_y, content_width, slider_hit_h),
            event: ToolbarEvent::SetFontSize(snapshot.font_size),
            kind: HitKind::DragSetFontSize,
            tooltip: None,
        });
        y += ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT + section_gap;
        y += ToolbarLayoutSpec::SIDE_FONT_CARD_HEIGHT + section_gap;
    }

    // Actions section
    let show_actions = snapshot.show_actions_section || snapshot.show_actions_advanced;
    if show_actions {
        let actions_card_h = spec.side_actions_height(snapshot);
        let mut action_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
        let basic_actions = [
            ToolbarEvent::Undo,
            ToolbarEvent::Redo,
            ToolbarEvent::ClearCanvas,
        ];
        let advanced_actions = [
            ToolbarEvent::UndoAll,
            ToolbarEvent::RedoAll,
            ToolbarEvent::UndoAllDelayed,
            ToolbarEvent::RedoAllDelayed,
            ToolbarEvent::ToggleFreeze,
            ToolbarEvent::ZoomIn,
            ToolbarEvent::ZoomOut,
            ToolbarEvent::ResetZoom,
            ToolbarEvent::ToggleZoomLock,
        ];
        let action_label = |event: &ToolbarEvent| match event {
            ToolbarEvent::Undo => "Undo",
            ToolbarEvent::Redo => "Redo",
            ToolbarEvent::ClearCanvas => "Clear",
            ToolbarEvent::UndoAll => "Undo All",
            ToolbarEvent::RedoAll => "Redo All",
            ToolbarEvent::UndoAllDelayed => "Undo All Delay",
            ToolbarEvent::RedoAllDelayed => "Redo All Delay",
            ToolbarEvent::ToggleFreeze => {
                if snapshot.frozen_active {
                    "Unfreeze"
                } else {
                    "Freeze"
                }
            }
            ToolbarEvent::ZoomIn => "Zoom In",
            ToolbarEvent::ZoomOut => "Zoom Out",
            ToolbarEvent::ResetZoom => "Reset Zoom",
            ToolbarEvent::ToggleZoomLock => {
                if snapshot.zoom_locked {
                    "Unlock Zoom"
                } else {
                    "Lock Zoom"
                }
            }
            _ => "Action",
        };

        if snapshot.show_actions_section {
            if use_icons {
                let icon_btn = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
                let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
                let icons_per_row = basic_actions.len();
                let total_icons_w =
                    icons_per_row as f64 * icon_btn + (icons_per_row as f64 - 1.0) * icon_gap;
                let icons_start_x = x + (content_width - total_icons_w) / 2.0;
                for (idx, evt) in basic_actions.iter().enumerate() {
                    let bx = icons_start_x + (icon_btn + icon_gap) * idx as f64;
                    hits.push(HitRegion {
                        rect: (bx, action_y, icon_btn, icon_btn),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            action_label(evt),
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
                action_y += icon_btn;
            } else {
                let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
                let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
                for (idx, evt) in basic_actions.iter().enumerate() {
                    let by = action_y + (action_h + action_gap) * idx as f64;
                    hits.push(HitRegion {
                        rect: (x, by, content_width, action_h),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            action_label(evt),
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
                action_y += action_h * basic_actions.len() as f64
                    + action_gap * (basic_actions.len() as f64 - 1.0);
            }
        }

        if snapshot.show_actions_section && snapshot.show_actions_advanced {
            action_y += ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        }

        if snapshot.show_actions_advanced {
            if use_icons {
                let icon_btn = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
                let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
                let icons_per_row = 5usize;
                let total_icons_w =
                    icons_per_row as f64 * icon_btn + (icons_per_row as f64 - 1.0) * icon_gap;
                let icons_start_x = x + (content_width - total_icons_w) / 2.0;
                for (idx, evt) in advanced_actions.iter().enumerate() {
                    let row = idx / icons_per_row;
                    let col = idx % icons_per_row;
                    let bx = icons_start_x + (icon_btn + icon_gap) * col as f64;
                    let by = action_y + (icon_btn + icon_gap) * row as f64;
                    hits.push(HitRegion {
                        rect: (bx, by, icon_btn, icon_btn),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            action_label(evt),
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
            } else {
                let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
                let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
                let action_col_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
                let action_w = (content_width - action_col_gap) / 2.0;
                for (idx, evt) in advanced_actions.iter().enumerate() {
                    let row = idx / 2;
                    let col = idx % 2;
                    let bx = x + (action_w + action_col_gap) * col as f64;
                    let by = action_y + (action_h + action_gap) * row as f64;
                    hits.push(HitRegion {
                        rect: (bx, by, action_w, action_h),
                        event: evt.clone(),
                        kind: HitKind::Click,
                        tooltip: Some(format_binding_label(
                            action_label(evt),
                            snapshot.binding_hints.binding_for_event(evt),
                        )),
                    });
                }
            }
        }

        y += actions_card_h + section_gap;
    }

    if snapshot.show_actions_advanced {
        let pages_card_h = spec.side_pages_height(snapshot);
        let pages_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
        let btn_h = if use_icons {
            ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON
        } else {
            ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT
        };
        let btn_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        let btn_w = (content_width - btn_gap * 4.0) / 5.0;
        let buttons = [
            (ToolbarEvent::PagePrev, "Prev"),
            (ToolbarEvent::PageNext, "Next"),
            (ToolbarEvent::PageNew, "New"),
            (ToolbarEvent::PageDuplicate, "Dup"),
            (ToolbarEvent::PageDelete, "Del"),
        ];
        for (idx, (evt, label)) in buttons.iter().enumerate() {
            let bx = x + (btn_w + btn_gap) * idx as f64;
            hits.push(HitRegion {
                rect: (bx, pages_y, btn_w, btn_h),
                event: evt.clone(),
                kind: HitKind::Click,
                tooltip: Some(format_binding_label(
                    label,
                    snapshot.binding_hints.binding_for_event(evt),
                )),
            });
        }
        y += pages_card_h + section_gap;
    }

    // Delay sliders
    if snapshot.show_step_section && snapshot.show_delay_sliders {
        let undo_t = delay_t_from_ms(snapshot.undo_all_delay_ms);
        let redo_t = delay_t_from_ms(snapshot.redo_all_delay_ms);
        let toggles_h =
            ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT * 2.0 + ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
        let custom_h = if snapshot.custom_section_enabled {
            ToolbarLayoutSpec::SIDE_CUSTOM_SECTION_HEIGHT
        } else {
            0.0
        };
        let slider_start_y = y
            + ToolbarLayoutSpec::SIDE_STEP_HEADER_HEIGHT
            + toggles_h
            + custom_h
            + ToolbarLayoutSpec::SIDE_STEP_SLIDER_TOP_PADDING;
        let slider_hit_h = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HEIGHT
            + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING * 2.0;
        let undo_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_UNDO_OFFSET_Y;
        hits.push(HitRegion {
            rect: (
                x,
                undo_y - ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING,
                content_width,
                slider_hit_h,
            ),
            event: ToolbarEvent::SetUndoDelay(delay_secs_from_t(undo_t)),
            kind: HitKind::DragUndoDelay,
            tooltip: None,
        });
        let redo_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_REDO_OFFSET_Y;
        hits.push(HitRegion {
            rect: (
                x,
                redo_y - ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING,
                content_width,
                slider_hit_h,
            ),
            event: ToolbarEvent::SetRedoDelay(delay_secs_from_t(redo_t)),
            kind: HitKind::DragRedoDelay,
            tooltip: None,
        });
    }

    if snapshot.show_step_section {
        y += spec.side_step_height(snapshot) + section_gap;
    }

    if snapshot.show_settings_section {
        let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
        let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
        let mut toggles: Vec<(ToolbarEvent, Option<&str>)> = vec![
            (
                ToolbarEvent::ToggleToolPreview(!snapshot.show_tool_preview),
                Some("Tool preview: cursor bubble."),
            ),
            (
                ToolbarEvent::TogglePresetToasts(!snapshot.show_preset_toasts),
                Some("Preset toasts: apply/save/clear."),
            ),
        ];
        if snapshot.layout_mode == ToolbarLayoutMode::Advanced {
            toggles.extend_from_slice(&[
                (
                    ToolbarEvent::TogglePresets(!snapshot.show_presets),
                    Some("Presets: quick slots."),
                ),
                (
                    ToolbarEvent::ToggleActionsSection(!snapshot.show_actions_section),
                    Some("Actions: undo/redo/clear."),
                ),
                (
                    ToolbarEvent::ToggleActionsAdvanced(!snapshot.show_actions_advanced),
                    Some("Advanced: undo-all/delay/zoom."),
                ),
                (
                    ToolbarEvent::ToggleStepSection(!snapshot.show_step_section),
                    Some("Step: step undo/redo."),
                ),
                (
                    ToolbarEvent::ToggleTextControls(!snapshot.show_text_controls),
                    Some("Text: font size/family."),
                ),
            ]);
        }

        let mut toggle_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
        for (idx, (evt, tooltip)) in toggles.iter().enumerate() {
            hits.push(HitRegion {
                rect: (x, toggle_y, content_width, toggle_h),
                event: evt.clone(),
                kind: HitKind::Click,
                tooltip: tooltip.map(|text| text.to_string()),
            });
            if idx + 1 < toggles.len() {
                toggle_y += toggle_h + toggle_gap;
            } else {
                toggle_y += toggle_h;
            }
        }

        let buttons_y = toggle_y + toggle_gap;
        let button_h = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_HEIGHT;
        let button_gap = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_GAP;
        let button_w = (content_width - button_gap) / 2.0;
        hits.push(HitRegion {
            rect: (x, buttons_y, button_w, button_h),
            event: ToolbarEvent::OpenConfigurator,
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                "Config UI",
                snapshot.binding_hints.open_configurator.as_deref(),
            )),
        });
        hits.push(HitRegion {
            rect: (x + button_w + button_gap, buttons_y, button_w, button_h),
            event: ToolbarEvent::OpenConfigFile,
            kind: HitKind::Click,
            tooltip: Some("Config file".to_string()),
        });
    }
}
