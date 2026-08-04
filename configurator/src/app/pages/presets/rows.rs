use super::*;

/// Row builder for one slot.
///
/// Mirrors the [`PageBuilder`] row helpers, but adds rows to the slot's
/// expander and reads the slot out of the model, so a slot the draft does
/// not hold simply leaves its rows alone.
pub(super) struct SlotBuilder<'a> {
    pub(super) page: &'a mut PageBuilder,
    pub(super) expander: adw::ExpanderRow,
    pub(super) slot: usize,
}

impl SlotBuilder<'_> {
    /// A free-text row sending `to_message(slot, text)` on change.
    pub(super) fn entry_row(
        &mut self,
        title: &str,
        get: impl Fn(&ConfiguratorApp) -> Option<String> + 'static,
        to_message: impl Fn(usize, String) -> Message + 'static,
    ) -> adw::EntryRow {
        self.entry_row_validated(title, get, to_message, |_app| None)
    }

    /// A free-text row with live validation: a non-`None` result marks the
    /// row `.error` and shows the text as its tooltip.
    pub(super) fn entry_row_validated(
        &mut self,
        title: &str,
        get: impl Fn(&ConfiguratorApp) -> Option<String> + 'static,
        to_message: impl Fn(usize, String) -> Message + 'static,
        validate: impl Fn(&ConfiguratorApp) -> Option<String> + 'static,
    ) -> adw::EntryRow {
        let row = adw::EntryRow::builder().title(title).build();
        let slot = self.slot;
        let handler = {
            let sender = self.page.sender();
            row.connect_changed(move |row| {
                sender.input(to_message(slot, row.text().to_string()));
            })
        };
        self.expander.add_row(&row);
        {
            let row = row.clone();
            self.page.bind(move |app, _search| {
                if let Some(value) = get(app) {
                    // Blocked: the draft owns this text, and a load reporting
                    // its own value back as a user edit clears that load's
                    // diagnostics from the status line.
                    set_text_blocked(&row, &handler, &value);
                }
                set_row_error(&row, validate(app));
            });
        }
        row
    }

    /// A single-choice row sending `to_message(slot, value)` on selection.
    pub(super) fn combo_row<O>(
        &mut self,
        title: &str,
        values: Vec<O>,
        labels: Vec<String>,
        get: impl Fn(&ConfiguratorApp) -> Option<O> + 'static,
        to_message: impl Fn(usize, O) -> Message + 'static,
    ) -> adw::ComboRow
    where
        O: Copy + PartialEq + 'static,
    {
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let row = adw::ComboRow::builder()
            .title(title)
            .model(&gtk::StringList::new(&label_refs))
            .build();
        let slot = self.slot;
        let handler: SignalHandlerId = {
            let sender = self.page.sender();
            let values = values.clone();
            row.connect_selected_notify(move |row| {
                if let Some(value) = values.get(row.selected() as usize) {
                    sender.input(to_message(slot, *value));
                }
            })
        };
        self.expander.add_row(&row);
        {
            let row = row.clone();
            self.page.bind(move |app, _search| {
                let Some(current) = get(app) else {
                    return;
                };
                if let Some(index) = values.iter().position(|value| *value == current) {
                    // Blocked: the draft chose this, and reporting it back as
                    // a user pick clears the status line a load just wrote.
                    set_selected_blocked(&row, &handler, index as u32);
                }
            });
        }
        row
    }

    /// A Default/On/Off row for one of the slot's override fields.
    pub(super) fn override_row(
        &mut self,
        title: &str,
        field: PresetToggleField,
        get: impl Fn(&ConfiguratorApp) -> Option<OverrideOption> + 'static,
    ) -> adw::ComboRow {
        let values = OverrideOption::list();
        let labels = labels_of(&values, OverrideOption::label);
        self.combo_row(title, values, labels, get, move |slot, value| {
            Message::PresetToggleOptionChanged(slot, field, value)
        })
    }

    /// Binds a row's visibility to a model condition.
    pub(super) fn visible_when(
        &mut self,
        row: &impl IsA<gtk::Widget>,
        visible: impl Fn(&ConfiguratorApp) -> bool + 'static,
    ) {
        let row = row.clone().upcast::<gtk::Widget>();
        self.page.bind(move |app, _search| {
            let value = visible(app);
            if row.is_visible() != value {
                row.set_visible(value);
            }
        });
    }
}

/// A flat header button for one of the slot's actions.
pub(super) fn slot_button(
    page: &PageBuilder,
    label: &str,
    tooltip: &str,
    message: Message,
) -> gtk::Button {
    let button = gtk::Button::builder()
        .label(label)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    let sender = page.sender();
    button.connect_clicked(move |_| sender.input(message.clone()));
    button
}

/// A read-only preview of the slot's resolved color.
///
/// Blank until its binding installs a draw function carrying the color, so
/// the widget holds no state the binding has to encode and decode.
pub(super) fn color_swatch() -> gtk::DrawingArea {
    gtk::DrawingArea::builder()
        .content_width(24)
        .content_height(24)
        .valign(gtk::Align::Center)
        .css_classes(["card"])
        .build()
}

fn set_row_error(row: &adw::EntryRow, error: Option<String>) {
    let has_error_class = row.has_css_class("error");
    match error {
        Some(message) => {
            if !has_error_class {
                row.add_css_class("error");
            }
            if row.tooltip_text().as_deref() != Some(message.as_str()) {
                row.set_tooltip_text(Some(&message));
            }
        }
        None => {
            if has_error_class {
                row.remove_css_class("error");
                row.set_tooltip_text(None);
            }
        }
    }
}

pub(super) fn labels_of<O>(values: &[O], label: impl Fn(&O) -> &'static str) -> Vec<String> {
    values
        .iter()
        .map(|value| label(value).to_string())
        .collect()
}
