//! Settings pane: layout-mode presets, feature toggles, the settings
//! button grid, and the toolbar-item customization sub-panel (group
//! chooser plus per-item show/hide and reorder rows).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;

use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection, model};

use super::super::super::icons::{IconPainter, IconWidget};
use super::super::super::widgets::{send_event, set_active_class, text_button};
use super::{SectionCtx, section_card};

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let settings_model = model::ToolbarSettingsModel::from_snapshot(ctx.snapshot)?;
    let customizing = ctx.snapshot.customize_items_open;
    let title = if customizing {
        "Customize toolbar"
    } else {
        ToolbarSideSection::Settings.label()
    };
    let card = section_card(ctx, ToolbarSideSection::Settings, title);
    if customizing {
        // The customization sub-panel is a dedicated view: it stays open
        // even while the Settings section is flagged collapsed.
        card.body.set_visible(true);
    }

    if !customizing {
        card.body.append(&layout_mode_segments(ctx));
    }
    if let Some(grid) = toggle_grid(ctx, &settings_model) {
        card.body.append(&grid);
    }
    if let Some(grid) = buttons_grid(ctx, settings_model.buttons()) {
        card.body.append(&grid);
    }
    if let Some(chooser) = group_chooser(ctx, &settings_model) {
        card.body.append(&chooser);
    }
    if let Some(items) = item_override_rows(ctx, &settings_model) {
        card.body.append(&items);
    }
    Some(card.root.upcast())
}

/// Simple / Regular / Advanced presets. Non-destructive: switching
/// re-baselines the sections without touching explicit user overrides.
fn layout_mode_segments(ctx: &mut SectionCtx) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(4.0));
    row.set_homogeneous(true);
    let control = model::layout_mode_control(ctx.snapshot.layout_mode);
    let model::ToolbarControlKind::Segmented(segmented) = &control.kind else {
        return row;
    };
    let active = segmented.active_segment();
    let mut handles: Vec<(gtk4::Button, model::ToolbarControlId)> = Vec::new();
    for segment in segmented.segments() {
        let button = gtk4::Button::with_label(segment.label.as_ref());
        button.add_css_class("tab");
        button.set_size_request(-1, ctx.px(22.0));
        button.set_sensitive(segment.enabled);
        if let Some(tooltip) = segment.tooltip.as_string() {
            button.set_tooltip_text(Some(&tooltip));
        }
        set_active_class(&button, active == Some(segment.id));
        let sender = ctx.feedback.clone();
        let event = segment.activation.compatibility_event();
        button.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        handles.push((button.clone(), segment.id));
        row.append(&button);
    }
    ctx.updaters.push(Box::new(move |snapshot| {
        let control = model::layout_mode_control(snapshot.layout_mode);
        let model::ToolbarControlKind::Segmented(segmented) = &control.kind else {
            return;
        };
        let active = segmented.active_segment();
        for (button, id) in &handles {
            set_active_class(button, active == Some(*id));
        }
    }));
    row
}

/// Handle keeping one settings toggle in sync: the checked state follows
/// the snapshot and the pending event always flips the latest value.
struct ToggleSync {
    id: model::ToolbarControlId,
    check: gtk4::CheckButton,
    event: Rc<RefCell<ToolbarEvent>>,
    syncing: Rc<Cell<bool>>,
}

