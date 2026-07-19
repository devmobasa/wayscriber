//! Side-palette pane content: one module per section, dispatched in the
//! user's configured order with the same gating the built-in palette uses
//! (`ToolContext` + per-section hidden flags + block grouping).

mod actions;
mod arrow;
mod boards;
mod colors;
mod marker;
mod pages;
mod presets;
pub(in crate::toolbar_gtk) mod session_pane;
pub(in crate::toolbar_gtk) mod settings_pane;
mod step_marker;
mod step_undo;
mod text_controls;
mod thickness;

use gtk4::prelude::*;

use crate::label_format::format_binding_label;
use crate::toolbar_icons;
use crate::ui::toolbar::snapshot::ToolContext;
use crate::ui::toolbar::{
    SidePane, ToolbarEvent, ToolbarSideSection, ToolbarSnapshot, bindings, model,
};

use super::super::icons::IconPainter;
use super::super::widgets::{FeedbackSender, send_event, sized_button};
use super::Updater;

/// Everything a section builder needs; `updaters` collects the closures
/// that keep the built widgets in sync with later snapshots.
pub(in crate::toolbar_gtk) struct SectionCtx<'a> {
    pub(in crate::toolbar_gtk) snapshot: &'a ToolbarSnapshot,
    pub(in crate::toolbar_gtk) feedback: FeedbackSender,
    pub(in crate::toolbar_gtk) scale: f64,
    pub(in crate::toolbar_gtk) use_icons: bool,
    pub(in crate::toolbar_gtk) updaters: &'a mut Vec<Updater>,
}

impl SectionCtx<'_> {
    pub(in crate::toolbar_gtk) fn sz(&self, value: f64) -> f64 {
        value * self.scale
    }

    pub(in crate::toolbar_gtk) fn px(&self, value: f64) -> i32 {
        (value * self.scale).round() as i32
    }
}

/// Scope a section title to the tool it currently edits ("Color — Pen"),
/// mirroring the built-in `scoped_title`.
pub(in crate::toolbar_gtk) fn scoped_title(base: &str, snapshot: &ToolbarSnapshot) -> String {
    if !snapshot.context_aware_ui {
        return base.to_string();
    }
    let scope = if snapshot.text_active {
        "Text"
    } else if snapshot.note_active {
        "Note"
    } else {
        bindings::tool_label(snapshot.active_tool)
    };
    format!("{base} — {scope}")
}

/// Collapsible group card. The header toggles the section's collapsed
/// flag; the body is only built when expanded (collapse changes are
/// structural, so a rebuild follows the toggle).
pub(in crate::toolbar_gtk) struct SectionCard {
    pub(in crate::toolbar_gtk) root: gtk4::Box,
    pub(in crate::toolbar_gtk) body: gtk4::Box,
}

pub(in crate::toolbar_gtk) fn section_card(
    ctx: &SectionCtx,
    section: ToolbarSideSection,
    title: &str,
) -> SectionCard {
    let collapsed = ctx.snapshot.collapsed_side_sections.contains(&section);
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, ctx.px(4.0));
    root.add_css_class("card");

    let header = gtk4::Button::new();
    header.add_css_class("section-header");
    header.set_tooltip_text(Some(&format!(
        "{} {}",
        if collapsed { "Expand" } else { "Collapse" },
        title
    )));
    let header_row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(4.0));
    let label = gtk4::Label::new(Some(title));
    label.add_css_class("section-title");
    label.set_xalign(0.0);
    label.set_hexpand(true);
    header_row.append(&label);
    let chevron = super::super::icons::IconWidget::new(
        if collapsed {
            toolbar_icons::draw_icon_chevron_right
        } else {
            toolbar_icons::draw_icon_chevron_down
        },
        ctx.sz(12.0),
    );
    header_row.append(&chevron.area);
    header.set_child(Some(&header_row));
    let sender = ctx.feedback.clone();
    header.connect_clicked(move |_| {
        send_event(
            &sender,
            ToolbarEvent::ToggleSideSectionCollapsed(section, !collapsed),
        );
    });
    root.append(&header);

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, ctx.px(6.0));
    body.set_visible(!collapsed);
    root.append(&body);
    SectionCard { root, body }
}

