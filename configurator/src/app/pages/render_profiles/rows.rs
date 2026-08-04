use super::*;

pub(super) fn build_header_row(
    index: usize,
    title: &gtk::Label,
    sender: &ComponentSender<ConfiguratorApp>,
) -> gtk::ListBoxRow {
    let content = row_content_box();
    content.append(title);

    let duplicate = gtk::Button::builder()
        .label("Duplicate")
        .valign(gtk::Align::Center)
        .build();
    connect_button(&duplicate, Message::RenderProfileDuplicate(index), sender);
    content.append(&duplicate);

    let remove = gtk::Button::builder()
        .label("Delete")
        .valign(gtk::Align::Center)
        .css_classes(["destructive-action"])
        .build();
    connect_button(&remove, Message::RenderProfileRemove(index), sender);
    content.append(&remove);

    plain_row(&content)
}

/// An entry row whose text the model owns, kept with the handler a refresh
/// has to block before writing it.
pub(super) struct TextRow {
    pub(super) row: adw::EntryRow,
    pub(super) handler: SignalHandlerId,
}

pub(super) fn build_text_row(
    title: &str,
    index: usize,
    field: RenderProfileTextField,
    sender: &ComponentSender<ConfiguratorApp>,
) -> TextRow {
    let row = adw::EntryRow::builder().title(title).build();
    let handler = {
        let sender = sender.clone();
        row.connect_changed(move |row| {
            sender.input(Message::RenderProfileTextChanged(
                index,
                field,
                row.text().to_string(),
            ));
        })
    };
    TextRow { row, handler }
}

pub(super) fn connect_button(
    button: &gtk::Button,
    message: Message,
    sender: &ComponentSender<ConfiguratorApp>,
) {
    let sender = sender.clone();
    button.connect_clicked(move |_| sender.input(message.clone()));
}

/// Rewrites a combo's model only when the choices themselves changed, and
/// writes both model and selection with the change handler blocked: replacing
/// a model resets the selection to the first row, which would otherwise be
/// reported as if the user had picked it.
///
/// `shown` is what the combo currently offers, owned by the binding that
/// calls this — the entries themselves, not a rendering of them.
pub(super) fn sync_combo(
    row: &adw::ComboRow,
    handler: &SignalHandlerId,
    shown: &mut Vec<String>,
    entries: &[String],
    selected: Option<usize>,
) {
    let rebuild = shown.as_slice() != entries;
    let target = selected.map_or(NO_SELECTION, |index| index as u32);
    if !rebuild && row.selected() == target {
        return;
    }

    row.block_signal(handler);
    if rebuild {
        let refs: Vec<&str> = entries.iter().map(String::as_str).collect();
        row.set_model(Some(&gtk::StringList::new(&refs)));
        shown.clear();
        shown.extend_from_slice(entries);
    }
    if row.selected() != target {
        row.set_selected(target);
    }
    row.unblock_signal(handler);
}

pub(super) fn selected_string(row: &adw::ComboRow) -> Option<String> {
    let item = row
        .selected_item()
        .and_then(|item| item.downcast::<gtk::StringObject>().ok())?;
    Some(item.string().to_string())
}

pub(super) fn row_content_box() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build()
}

pub(super) fn plain_row(content: &impl IsA<gtk::Widget>) -> gtk::ListBoxRow {
    gtk::ListBoxRow::builder()
        .child(content)
        .activatable(false)
        .selectable(false)
        .build()
}

pub(super) fn side_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .valign(gtk::Align::Center)
        .css_classes(["dim-label", "caption"])
        .build()
}

/// The picker's editing text, falling back to the stored mapping value the
/// way the Iced hex field did.
pub(super) fn picker_hex<'a>(
    app: &'a ConfiguratorApp,
    id: ColorPickerId,
    value: &'a str,
) -> &'a str {
    app.color_picker_hex.get(&id).map_or(value, String::as_str)
}