/// Two-column toggle grid: wide toggles span the full row, narrow ones
/// pair up, and a lone narrow leftover keeps its half-width cell.
fn toggle_grid(
    ctx: &mut SectionCtx,
    settings_model: &model::ToolbarSettingsModel,
) -> Option<gtk4::Grid> {
    let rows = settings_model.toggle_rows();
    if rows.is_empty() {
        return None;
    }
    let grid = gtk4::Grid::new();
    grid.set_column_homogeneous(true);
    grid.set_column_spacing(ctx.px(6.0) as u32);
    grid.set_row_spacing(ctx.px(6.0) as u32);
    let mut handles: Vec<ToggleSync> = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let full_row = row.len() == 1 && row[0].wide;
        for (col, toggle) in row.iter().enumerate() {
            let check = gtk4::CheckButton::with_label(toggle.label.as_ref());
            check.add_css_class("mini");
            check.set_active(toggle.checked);
            check.set_size_request(-1, ctx.px(24.0));
            if let Some(tooltip) = toggle.tooltip.as_string() {
                check.set_tooltip_text(Some(&tooltip));
            }
            let event = Rc::new(RefCell::new(toggle.activation.compatibility_event()));
            let syncing = Rc::new(Cell::new(false));
            let sender = ctx.feedback.clone();
            let toggle_event = event.clone();
            let toggle_sync = syncing.clone();
            check.connect_toggled(move |_| {
                if !toggle_sync.get() {
                    let event = toggle_event.borrow().clone();
                    send_event(&sender, event);
                }
            });
            let width = if full_row { 2 } else { 1 };
            grid.attach(&check, col as i32, row_index as i32, width, 1);
            handles.push(ToggleSync {
                id: toggle.id,
                check,
                event,
                syncing,
            });
        }
    }
    ctx.updaters.push(Box::new(move |snapshot| {
        let Some(fresh) = model::ToolbarSettingsModel::from_snapshot(snapshot) else {
            return;
        };
        for handle in &handles {
            let Some(toggle) = fresh.toggles().iter().find(|toggle| toggle.id == handle.id) else {
                continue;
            };
            *handle.event.borrow_mut() = toggle.activation.compatibility_event();
            if handle.check.is_active() != toggle.checked {
                handle.syncing.set(true);
                handle.check.set_active(toggle.checked);
                handle.syncing.set(false);
            }
        }
    }));
    Some(grid)
}

/// Settings/customize button grid (Customize · Reset hidden · Configurator
/// · Config file, or Back · Reset hidden · Reset order while customizing).
/// The button set is structural (customize state, hidden overrides), so a
/// rebuild follows every change and no updaters are needed.
fn buttons_grid(ctx: &SectionCtx, buttons: &[model::ToolbarSettingsButton]) -> Option<gtk4::Grid> {
    if buttons.is_empty() {
        return None;
    }
    let grid = gtk4::Grid::new();
    grid.set_column_homogeneous(true);
    grid.set_column_spacing(ctx.px(6.0) as u32);
    grid.set_row_spacing(ctx.px(6.0) as u32);
    for (index, button_model) in buttons.iter().enumerate() {
        let button = settings_button(ctx, button_model);
        grid.attach(&button, (index % 2) as i32, (index / 2) as i32, 1, 1);
    }
    Some(grid)
}

fn settings_button(ctx: &SectionCtx, button_model: &model::ToolbarSettingsButton) -> gtk4::Button {
    let button = gtk4::Button::new();
    button.set_size_request(-1, ctx.px(24.0));
    if ctx.use_icons {
        // Icon plus a left-aligned label, mirroring the built-in treatment:
        // icon-only glyphs were ambiguous here.
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(5.0));
        row.set_margin_start(ctx.px(6.0));
        row.set_margin_end(ctx.px(6.0));
        let icon = IconWidget::new(settings_icon_painter(button_model.icon), ctx.sz(16.0));
        row.append(&icon.area);
        let label = gtk4::Label::new(Some(button_model.label.as_ref()));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        row.append(&label);
        button.set_child(Some(&row));
    } else {
        button.set_label(button_model.label.as_ref());
    }
    if let Some(tooltip) = button_model.tooltip.as_string() {
        button.set_tooltip_text(Some(&tooltip));
    }
    let sender = ctx.feedback.clone();
    let event = button_model.event.clone();
    button.connect_clicked(move |_| {
        send_event(&sender, event.clone());
    });
    button
}

