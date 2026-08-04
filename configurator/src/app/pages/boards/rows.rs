use super::*;

/// An entry row whose text the model owns, kept with the handler a refresh
/// has to block before writing it.
pub(super) struct TextRow {
    pub(super) row: adw::EntryRow,
    pub(super) handler: SignalHandlerId,
}

pub(super) fn build_text_row(
    title: &str,
    index: usize,
    field: BoardItemTextField,
    sender: &ComponentSender<ConfiguratorApp>,
) -> TextRow {
    let row = adw::EntryRow::builder().title(title).build();
    let handler = {
        let sender = sender.clone();
        row.connect_changed(move |row| {
            sender.input(Message::BoardsItemTextChanged(
                index,
                field,
                row.text().to_string(),
            ));
        })
    };
    TextRow { row, handler }
}

pub(super) fn build_kind_row(
    index: usize,
    selected: BoardBackgroundOption,
    sender: &ComponentSender<ConfiguratorApp>,
) -> adw::ComboRow {
    let options = BoardBackgroundOption::list();
    let labels: Vec<String> = options
        .iter()
        .map(|option| option.label().to_string())
        .collect();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let row = adw::ComboRow::builder()
        .title("Background")
        .model(&gtk::StringList::new(&label_refs))
        .build();

    // The selection is part of the rebuild fingerprint, so it is set before
    // the handler exists and never written again behind the user's back.
    if let Some(position) = options.iter().position(|option| *option == selected) {
        row.set_selected(position as u32);
    }
    {
        let sender = sender.clone();
        row.connect_selected_notify(move |row| {
            if let Some(option) = options.get(row.selected() as usize) {
                sender.input(Message::BoardsBackgroundKindChanged(index, *option));
            }
        });
    }
    row
}

pub(super) fn build_toggle_row(
    title: &str,
    index: usize,
    field: BoardItemToggleField,
    active: bool,
    expanded: bool,
    sender: &ComponentSender<ConfiguratorApp>,
) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(title)
        .active(active)
        .visible(expanded)
        .build();
    let sender = sender.clone();
    row.connect_active_notify(move |row| {
        sender.input(Message::BoardsItemToggleChanged(
            index,
            field,
            row.is_active(),
        ));
    });
    row
}

pub(super) fn connect_button(
    button: &gtk::Button,
    message: Message,
    sender: &ComponentSender<ConfiguratorApp>,
) {
    let sender = sender.clone();
    button.connect_clicked(move |_| sender.input(message.clone()));
}

pub(super) fn set_label(label: &gtk::Label, value: &str) {
    if label.text() != value {
        label.set_text(value);
    }
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

pub(super) fn row_content_box(orientation: gtk::Orientation) -> gtk::Box {
    gtk::Box::builder()
        .orientation(orientation)
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

pub(super) fn note_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label", "caption"])
        .build()
}

pub(super) fn picker_hex(app: &ConfiguratorApp, id: ColorPickerId) -> &str {
    app.color_picker_hex.get(&id).map_or("", String::as_str)
}

pub(super) fn is_collapsed(app: &ConfiguratorApp, index: usize) -> bool {
    app.boards_collapsed.get(index).copied().unwrap_or(false)
}

/// Error text for a count field with a lower bound, worded as the Iced view
/// worded it.
pub(super) fn validate_min(value: &str, min: usize) -> Option<String> {
    match value.trim().parse::<usize>() {
        Ok(parsed) if parsed >= min => None,
        Ok(_) => Some(format!("Minimum: {min}")),
        Err(_) => Some("Expected a whole number".to_string()),
    }
}
