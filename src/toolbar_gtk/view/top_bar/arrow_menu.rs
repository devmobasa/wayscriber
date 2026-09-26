//! The GTK style pill's arrow style chip and the menu popover it opens.
//!
//! Mirrors the builtin `view/top/arrow_menu.rs`: the rows, wording, and
//! geometry come from the shared `model::arrow_style_*`, and the previews are
//! drawn by the painter both frontends share. The popover follows the layout
//! menu's pattern: no autohide grab (the backend's dismissal policy owns
//! click-away), keys relayed to the overlay, and `closed` echoing a user
//! dismissal. Choosing a style closes the menu from the backend.

use crate::draw::ArrowStyle;

use super::*;

/// Space between a preview and the name after it, as in the builtin menu.
const PREVIEW_GAP: f64 = 8.0;

impl TopBar {
    /// Appends the arrow style chip to `pill`, followed by `gap_px` of
    /// margin, installs the menu popover anchored to it, and registers the
    /// chip's updater.
    pub(super) fn append_arrow_style_chip(
        &mut self,
        pill: &gtk4::Box,
        control: model::StylePillControl,
        snapshot: &ToolbarSnapshot,
        scale: f64,
        gap_px: i32,
    ) {
        let px = |value: f64| (value * scale).round() as i32;
        let chip = sized_button(model::ARROW_STYLE_CHIP_W * scale, STYLE_ROW_H * scale);
        set_semantic_widget_id(&chip, control.id().as_ref());
        let content = gtk4::Box::new(gtk4::Orientation::Horizontal, px(PREVIEW_GAP / 2.0));
        content.set_margin_start(px(model::ARROW_STYLE_MENU_INSET));
        let glyph = ArrowStylePreviewArea::new(
            snapshot.arrow_style,
            &format!("{}.glyph", control.id()),
            (
                px(model::ARROW_STYLE_CHIP_GLYPH_W),
                px(model::ARROW_STYLE_CHIP_GLYPH_H),
            ),
        );
        let label = gtk4::Label::new(Some(&model::arrow_style_chip_label(snapshot.arrow_style)));
        label.set_xalign(0.0);
        content.append(&glyph.area);
        content.append(&label);
        chip.set_child(Some(&content));

        let sender = self.feedback.clone();
        let expected = self.arrow_style.expected_open.clone();
        chip.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::ToggleArrowStyleMenu(!expected.get()));
        });
        let handle = chip.clone();
        let refresh = move |snapshot: &ToolbarSnapshot| {
            handle.set_tooltip_text(control.tooltip(snapshot).as_deref());
            handle.update_property(&[gtk4::accessible::Property::Label(&format!(
                "{}: {}",
                control.label(snapshot),
                snapshot.arrow_style.label()
            ))]);
            set_active_class(&handle, control.active(snapshot));
            if glyph.set_style(snapshot.arrow_style) {
                label.set_label(&model::arrow_style_chip_label(snapshot.arrow_style));
            }
        };
        refresh(snapshot);
        self.updaters.borrow_mut().push(Box::new(refresh));

        let popover = gtk4::Popover::new();
        popover.set_parent(&chip);
        popover.set_position(gtk4::PositionType::Bottom);
        popover.set_autohide(false);
        install_key_relay(&popover, &self.feedback);
        let sender = self.feedback.clone();
        let expected = self.arrow_style.expected_open.clone();
        popover.connect_closed(move |_| {
            if expected.get() {
                send_event(&sender, ToolbarEvent::ToggleArrowStyleMenu(false));
            }
        });
        let capture_surface = CaptureSurfaceContent::empty();
        popover.set_child(Some(capture_surface.widget()));
        self.arrow_style.install(popover, capture_surface);

        chip.set_margin_end(gap_px.max(0));
        pill.append(&chip);
    }

    /// Keep the menu's content and open state in line with the snapshot.
    /// Content rebuilds only when the current style changes.
    pub(super) fn sync_arrow_style_menu(&mut self, snapshot: &ToolbarSnapshot, scale: f64) {
        let Some(resources) = self.arrow_style.mounted.clone() else {
            self.arrow_style.expected_open.set(false);
            return;
        };

        let open = snapshot.arrow_style_menu_open;
        if open && self.arrow_style.content_key != Some(snapshot.arrow_style) {
            resources
                .capture_surface
                .set_content(&self.build_arrow_style_menu_content(snapshot, scale));
            self.arrow_style.content_key = Some(snapshot.arrow_style);
        }
        self.arrow_style.set_open(open);
    }

    /// Menu content: one row per style, its preview drawn and the current
    /// style marked. Choosing a row sets the style; the backend closes the
    /// menu.
    pub(super) fn build_arrow_style_menu_content(
        &self,
        snapshot: &ToolbarSnapshot,
        scale: f64,
    ) -> gtk4::Box {
        let gap = (model::ARROW_STYLE_MENU_ROW_GAP * scale).round() as i32;
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, gap);
        set_semantic_widget_id(&content, "top.arrow-style.panel");
        install_click_modifier_capture(&content, &self.feedback);

        for entry in model::arrow_style_menu_entries(snapshot.arrow_style) {
            content.append(&self.arrow_style_menu_row(&entry, scale));
        }
        content
    }

    fn arrow_style_menu_row(&self, entry: &model::ArrowStyleMenuEntry, scale: f64) -> gtk4::Button {
        let px = |value: f64| (value * scale).round() as i32;
        let id = entry.id();
        let row = sized_button(
            model::ARROW_STYLE_MENU_ROW_W * scale,
            model::ARROW_STYLE_MENU_ROW_H * scale,
        );
        set_semantic_widget_id(&row, &id);
        row.set_tooltip_text(Some(&entry.tooltip()));
        row.update_property(&[gtk4::accessible::Property::Label(&format!(
            "{} arrow: {}",
            entry.label, entry.hint
        ))]);
        set_active_class(&row, entry.current);

        let content = gtk4::Box::new(gtk4::Orientation::Horizontal, px(PREVIEW_GAP));
        content.set_margin_start(px(model::ARROW_STYLE_MENU_INSET));
        let preview = ArrowStylePreviewArea::new(
            entry.style,
            &format!("{id}.preview"),
            (
                px(model::ARROW_STYLE_MENU_PREVIEW_W),
                px(model::ARROW_STYLE_MENU_PREVIEW_H),
            ),
        );
        let name = gtk4::Label::new(Some(entry.label));
        name.set_xalign(0.0);
        name.add_css_class("section-title");
        set_semantic_widget_id(&name, &format!("{id}.name"));
        content.append(&preview.area);
        content.append(&name);
        row.set_child(Some(&content));

        let sender = self.feedback.clone();
        let event = entry.event.clone();
        row.connect_clicked(move |_| send_event(&sender, event.clone()));
        row
    }
}