/// Customize-mode group chooser: "Choose a group" over a two-column grid.
fn group_chooser(
    ctx: &SectionCtx,
    settings_model: &model::ToolbarSettingsModel,
) -> Option<gtk4::Box> {
    let groups = settings_model.groups();
    if groups.is_empty() {
        return None;
    }
    let column = gtk4::Box::new(gtk4::Orientation::Vertical, ctx.px(6.0));
    column.append(&sub_header("Choose a group"));
    let grid = gtk4::Grid::new();
    grid.set_column_homogeneous(true);
    grid.set_column_spacing(ctx.px(6.0) as u32);
    grid.set_row_spacing(ctx.px(6.0) as u32);
    for (index, group) in groups.iter().enumerate() {
        let button = gtk4::Button::with_label(group.label.as_ref());
        button.set_size_request(-1, ctx.px(24.0));
        if let Some(tooltip) = group.tooltip.as_string() {
            button.set_tooltip_text(Some(&tooltip));
        }
        let sender = ctx.feedback.clone();
        let event = group.event.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        grid.attach(&button, (index % 2) as i32, (index / 2) as i32, 1, 1);
    }
    column.append(&grid);
    Some(column)
}

/// Per-item override rows for the selected group: a show/hide checkbox
/// plus, for orderable items, up/down move buttons. Show/hide and order
/// live in `resolved_toolbar_items` (structural), so every change
/// rebuilds the pane and the rows need no updaters.
fn item_override_rows(
    ctx: &SectionCtx,
    settings_model: &model::ToolbarSettingsModel,
) -> Option<gtk4::Box> {
    let overrides = settings_model.item_overrides();
    if overrides.is_empty() {
        return None;
    }
    let column = gtk4::Box::new(gtk4::Orientation::Vertical, ctx.px(6.0));
    let header = ctx
        .snapshot
        .customize_items_group
        .map_or("Uncheck items to hide", |group| group.label());
    column.append(&sub_header(header));
    for override_item in overrides {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(4.0));
        let check = gtk4::CheckButton::with_label(override_item.label.as_ref());
        check.add_css_class("mini");
        check.set_active(override_item.shown);
        check.set_hexpand(true);
        check.set_size_request(-1, ctx.px(24.0));
        if let Some(tooltip) = override_item.tooltip.as_string() {
            check.set_tooltip_text(Some(&tooltip));
        }
        let sender = ctx.feedback.clone();
        let id = override_item.id;
        check.connect_toggled(move |check| {
            send_event(
                &sender,
                ToolbarEvent::SetToolbarItemHidden(id, !check.is_active()),
            );
        });
        row.append(&check);
        if let Some(order) = override_item.order.as_ref() {
            for (label, enabled, activation, tooltip) in [
                ("^", order.can_move_up, &order.move_up, "Move up"),
                ("v", order.can_move_down, &order.move_down, "Move down"),
            ] {
                let button = text_button(
                    label,
                    (ctx.sz(28.0), ctx.sz(24.0)),
                    &format!("{} {}", tooltip, override_item.label),
                );
                button.set_sensitive(enabled);
                let sender = ctx.feedback.clone();
                let event = activation.compatibility_event();
                button.connect_clicked(move |_| {
                    send_event(&sender, event.clone());
                });
                row.append(&button);
            }
        }
        column.append(&row);
    }
    Some(column)
}

fn sub_header(text: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(text));
    label.add_css_class("section-title");
    label.set_xalign(0.0);
    label
}

fn settings_icon_painter(icon: model::ToolbarIcon) -> IconPainter {
    match icon {
        model::ToolbarIcon::Back => draw_back_icon,
        model::ToolbarIcon::Settings => toolbar_icons::draw_icon_settings,
        model::ToolbarIcon::Search => toolbar_icons::draw_icon_search,
        model::ToolbarIcon::Visibility => toolbar_icons::draw_icon_visibility,
        model::ToolbarIcon::File => toolbar_icons::draw_icon_file,
        model::ToolbarIcon::More | model::ToolbarIcon::Board => draw_no_icon,
    }
}

/// Back chevron, drawn exactly like the built-in settings panel draws it.
fn draw_back_icon(ctx: &cairo::Context, x: f64, y: f64, size: f64) {
    let mid_y = y + size * 0.5;
    ctx.set_line_width(2.0);
    ctx.move_to(x + size * 0.65, y + size * 0.25);
    ctx.line_to(x + size * 0.35, mid_y);
    ctx.line_to(x + size * 0.65, y + size * 0.75);
    let _ = ctx.stroke();
}

fn draw_no_icon(_ctx: &cairo::Context, _x: f64, _y: f64, _size: f64) {}
