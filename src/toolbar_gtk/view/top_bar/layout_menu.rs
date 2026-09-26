//! The chrome island's layout-preset menu.
//!
//! Rows come from the shared `model::layout_menu_entries`, so this menu lists
//! the same presets with the same wording as the built-in one.

use super::*;

impl TopBar {
    /// Keep the layout menu's content and open state in line with the
    /// snapshot. Content rebuilds only when the current preset changes.
    pub(super) fn sync_layout_menu(&mut self, snapshot: &ToolbarSnapshot, scale: f64) {
        let Some(resources) = self.layout.mounted.clone() else {
            self.layout.expected_open.set(false);
            return;
        };

        let open = snapshot.layout_menu_open;
        if open && self.layout.content_key != Some(snapshot.layout_mode) {
            resources
                .capture_surface
                .set_content(&self.build_layout_menu_content(snapshot, scale));
            self.layout.content_key = Some(snapshot.layout_mode);
        }
        self.layout.set_open(open);
    }

    /// Layout menu content: one row per preset, the current preset marked.
    /// Choosing a row applies the preset; the backend closes the menu.
    pub(super) fn build_layout_menu_content(
        &self,
        snapshot: &ToolbarSnapshot,
        scale: f64,
    ) -> gtk4::Box {
        let gap = (model::LAYOUT_MENU_ROW_GAP * scale).round() as i32;
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, gap);
        set_semantic_widget_id(&content, "top.layout.panel");
        install_click_modifier_capture(&content, &self.feedback);

        for entry in model::layout_menu_entries(snapshot.layout_mode) {
            content.append(&self.layout_menu_row(&entry, scale));
        }
        content
    }

    fn layout_menu_row(&self, entry: &model::LayoutMenuEntry, scale: f64) -> gtk4::Button {
        let row = sized_button(
            model::LAYOUT_MENU_ROW_W * scale,
            model::LAYOUT_MENU_ROW_H * scale,
        );
        set_semantic_widget_id(
            &row,
            &format!(
                "top.layout.{}",
                model::layout_mode_label(entry.mode).to_ascii_lowercase()
            ),
        );
        row.set_tooltip_text(Some(&format!("Switch to the {} layout", entry.label)));
        row.update_property(&[gtk4::accessible::Property::Label(&format!(
            "{} layout: {}",
            entry.label, entry.description
        ))]);
        set_active_class(&row, entry.current);

        let text = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        text.set_valign(gtk4::Align::Center);
        text.set_margin_start((10.0 * scale).round() as i32);
        let name_text = if entry.current {
            format!("\u{2713} {}", entry.label)
        } else {
            entry.label.to_string()
        };
        let name = gtk4::Label::new(Some(&name_text));
        name.set_halign(gtk4::Align::Start);
        name.add_css_class("section-title");
        let description = gtk4::Label::new(Some(entry.description));
        description.set_halign(gtk4::Align::Start);
        description.add_css_class("hint");
        text.append(&name);
        text.append(&description);
        row.set_child(Some(&text));

        let sender = self.feedback.clone();
        let event = entry.event.clone();
        row.connect_clicked(move |_| send_event(&sender, event.clone()));
        row
    }
}
