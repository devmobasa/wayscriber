use super::SidePaletteLayout;
use crate::backend::wayland::toolbar::events::{HitKind, delay_secs_from_t, delay_t_from_ms};
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;

use super::super::widgets::*;

pub(super) fn draw_step_section(layout: &mut SidePaletteLayout, y: &mut f64) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hits = &mut layout.hits;
    let hover = layout.hover;
    let x = layout.x;
    let card_x = layout.card_x;
    let card_w = layout.card_w;
    let content_width = layout.content_width;
    let section_gap = layout.section_gap;

    if !snapshot.show_step_section {
        return;
    }

    let custom_toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
    let toggles_h = custom_toggle_h * 2.0 + toggle_gap;
    let custom_content_h = if snapshot.custom_section_enabled {
        ToolbarLayoutSpec::SIDE_CUSTOM_SECTION_HEIGHT
    } else {
        0.0
    };
    let delay_sliders_h = if snapshot.show_delay_sliders {
        ToolbarLayoutSpec::SIDE_DELAY_SECTION_HEIGHT
    } else {
        0.0
    };
    let custom_card_h =
        ToolbarLayoutSpec::SIDE_STEP_HEADER_HEIGHT + toggles_h + custom_content_h + delay_sliders_h;
    draw_group_card(ctx, card_x, *y, card_w, custom_card_h);
    draw_section_label(
        ctx,
        x,
        *y + ToolbarLayoutSpec::SIDE_SECTION_LABEL_OFFSET_TALL,
        "Step Undo/Redo",
    );

    let custom_toggle_y = *y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let toggle_w = content_width;

    let step_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, custom_toggle_y, toggle_w, custom_toggle_h))
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        custom_toggle_y,
        toggle_w,
        custom_toggle_h,
        snapshot.custom_section_enabled,
        step_hover,
        "Step controls",
    );
    hits.push(HitRegion {
        rect: (x, custom_toggle_y, toggle_w, custom_toggle_h),
        event: ToolbarEvent::ToggleCustomSection(!snapshot.custom_section_enabled),
        kind: HitKind::Click,
        tooltip: Some("Step controls: multi-step undo/redo.".to_string()),
    });

    let delay_toggle_y = custom_toggle_y + custom_toggle_h + toggle_gap;
    let delay_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, delay_toggle_y, toggle_w, custom_toggle_h))
        .unwrap_or(false);
    draw_checkbox(
        ctx,
        x,
        delay_toggle_y,
        toggle_w,
        custom_toggle_h,
        snapshot.show_delay_sliders,
        delay_hover,
        "Delay sliders",
    );
    hits.push(HitRegion {
        rect: (x, delay_toggle_y, toggle_w, custom_toggle_h),
        event: ToolbarEvent::ToggleDelaySliders(!snapshot.show_delay_sliders),
        kind: HitKind::Click,
        tooltip: Some("Delay sliders: undo/redo delays.".to_string()),
    });

    let mut custom_y = delay_toggle_y + custom_toggle_h + toggle_gap;

    if snapshot.custom_section_enabled {
        let render_custom_row = |ctx: &cairo::Context,
                                 hits: &mut Vec<HitRegion>,
                                 x: f64,
                                 y: f64,
                                 w: f64,
                                 snapshot: &crate::ui::toolbar::ToolbarSnapshot,
                                 is_undo: bool,
                                 hover: Option<(f64, f64)>| {
            let row_h = 26.0;
            let btn_w = if snapshot.use_icons { 42.0 } else { 90.0 };
            let steps_btn_w = 26.0;
            let gap = 6.0;
            let label = if is_undo { "Step Undo" } else { "Step Redo" };
            let steps = if is_undo {
                snapshot.custom_undo_steps
            } else {
                snapshot.custom_redo_steps
            };
            let delay_ms = if is_undo {
                snapshot.custom_undo_delay_ms
            } else {
                snapshot.custom_redo_delay_ms
            };

            let btn_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_w, row_h))
                .unwrap_or(false);

            if snapshot.use_icons {
                let icon_size = 20.0;
                draw_button(ctx, x, y, btn_w, row_h, false, btn_hover);
                set_icon_color(ctx, btn_hover);
                if is_undo {
                    toolbar_icons::draw_icon_step_undo(
                        ctx,
                        x + (btn_w - icon_size) / 2.0,
                        y + (row_h - icon_size) / 2.0,
                        icon_size,
                    );
                } else {
                    toolbar_icons::draw_icon_step_redo(
                        ctx,
                        x + (btn_w - icon_size) / 2.0,
                        y + (row_h - icon_size) / 2.0,
                        icon_size,
                    );
                }
            } else {
                draw_button(ctx, x, y, btn_w, row_h, false, btn_hover);
                draw_label_left(ctx, x + 10.0, y, btn_w - 20.0, row_h, label);
            }
            hits.push(HitRegion {
                rect: (x, y, btn_w, row_h),
                event: if is_undo {
                    ToolbarEvent::CustomUndo
                } else {
                    ToolbarEvent::CustomRedo
                },
                kind: HitKind::Click,
                tooltip: Some(if is_undo {
                    "Step undo".to_string()
                } else {
                    "Step redo".to_string()
                }),
            });

            let steps_x = x + btn_w + gap;
            let minus_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, steps_x, y, steps_btn_w, row_h))
                .unwrap_or(false);
            draw_button(ctx, steps_x, y, steps_btn_w, row_h, false, minus_hover);
            set_icon_color(ctx, minus_hover);
            toolbar_icons::draw_icon_minus(
                ctx,
                steps_x + (steps_btn_w - ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE) / 2.0,
                y + (row_h - ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE) / 2.0,
                ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE,
            );
            hits.push(HitRegion {
                rect: (steps_x, y, steps_btn_w, row_h),
                event: if is_undo {
                    ToolbarEvent::SetCustomUndoSteps(steps.saturating_sub(1).max(1))
                } else {
                    ToolbarEvent::SetCustomRedoSteps(steps.saturating_sub(1).max(1))
                },
                kind: HitKind::Click,
                tooltip: Some(if is_undo {
                    "Decrease undo steps".to_string()
                } else {
                    "Decrease redo steps".to_string()
                }),
            });

            let steps_val_x = steps_x + steps_btn_w + 4.0;
            draw_label_center(
                ctx,
                steps_val_x,
                y,
                54.0,
                row_h,
                &format!("{} steps", steps),
            );

            let steps_plus_x = steps_val_x + 58.0;
            let plus_hover = hover
                .map(|(hx, hy)| point_in_rect(hx, hy, steps_plus_x, y, steps_btn_w, row_h))
                .unwrap_or(false);
            draw_button(ctx, steps_plus_x, y, steps_btn_w, row_h, false, plus_hover);
            set_icon_color(ctx, plus_hover);
            toolbar_icons::draw_icon_plus(
                ctx,
                steps_plus_x + (steps_btn_w - ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE) / 2.0,
                y + (row_h - ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE) / 2.0,
                ToolbarLayoutSpec::SIDE_NUDGE_ICON_SIZE,
            );
            hits.push(HitRegion {
                rect: (steps_plus_x, y, steps_btn_w, row_h),
                event: if is_undo {
                    ToolbarEvent::SetCustomUndoSteps(steps.saturating_add(1))
                } else {
                    ToolbarEvent::SetCustomRedoSteps(steps.saturating_add(1))
                },
                kind: HitKind::Click,
                tooltip: Some(if is_undo {
                    "Increase undo steps".to_string()
                } else {
                    "Increase redo steps".to_string()
                }),
            });

            let slider_y = y + row_h + 8.0;
            let slider_h = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HEIGHT;
            let slider_r = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_KNOB_RADIUS;
            let slider_w = w - ToolbarLayoutSpec::SIDE_CARD_INSET * 2.0;
            ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
            draw_round_rect(ctx, x, slider_y, slider_w, slider_h, 3.0);
            let _ = ctx.fill();
            let t = delay_t_from_ms(delay_ms);
            let knob_x = x + t * (slider_w - slider_r * 2.0) + slider_r;
            ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
            ctx.arc(
                knob_x,
                slider_y + slider_h / 2.0,
                slider_r,
                0.0,
                std::f64::consts::PI * 2.0,
            );
            let _ = ctx.fill();
            let hit_pad = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING;
            hits.push(HitRegion {
                rect: (x, slider_y - hit_pad, slider_w, slider_h + hit_pad * 2.0),
                event: if is_undo {
                    ToolbarEvent::SetCustomUndoDelay(delay_secs_from_t(t))
                } else {
                    ToolbarEvent::SetCustomRedoDelay(delay_secs_from_t(t))
                },
                kind: if is_undo {
                    HitKind::DragCustomUndoDelay
                } else {
                    HitKind::DragCustomRedoDelay
                },
                tooltip: Some(if is_undo {
                    format!("Undo step delay: {:.1}s (drag)", delay_ms as f64 / 1000.0)
                } else {
                    format!("Redo step delay: {:.1}s (drag)", delay_ms as f64 / 1000.0)
                }),
            });

            slider_y + slider_h + 10.0 - y
        };

        let undo_row_h = render_custom_row(ctx, hits, x, custom_y, card_w, snapshot, true, hover);
        custom_y += undo_row_h + 8.0;
        let _redo_row_h = render_custom_row(ctx, hits, x, custom_y, card_w, snapshot, false, hover);
    }

    if snapshot.show_delay_sliders {
        let sliders_w = content_width;
        let slider_h = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HEIGHT;
        let slider_knob_r = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_KNOB_RADIUS;
        let slider_start_y = *y
            + ToolbarLayoutSpec::SIDE_STEP_HEADER_HEIGHT
            + toggles_h
            + custom_content_h
            + ToolbarLayoutSpec::SIDE_STEP_SLIDER_TOP_PADDING;

        let undo_label = format!(
            "Undo delay: {:.1}s",
            snapshot.undo_all_delay_ms as f64 / 1000.0
        );
        ctx.set_source_rgba(0.7, 0.7, 0.75, 0.9);
        ctx.set_font_size(11.0);
        ctx.move_to(x, slider_start_y + 10.0);
        let _ = ctx.show_text(&undo_label);

        let undo_slider_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_UNDO_OFFSET_Y;
        ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
        draw_round_rect(ctx, x, undo_slider_y, sliders_w, slider_h, 3.0);
        let _ = ctx.fill();
        let undo_t = delay_t_from_ms(snapshot.undo_all_delay_ms);
        let undo_knob_x = x + undo_t * (sliders_w - slider_knob_r * 2.0) + slider_knob_r;
        ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
        ctx.arc(
            undo_knob_x,
            undo_slider_y + slider_h / 2.0,
            slider_knob_r,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        let _ = ctx.fill();
        let hit_pad = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING;
        hits.push(HitRegion {
            rect: (
                x,
                undo_slider_y - hit_pad,
                sliders_w,
                slider_h + hit_pad * 2.0,
            ),
            event: ToolbarEvent::SetUndoDelay(delay_secs_from_t(undo_t)),
            kind: HitKind::DragUndoDelay,
            tooltip: Some(format!(
                "Undo-all delay: {:.1}s (drag)",
                snapshot.undo_all_delay_ms as f64 / 1000.0
            )),
        });

        let redo_label = format!(
            "Redo delay: {:.1}s",
            snapshot.redo_all_delay_ms as f64 / 1000.0
        );
        ctx.set_source_rgba(0.7, 0.7, 0.75, 0.9);
        ctx.move_to(x + sliders_w / 2.0 + 10.0, slider_start_y + 10.0);
        let _ = ctx.show_text(&redo_label);

        let redo_slider_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_REDO_OFFSET_Y;
        ctx.set_source_rgba(0.4, 0.4, 0.45, 0.7);
        draw_round_rect(ctx, x, redo_slider_y, sliders_w, slider_h, 3.0);
        let _ = ctx.fill();
        let redo_t = delay_t_from_ms(snapshot.redo_all_delay_ms);
        let redo_knob_x = x + redo_t * (sliders_w - slider_knob_r * 2.0) + slider_knob_r;
        ctx.set_source_rgba(0.25, 0.5, 0.95, 0.9);
        ctx.arc(
            redo_knob_x,
            redo_slider_y + slider_h / 2.0,
            slider_knob_r,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        let _ = ctx.fill();
        hits.push(HitRegion {
            rect: (
                x,
                redo_slider_y - hit_pad,
                sliders_w,
                slider_h + hit_pad * 2.0,
            ),
            event: ToolbarEvent::SetRedoDelay(delay_secs_from_t(redo_t)),
            kind: HitKind::DragRedoDelay,
            tooltip: Some(format!(
                "Redo-all delay: {:.1}s (drag)",
                snapshot.redo_all_delay_ms as f64 / 1000.0
            )),
        });
    }

    *y += custom_card_h + section_gap;
}
