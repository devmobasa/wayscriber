use super::*;

fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
    Rect::new(x, y, width, height).expect("positive test rectangle")
}

fn prepared() -> PreparedFrame {
    PreparedFrame {
        geometry: FrameGeometry::new(101, 79, 2),
        visibility: FrameVisibility::new(OverlaySuppression::None, true),
        canvas: CanvasPolicyInputs {
            capture_picker_active: false,
            include_drawings: true,
            transform_active: false,
            origin: (0.0, 0.0),
            zoom_scale: None,
            text_halo_enabled: true,
            layer_cache_usable: true,
        },
        damage_screen: vec![rect(3, 7, 11, 13), rect(70, 50, 9, 8)],
        full_damage_reason: None,
        damage_diagnostics: PerfDamageDiagnostics::default(),
        profile: FrameProfile::new(None, false, false),
        now: Instant::now(),
        keep_rendering: false,
    }
}

#[test]
fn untransformed_damage_preserves_screen_regions_and_scales_only_buffer_regions() {
    let plan = plan_frame(prepared());
    assert_eq!(
        plan.damage.screen,
        vec![rect(3, 7, 11, 13), rect(70, 50, 9, 8)]
    );
    assert_eq!(plan.damage.world, plan.damage.screen);
    assert_eq!(
        plan.damage.buffer,
        vec![rect(6, 14, 22, 26), rect(140, 100, 18, 16)]
    );
}

#[test]
fn pan_uses_world_viewport_while_compositor_damage_stays_in_screen_space() {
    let mut input = prepared();
    input.canvas.transform_active = true;
    input.canvas.origin = (-12.25, 8.75);
    let plan = plan_frame(input);
    assert_eq!(plan.damage.world, vec![rect(-13, 8, 101, 79)]);
    assert_eq!(plan.damage.screen[0], rect(3, 7, 11, 13));
    assert_eq!(plan.damage.buffer[0], rect(6, 14, 22, 26));
    assert_eq!(plan.canvas.origin, (-12.25, 8.75));
}

#[test]
fn fractional_zoom_rounds_world_extent_up_and_negative_origin_down() {
    for (zoom, width, height) in [(1.5, 68, 53), (0.75, 135, 106)] {
        let mut input = prepared();
        input.canvas.transform_active = true;
        input.canvas.origin = (-0.1, -40.01);
        input.canvas.zoom_scale = Some(zoom);
        let plan = plan_frame(input);
        assert_eq!(plan.damage.world, vec![rect(-1, -41, width, height)]);
        assert_eq!(plan.canvas.zoom_scale, Some(zoom));
        assert_eq!(plan.damage.buffer[0], rect(6, 14, 22, 26));
    }
}

#[test]
fn nonpositive_zoom_preserves_saturated_world_extent_behavior() {
    for zoom in [0.0, -1.0] {
        let mut input = prepared();
        input.canvas.transform_active = true;
        input.canvas.zoom_scale = Some(zoom);
        let plan = plan_frame(input);
        assert_eq!(plan.damage.world, vec![rect(0, 0, i32::MAX, i32::MAX)]);
    }
}

#[test]
fn empty_damage_is_not_replaced_or_given_a_new_full_damage_reason() {
    let mut input = prepared();
    input.damage_screen.clear();
    let plan = plan_frame(input);
    assert!(plan.damage.screen.is_empty());
    assert!(plan.damage.world.is_empty());
    assert!(plan.damage.buffer.is_empty());
    assert_eq!(plan.damage.full_reason, None);
}

#[test]
fn transformed_world_viewport_does_not_depend_on_screen_damage() {
    let mut input = prepared();
    input.damage_screen.clear();
    input.canvas.transform_active = true;
    let plan = plan_frame(input);
    assert_eq!(plan.damage.world, vec![rect(0, 0, 101, 79)]);
    assert!(plan.damage.screen.is_empty());
    assert!(plan.damage.buffer.is_empty());
}