/// Command row shared by Boards/Pages/Actions: icon mode packs plain
/// buttons left and destructive ones right; text mode uses an equal-width
/// grid. Enabled state tracks later snapshots through an updater.
pub(in crate::toolbar_gtk) fn command_row(
    ctx: &mut SectionCtx,
    buttons: &[model::ToolbarButtonModel],
    noun: &'static str,
    icon_for: fn(&ToolbarEvent) -> IconPainter,
    model_fn: fn(&ToolbarSnapshot) -> Option<model::ToolbarCommandGroup>,
) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, ctx.px(6.0));
    row.set_homogeneous(!ctx.use_icons);
    let btn_h = if ctx.use_icons {
        ctx.sz(32.0)
    } else {
        ctx.sz(24.0)
    };
    let mut handles: Vec<gtk4::Button> = Vec::new();
    let mut spacer_added = false;
    for button_model in buttons {
        let button = if ctx.use_icons {
            let handle = sized_button(btn_h, btn_h);
            let icon =
                super::super::icons::IconWidget::new(icon_for(&button_model.event), ctx.sz(18.0));
            handle.set_child(Some(&icon.area));
            handle
        } else {
            let handle = gtk4::Button::with_label(button_model.short_label(ctx.snapshot, noun));
            handle.set_size_request(-1, btn_h.round() as i32);
            handle
        };
        button.set_tooltip_text(Some(&format_binding_label(
            button_model.tooltip_label(ctx.snapshot, noun),
            button_model.binding_hint(ctx.snapshot),
        )));
        button.set_sensitive(button_model.enabled);
        if button_model.event.is_destructive() {
            button.add_css_class("destructive");
            if ctx.use_icons && !spacer_added {
                // Guard gap: destructive buttons sit apart on the right.
                let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
                spacer.set_hexpand(true);
                row.append(&spacer);
                spacer_added = true;
            }
        }
        let sender = ctx.feedback.clone();
        let event = button_model.event.clone();
        button.connect_clicked(move |_| {
            send_event(&sender, event.clone());
        });
        row.append(&button);
        handles.push(button);
    }
    ctx.updaters.push(Box::new(move |snapshot| {
        if let Some(group) = model_fn(snapshot) {
            for (handle, button_model) in handles.iter().zip(group.buttons.iter()) {
                handle.set_sensitive(button_model.enabled);
            }
        }
    }));
    row
}

/// Build the active pane's content, mirroring the built-in dispatch:
/// Session and Settings are whole panes; Draw and Canvas run their
/// sections in the user's order with `ToolContext` gating and the
/// thickness/text block grouping.
pub(in crate::toolbar_gtk) fn build_pane_content(
    snapshot: &ToolbarSnapshot,
    feedback: FeedbackSender,
    scale: f64,
    updaters: &mut Vec<Updater>,
) -> gtk4::Box {
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, (12.0 * scale).round() as i32);
    let mut ctx = SectionCtx {
        snapshot,
        feedback,
        scale,
        use_icons: snapshot.use_icons,
        updaters,
    };
    match snapshot.active_side_pane {
        SidePane::Session => {
            if let Some(widget) = session_pane::build(&mut ctx) {
                content.append(&widget);
            }
        }
        SidePane::Settings => {
            if let Some(widget) = settings_pane::build(&mut ctx) {
                content.append(&widget);
            }
        }
        SidePane::Draw | SidePane::Canvas => {
            build_pane_sections(&mut ctx, &content);
        }
    }
    content
}

fn build_pane_sections(ctx: &mut SectionCtx, content: &gtk4::Box) {
    let snapshot = ctx.snapshot;
    let tool_context = ToolContext::from_snapshot(snapshot);
    let mut thickness_block_built = false;
    let mut text_block_built = false;
    for section in model::ordered_pane_sections(snapshot) {
        let widget = match section {
            ToolbarSideSection::Colors
                if tool_context.needs_color
                    && !snapshot.side_section_hidden(ToolbarSideSection::Colors) =>
            {
                colors::build(ctx)
            }
            ToolbarSideSection::Presets
                if !snapshot.side_section_hidden(ToolbarSideSection::Presets) =>
            {
                presets::build(ctx)
            }
            ToolbarSideSection::Thickness
            | ToolbarSideSection::EraserMode
            | ToolbarSideSection::PolygonSides
                if tool_context.needs_thickness && !thickness_block_built =>
            {
                thickness_block_built = true;
                thickness::build(ctx)
            }
            ToolbarSideSection::ArrowLabels
                if tool_context.show_arrow_labels
                    && !snapshot.side_section_hidden(ToolbarSideSection::ArrowLabels) =>
            {
                arrow::build(ctx)
            }
            ToolbarSideSection::StepMarkers
                if tool_context.show_step_counter
                    && !snapshot.side_section_hidden(ToolbarSideSection::StepMarkers) =>
            {
                step_marker::build(ctx)
            }
            ToolbarSideSection::MarkerOpacity
                if tool_context.show_marker_opacity
                    && !snapshot.side_section_hidden(ToolbarSideSection::MarkerOpacity) =>
            {
                marker::build(ctx)
            }
            ToolbarSideSection::TextSize | ToolbarSideSection::Font
                if tool_context.show_font_controls && !text_block_built =>
            {
                text_block_built = true;
                text_controls::build(ctx)
            }
            ToolbarSideSection::Actions => actions::build(ctx),
            ToolbarSideSection::Boards => boards::build(ctx),
            ToolbarSideSection::Pages => pages::build(ctx),
            ToolbarSideSection::StepUndo => step_undo::build(ctx),
            _ => None,
        };
        if let Some(widget) = widget {
            content.append(&widget);
        }
    }
}
