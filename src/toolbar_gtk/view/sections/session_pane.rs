//! Session pane: active-session meta labels, the Open / Save As / Info /
//! Clear / Manager grid (replaced by the Save-As overwrite confirmation
//! while one is pending), and the recent-session list.

use gtk4::prelude::*;

use crate::label_format::format_binding_label;
use crate::toolbar_icons;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection, model};

use super::super::super::icons::{IconPainter, IconWidget};
use super::super::super::widgets::send_event;
use super::{SectionCtx, section_card};

pub(in crate::toolbar_gtk) fn build(ctx: &mut SectionCtx) -> Option<gtk4::Widget> {
    let session = model::ToolbarSessionModel::from_snapshot(ctx.snapshot)?;
    let card = section_card(
        ctx,
        ToolbarSideSection::Session,
        ToolbarSideSection::Session.label(),
    );

    let name_label = gtk4::Label::new(Some(&session.active_name));
    name_label.set_xalign(0.0);
    name_label.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    card.body.append(&name_label);
    // Keep the tail of the path — the leading directories are the least
    // informative part of a session path.
    let path_label = gtk4::Label::new(Some(&session.active_path_label));
    path_label.add_css_class("hint");
    path_label.set_xalign(0.0);
    path_label.set_ellipsize(gtk4::pango::EllipsizeMode::Start);
    card.body.append(&path_label);

    // A pending Save-As overwrite confirmation replaces the button grid.
    let mut handles: Vec<gtk4::Button> = Vec::new();
    if let Some(confirmation) = session.overwrite_confirmation.as_ref() {
        card.body
            .append(&overwrite_confirmation_rows(ctx, confirmation));
    } else {
        let columns = session.button_columns();
        let grid = gtk4::Grid::new();
        grid.set_column_homogeneous(true);
        grid.set_row_spacing(ctx.px(5.0) as u32);
        grid.set_column_spacing(ctx.px(5.0) as u32);
        for (index, button_model) in session.buttons.iter().enumerate() {
            let button = session_button(ctx, button_model);
            grid.attach(
                &button,
                (index % columns) as i32,
                (index / columns) as i32,
                1,
                1,
            );
            handles.push(button);
        }
        card.body.append(&grid);
    }

    for recent in &session.recents {
        card.body.append(&recent_row(ctx, recent));
    }

    ctx.updaters.push(Box::new(move |snapshot| {
        let Some(session) = model::ToolbarSessionModel::from_snapshot(snapshot) else {
            return;
        };
        name_label.set_text(&session.active_name);
        path_label.set_text(&session.active_path_label);
        for (handle, button_model) in handles.iter().zip(session.buttons.iter()) {
            handle.set_sensitive(button_model.enabled);
        }
    }));
    Some(card.root.upcast())
}

fn session_button(ctx: &SectionCtx, button_model: &model::ToolbarSessionButton) -> gtk4::Button {
    let button = gtk4::Button::new();
    button.set_size_request(-1, ctx.px(24.0));
    button.set_hexpand(true);
    if let Some(painter) = session_button_icon(&button_model.event).filter(|_| ctx.use_icons) {
        let content = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(4.0));
        content.set_halign(gtk4::Align::Center);
        let icon = IconWidget::new(painter, ctx.sz(12.0));
        content.append(&icon.area);
        content.append(&gtk4::Label::new(Some(button_model.label)));
        button.set_child(Some(&content));
    } else {
        button.set_label(button_model.label);
    }
    button.set_tooltip_text(Some(button_model.label));
    button.set_sensitive(button_model.enabled);
    if matches!(button_model.event, ToolbarEvent::ClearSession) {
        button.add_css_class("destructive");
    }
    let sender = ctx.feedback.clone();
    let event = button_model.event.clone();
    button.connect_clicked(move |_| {
        send_event(&sender, event.clone());
    });
    button
}

fn session_button_icon(event: &ToolbarEvent) -> Option<IconPainter> {
    Some(match event {
        ToolbarEvent::OpenSession => toolbar_icons::draw_icon_file,
        ToolbarEvent::SaveSessionAs => toolbar_icons::draw_icon_save,
        ToolbarEvent::SessionInfo => toolbar_icons::draw_icon_info,
        ToolbarEvent::ClearSession => toolbar_icons::draw_icon_clear,
        ToolbarEvent::OpenConfigurator => toolbar_icons::draw_icon_settings,
        _ => return None,
    })
}

fn overwrite_confirmation_rows(
    ctx: &SectionCtx,
    confirmation: &model::session::ToolbarSessionOverwriteConfirmation,
) -> gtk4::Box {
    let rows = gtk4::Box::new(gtk4::Orientation::Vertical, ctx.px(5.0));
    let message = gtk4::Label::new(Some(&format!("Replace {}?", confirmation.label)));
    message.set_xalign(0.0);
    message.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    rows.append(&message);

    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(5.0));
    buttons.set_homogeneous(true);
    let actions = [
        ("Replace", confirmation.confirm_event(), true),
        ("Cancel", confirmation.cancel_event(), false),
    ];
    for (label, event, destructive) in actions {
        let button = gtk4::Button::with_label(label);
        button.set_size_request(-1, ctx.px(24.0));
        button.set_tooltip_text(Some(label));
        if destructive {
            button.add_css_class("destructive");
        }
        let sender = ctx.feedback.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        buttons.append(&button);
    }
    rows.append(&buttons);
    rows
}

fn recent_row(ctx: &SectionCtx, recent: &model::ToolbarSessionRecent) -> gtk4::Button {
    let button = gtk4::Button::new();
    button.set_size_request(-1, ctx.px(22.0));
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(6.0));
    row.set_margin_start(ctx.px(7.0));
    row.set_margin_end(ctx.px(7.0));
    let icon = IconWidget::new(toolbar_icons::draw_icon_file, ctx.sz(13.0));
    row.append(&icon.area);
    // Middle-ellipsize so both the head and the distinguishing tail of the
    // file name survive; the constant extension is dropped up front.
    let label = gtk4::Label::new(Some(strip_session_extension(&recent.label)));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
    row.append(&label);
    button.set_child(Some(&row));
    button.set_tooltip_text(Some(&format_binding_label(
        &format!("Open {}", recent.label),
        Some(&recent.path.display().to_string()),
    )));
    let sender = ctx.feedback.clone();
    let event = recent.event();
    button.connect_clicked(move |_| {
        send_event(&sender, event.clone());
    });
    button
}

/// Drop the constant session-file extension in list rows; it costs the
/// characters that distinguish one session from another.
fn strip_session_extension(value: &str) -> &str {
    value
        .strip_suffix(".wayscriber-session")
        .or_else(|| value.strip_suffix(".wayscriber"))
        .unwrap_or(value)
}