#[test]
fn zero_dimensions_do_not_create_a_world_viewport() {
    for (width, height) in [(0, 79), (101, 0), (0, 0)] {
        let mut input = prepared();
        input.geometry = FrameGeometry::new(width, height, 2);
        input.canvas.transform_active = true;
        input.damage_screen.clear();
        let plan = plan_frame(input);
        assert!(plan.damage.world.is_empty());
        assert_eq!(plan.geometry.byte_len, 0);
    }
}

#[test]
fn geometry_normalizes_buffer_scale_and_preserves_argb_layout() {
    for scale in [-2, 0, 1] {
        assert_eq!(
            FrameGeometry::new(101, 79, scale),
            FrameGeometry {
                width: 101,
                height: 79,
                scale: 1,
                physical_width: 101,
                physical_height: 79,
                stride: 404,
                byte_len: 31_916,
            }
        );
    }
    let geometry = FrameGeometry::new(101, 79, 2);
    assert_eq!(
        (geometry.physical_width, geometry.physical_height),
        (202, 158)
    );
    assert_eq!(geometry.stride, 808);
    assert_eq!(geometry.byte_len, 127_664);
}

#[test]
fn suppression_policy_covers_transparent_and_opaque_boards() {
    use OverlaySuppression::*;
    for transparent in [false, true] {
        for (suppression, expected) in [
            (None, (true, true, true)),
            (Capture, (true, false, false)),
            (DesktopBackdrop, (false, false, false)),
            (ExternalDialog, (false, false, false)),
            (Frozen, (false, false, false)),
            (Zoom, (!transparent, !transparent, !transparent)),
        ] {
            let mut input = prepared();
            input.visibility = FrameVisibility::new(suppression, transparent);
            let plan = plan_frame(input);
            assert_eq!(
                (
                    plan.render_canvas,
                    plan.canvas.render_transients,
                    plan.render_ui
                ),
                expected,
                "suppression={suppression:?}, transparent={transparent}"
            );
        }
    }
}

#[test]
fn picker_drawings_option_controls_committed_shapes_but_disables_transients_and_cache() {
    for (picker, include_drawings, committed, transients, cache) in [
        (false, false, true, true, true),
        (false, true, true, true, true),
        (true, false, false, false, false),
        (true, true, true, false, false),
    ] {
        let mut input = prepared();
        input.canvas.capture_picker_active = picker;
        input.canvas.include_drawings = include_drawings;
        let plan = plan_frame(input);
        assert_eq!(plan.canvas.draw_committed, committed);
        assert_eq!(plan.canvas.render_transients, transients);
        assert_eq!(plan.canvas.layer_cache_eligible, cache);
    }
}

#[test]
fn unavailable_cache_and_suppressed_transients_remain_disabled_outside_picker() {
    let mut input = prepared();
    input.canvas.layer_cache_usable = false;
    input.visibility = FrameVisibility::new(OverlaySuppression::Capture, true);
    let plan = plan_frame(input);
    assert!(plan.canvas.draw_committed);
    assert!(!plan.canvas.layer_cache_eligible);
    assert!(!plan.canvas.render_transients);
}

#[test]
fn planning_preserves_prepared_damage_diagnostics_and_animation_values() {
    let mut input = prepared();
    input.full_damage_reason = Some(FullDamageReason::EmptyDamageFallback);
    input.damage_diagnostics.input_regions = 7;
    input.damage_diagnostics.buffer_regions_before_merge = 9;
    input.keep_rendering = true;
    input.canvas.text_halo_enabled = false;
    let diagnostics = input.damage_diagnostics;
    let now = input.now;
    let plan = plan_frame(input);
    assert_eq!(
        plan.damage.full_reason,
        Some(FullDamageReason::EmptyDamageFallback)
    );
    assert_eq!(plan.damage.diagnostics, diagnostics);
    assert_eq!(plan.now, now);
    assert!(plan.keep_rendering);
    assert!(!plan.canvas.text_halo_enabled);
}
