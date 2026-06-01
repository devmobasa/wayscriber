use super::drawing::marker_color_with_opacity;
use super::*;
use crate::config::Action;
use crate::draw::Color;
use std::collections::HashSet;

fn color(r: f64) -> Color {
    Color {
        r,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    }
}

#[test]
fn descriptor_table_covers_every_tool_once() {
    let mut seen = HashSet::new();
    for tool in Tool::ALL {
        assert_eq!(tool.descriptor().tool, tool);
        assert!(seen.insert(tool));
    }
    assert_eq!(seen.len(), Tool::ALL.len());
}

#[test]
fn tool_profile_maps_compatibility_tools_to_pen_settings() {
    assert_eq!(Tool::Select.settings_slot(), ToolSettingsSlot::Pen);
    assert_eq!(Tool::Highlight.settings_slot(), ToolSettingsSlot::Pen);
    assert_eq!(Tool::Eraser.settings_slot(), ToolSettingsSlot::Pen);
    assert_eq!(
        Tool::Eraser.profile().size_source,
        ToolSizeSource::EraserSize
    );

    for slot in ToolSettingsSlot::ALL {
        assert_eq!(slot.representative_tool().settings_slot(), slot);
    }
}

#[test]
fn tool_profile_describes_toolbar_control_groups() {
    assert!(!Tool::Select.profile().needs_thickness_control());
    assert_eq!(Tool::Blur.profile().thickness_label, "Blur");
    assert!(Tool::Marker.profile().show_marker_opacity());
    assert!(Tool::Eraser.profile().show_eraser_mode());
    assert!(Tool::Rect.profile().show_fill_toggle());
    assert!(Tool::Arrow.profile().show_arrow_labels());
    assert!(Tool::StepMarker.profile().show_step_counter());
}

#[test]
fn polygon_tools_use_shape_controls_and_rect_settings() {
    for tool in [
        Tool::Triangle,
        Tool::Parallelogram,
        Tool::Rhombus,
        Tool::RegularPolygon,
        Tool::FreeformPolygon,
    ] {
        assert_eq!(tool.settings_slot(), ToolSettingsSlot::Rect);
        assert_eq!(tool.profile().control_group, ToolControlGroup::Shape);
        assert!(tool.profile().show_fill_toggle());
    }
}

#[test]
fn per_tool_settings_read_and_write_through_catalog_slot() {
    let mut settings = PerToolDrawingSettings::new(color(1.0), 4.0);
    settings.marker = ToolDrawingSettings::new(color(0.5), 12.0);

    assert_eq!(settings.get(Tool::Eraser), &settings.pen);
    assert_eq!(settings.get(Tool::Marker), &settings.marker);

    settings.get_mut(Tool::Highlight).thickness = 8.0;
    settings.get_mut(Tool::Marker).thickness = 16.0;

    assert_eq!(settings.pen.thickness, 8.0);
    assert_eq!(settings.marker.thickness, 16.0);
}

#[test]
fn selectable_tools_expose_actions_from_catalog() {
    assert_eq!(Tool::Select.action(), Some(Action::SelectSelectionTool));
    assert_eq!(Tool::Pen.action(), Some(Action::SelectPenTool));
    assert_eq!(Tool::Highlight.action(), Some(Action::SelectHighlightTool));
    assert_eq!(
        Tool::from_select_action(Action::SelectEraserTool),
        Some(Tool::Eraser)
    );
    assert_eq!(Tool::from_select_action(Action::ToggleEraserMode), None);
}

#[test]
fn drag_tools_round_trip_through_descriptor_table() {
    for tool in Tool::ALL {
        let drag_tool = DragTool::from_tool(tool);
        if let Some(drag_tool) = drag_tool {
            assert_ne!(drag_tool, DragTool::Default);
            assert_eq!(drag_tool.as_tool(), Some(tool));
        } else {
            assert_eq!(tool, Tool::FreeformPolygon);
        }
    }
    assert_eq!(DragTool::Default.as_tool(), None);
}

#[test]
fn drag_bindable_tool_list_excludes_freeform_polygon_and_default() {
    assert_eq!(DragBindableTool::from_tool(Tool::FreeformPolygon), None);
    assert_eq!(DragBindableTool::from_drag_tool(DragTool::Default), None);
    assert_eq!(
        DragBindableTool::from_tool(Tool::RegularPolygon),
        Some(DragBindableTool::RegularPolygon)
    );
    assert_eq!(
        DragTool::RegularPolygon.as_tool(),
        Some(Tool::RegularPolygon)
    );
}

#[test]
fn descriptor_exposes_press_motion_and_drawing_behavior() {
    assert_eq!(Tool::Select.press_behavior(), ToolPressBehavior::Selection);
    assert_eq!(
        Tool::Highlight.press_behavior(),
        ToolPressBehavior::HighlightNoop
    );
    assert_eq!(
        Tool::Blur.press_behavior(),
        ToolPressBehavior::StartDrawing {
            request_blur_capture: true
        }
    );
    assert!(matches!(
        Tool::Pen.motion_behavior(),
        ToolMotionBehavior::AccumulatePath {
            size_source: ToolMotionSizeSource::ToolSize
        }
    ));
    assert!(matches!(
        Tool::Eraser.motion_behavior(),
        ToolMotionBehavior::AccumulatePath {
            size_source: ToolMotionSizeSource::EraserSize
        }
    ));
    assert_eq!(
        Tool::Line.motion_behavior(),
        ToolMotionBehavior::NoPathAccumulation
    );
    assert!(matches!(
        Tool::Marker.drawing_behavior(),
        ToolDrawingBehavior::Path {
            kind: ToolPathKind::Marker,
            pressure: ToolPressureBehavior::None
        }
    ));
}

#[test]
fn marker_opacity_helper_preserves_current_alpha_clamp() {
    assert_eq!(marker_color_with_opacity(color(1.0), 0.0).a, 0.05);
    assert_eq!(marker_color_with_opacity(color(1.0), 2.0).a, 0.9);
}
