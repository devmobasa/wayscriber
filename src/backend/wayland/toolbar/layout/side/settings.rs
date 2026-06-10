use super::{HitKind, HitRegion, SideLayoutContext, ToolbarLayoutSpec};
use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
use crate::ui::toolbar::ToolbarSideSection;
use crate::ui::toolbar::model::{ToolbarActivation, ToolbarSettingsModel};

pub(super) fn push_settings_hits(ctx: &SideLayoutContext<'_>, y: f64, hits: &mut Vec<HitRegion>) {
    let Some(settings_model) = ToolbarSettingsModel::from_snapshot(ctx.snapshot) else {
        return;
    };

    super::section_header::push_collapsible_header_hit(ctx, y, ToolbarSideSection::Settings, hits);
    let dedicated_panel = ctx.snapshot.customize_items_open
        || matches!(
            ctx.snapshot.drawer_tab,
            crate::input::ToolbarDrawerTab::Sections | crate::input::ToolbarDrawerTab::Customize
        );
    if !dedicated_panel
        && ctx
            .snapshot
            .side_section_collapsed(ToolbarSideSection::Settings)
    {
        return;
    }

    let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
    let toggles = settings_model.toggles();

    let toggle_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let toggle_col_gap = toggle_gap;
    let toggle_col_w = row_item_width(ctx.content_width, 2, toggle_col_gap);
    let toggle_layout = grid_layout(
        ctx.x,
        toggle_y,
        toggle_col_w,
        toggle_h,
        toggle_col_gap,
        toggle_gap,
        2,
        toggles.len(),
    );
    for (item, toggle) in toggle_layout.items.iter().zip(toggles.iter()) {
        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: activation_event(&toggle.activation),
            kind: HitKind::Click,
            tooltip: toggle.tooltip.as_string(),
        });
    }

    let mut buttons_y = toggle_y;
    if toggle_layout.rows > 0 {
        buttons_y += toggle_layout.height;
    }
    buttons_y += toggle_gap;
    let button_h = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_HEIGHT;
    let button_gap = ToolbarLayoutSpec::SIDE_SETTINGS_BUTTON_GAP;
    let button_w = row_item_width(ctx.content_width, 2, button_gap);
    let buttons = settings_model.buttons();
    let button_layout = grid_layout(
        ctx.x,
        buttons_y,
        button_w,
        button_h,
        button_gap,
        0.0,
        2,
        buttons.len(),
    );
    for (item, button) in button_layout.items.iter().zip(buttons.iter()) {
        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: button.event.clone(),
            kind: HitKind::Click,
            tooltip: button.tooltip.as_string(),
        });
    }

    let mut customize_y = buttons_y;
    if button_layout.rows > 0 {
        customize_y += button_layout.height;
    }
    customize_y += toggle_gap;
    let groups = settings_model.groups();
    let group_layout = grid_layout(
        ctx.x,
        customize_y + toggle_h + toggle_gap,
        button_w,
        button_h,
        button_gap,
        button_gap,
        2,
        groups.len(),
    );
    for (item, group) in group_layout.items.iter().zip(groups.iter()) {
        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: group.event.clone(),
            kind: HitKind::Click,
            tooltip: group.tooltip.as_string(),
        });
    }

    let mut items_y = customize_y;
    if group_layout.rows > 0 {
        items_y += toggle_h + toggle_gap + group_layout.height + toggle_gap;
    }
    let item_overrides = settings_model.item_overrides();
    if !item_overrides.is_empty() {
        items_y += toggle_h + toggle_gap;
    }
    let item_layout = grid_layout(
        ctx.x,
        items_y,
        ctx.content_width,
        toggle_h,
        toggle_col_gap,
        toggle_gap,
        1,
        item_overrides.len(),
    );
    for (item, override_item) in item_layout.items.iter().zip(item_overrides.iter()) {
        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: activation_event(&override_item.activation),
            kind: HitKind::Click,
            tooltip: override_item.tooltip.as_string(),
        });
    }
}

fn activation_event(activation: &ToolbarActivation) -> crate::ui::toolbar::ToolbarEvent {
    activation.compatibility_event()
}
