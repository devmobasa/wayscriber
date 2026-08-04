use super::*;

/// The Iced view's triplet picker: a hex field, a popup picker, and the three
/// raw components. The native color dialog stands in for the popup; both feed
/// the same `ColorPickerHexChanged` path the popup's hex field used.
pub(super) struct ColorRow {
    pub(super) row: gtk::ListBoxRow,
    hex: gtk::Entry,
    hex_handler: SignalHandlerId,
    swatch: gtk::ColorDialogButton,
    swatch_handler: SignalHandlerId,
    components: [ComponentEntry; 3],
}

struct ComponentEntry {
    entry: gtk::Entry,
    handler: SignalHandlerId,
}

const COMPONENT_PLACEHOLDERS: [&str; 3] = ["R", "G", "B"];

impl ColorRow {
    pub(super) fn refresh(&self, values: &ColorValues<'_>) {
        set_text_blocked(&self.hex, &self.hex_handler, values.hex);
        // The same predicate the save gate counts with, so a field styled
        // clean can never be one the save refuses.
        mark_hex_error(&self.hex, values.hex);

        for (component, value) in self.components.iter().zip(values.color.components.iter()) {
            set_text_blocked(&component.entry, &component.handler, value);
        }

        let rgb = parse_triplet_values(&values.color.components);
        let rgba = gtk::gdk::RGBA::new(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32, 1.0);
        set_swatch_blocked(&self.swatch, &self.swatch_handler, &rgba);
    }
}

pub(super) fn build_color_row(
    title: &str,
    id: ColorPickerId,
    index: usize,
    to_component: fn(usize, usize, String) -> Message,
    sender: &ComponentSender<ConfiguratorApp>,
) -> ColorRow {
    let controls = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();

    let hex = gtk::Entry::builder()
        .placeholder_text("#RRGGBB")
        .width_chars(9)
        .max_width_chars(9)
        .build();
    let hex_handler = {
        let sender = sender.clone();
        hex.connect_changed(move |entry| {
            sender.input(Message::ColorPickerHexChanged(id, entry.text().to_string()));
        })
    };
    controls.append(&hex);

    let swatch =
        gtk::ColorDialogButton::new(Some(gtk::ColorDialog::builder().with_alpha(false).build()));
    swatch.set_valign(gtk::Align::Center);
    let swatch_handler = {
        let sender = sender.clone();
        swatch.connect_rgba_notify(move |button| {
            sender.input(Message::ColorPickerHexChanged(
                id,
                dialog_hex(&button.rgba()),
            ));
        })
    };
    controls.append(&swatch);

    let components = std::array::from_fn(|component| {
        let entry = gtk::Entry::builder()
            .placeholder_text(COMPONENT_PLACEHOLDERS.get(component).copied().unwrap_or(""))
            .width_chars(6)
            .max_width_chars(6)
            .build();
        let handler = {
            let sender = sender.clone();
            entry.connect_changed(move |entry| {
                sender.input(to_component(index, component, entry.text().to_string()));
            })
        };
        controls.append(&entry);
        ComponentEntry { entry, handler }
    });

    let content = row_content_box(gtk::Orientation::Vertical);
    content.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["dim-label", "caption"])
            .build(),
    );
    content.append(&controls);

    ColorRow {
        row: plain_row(&content),
        hex,
        hex_handler,
        swatch,
        swatch_handler,
        components,
    }
}