/// An arrow style drawn by the painter the builtin toolbar shares.
struct ArrowStylePreviewArea {
    area: gtk4::DrawingArea,
    style: Rc<Cell<ArrowStyle>>,
}

impl ArrowStylePreviewArea {
    fn new(style: ArrowStyle, id: &str, (width, height): (i32, i32)) -> Self {
        // Decoration: the button around it names the style.
        let area = gtk4::DrawingArea::builder()
            .accessible_role(gtk4::AccessibleRole::Presentation)
            .build();
        area.set_content_width(width);
        area.set_content_height(height);
        area.set_valign(gtk4::Align::Center);
        area.set_can_target(false);
        set_semantic_widget_id(&area, id);
        let style = Rc::new(Cell::new(style));
        let draw_style = style.clone();
        area.set_draw_func(move |_, ctx, width, height| {
            crate::toolbar_icons::draw_arrow_style_preview(
                ctx,
                (0.0, 0.0, f64::from(width), f64::from(height)),
                draw_style.get(),
                crate::ui::theme::toolbar::COLOR_TEXT_PRIMARY,
            );
        });
        Self { area, style }
    }

    /// Show `style`; returns whether it changed.
    fn set_style(&self, style: ArrowStyle) -> bool {
        let changed = self.style.replace(style) != style;
        if changed {
            self.area.queue_draw();
        }
        changed
    }
}
