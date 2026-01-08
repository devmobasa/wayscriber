use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::config::action_label;
use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};
use crate::ui::toolbar::bindings::{action_for_clear_preset, action_for_save_preset};

use super::super::super::super::widgets::{draw_button, point_in_rect, set_icon_color};
use super::PresetSlotLayout;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_preset_actions(
    ctx: &cairo::Context,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
    layout_spec: &PresetSlotLayout,
    slot_x: f64,
    slot: usize,
    preset_exists: bool,
    hover: Option<(f64, f64)>,
) {
    let save_hover = hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                slot_x,
                layout_spec.action_row_y,
                layout_spec.action_w,
                layout_spec.action_h,
            )
        })
        .unwrap_or(false);
    draw_button(
        ctx,
        slot_x,
        layout_spec.action_row_y,
        layout_spec.action_w,
        layout_spec.action_h,
        false,
        save_hover,
    );
    set_icon_color(ctx, save_hover);
    toolbar_icons::draw_icon_save(
        ctx,
        slot_x + (layout_spec.action_w - layout_spec.action_icon) / 2.0,
        layout_spec.action_row_y + (layout_spec.action_h - layout_spec.action_icon) / 2.0,
        layout_spec.action_icon,
    );
    hits.push(HitRegion {
        rect: (
            slot_x,
            layout_spec.action_row_y,
            layout_spec.action_w,
            layout_spec.action_h,
        ),
        event: ToolbarEvent::SavePreset(slot),
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            action_for_save_preset(slot)
                .map(action_label)
                .unwrap_or("Save Preset"),
            snapshot.binding_hints.save_preset(slot),
        )),
    });

    let clear_x = slot_x + layout_spec.action_w + layout_spec.action_gap;
    let clear_y = layout_spec.action_row_y;
    let clear_hover = hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                clear_x,
                clear_y,
                layout_spec.action_w,
                layout_spec.action_h,
            )
        })
        .unwrap_or(false)
        && preset_exists;
    draw_button(
        ctx,
        clear_x,
        clear_y,
        layout_spec.action_w,
        layout_spec.action_h,
        false,
        clear_hover,
    );
    if preset_exists {
        set_icon_color(ctx, clear_hover);
    } else {
        ctx.set_source_rgba(0.7, 0.7, 0.7, 0.6);
    }
    toolbar_icons::draw_icon_clear(
        ctx,
        clear_x + (layout_spec.action_w - layout_spec.action_icon) / 2.0,
        clear_y + (layout_spec.action_h - layout_spec.action_icon) / 2.0,
        layout_spec.action_icon,
    );
    if preset_exists {
        hits.push(HitRegion {
            rect: (clear_x, clear_y, layout_spec.action_w, layout_spec.action_h),
            event: ToolbarEvent::ClearPreset(slot),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_for_clear_preset(slot)
                    .map(action_label)
                    .unwrap_or("Clear Preset"),
                snapshot.binding_hints.clear_preset(slot),
            )),
        });
    }
}
