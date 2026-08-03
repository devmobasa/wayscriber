//! Presets page: the tool preset slots the toolbar offers.
//!
//! `PRESET_SLOTS_MAX` is a compile-time constant, so every slot the build
//! supports exists as one `AdwExpanderRow` from startup and the configured
//! count only decides which of them are visible — no row is ever added or
//! removed while the app runs.
//!
//! Each expander carries the whole slot: its enable switch is the slot's
//! `enabled` flag, its expansion mirrors `preset_collapsed`, and every row
//! inside sends the same indexed [`Message`] the Iced slot view sent.

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;
use wayscriber::config::{PRESET_SLOTS_MAX, PRESET_SLOTS_MIN};

use gtk::glib::SignalHandlerId;

use crate::messages::Message;
use crate::models::{
    ColorMode, NamedColorOption, OverrideOption, PresetEraserKindOption, PresetEraserModeOption,
    PresetTextField, PresetToggleField, TabId, ToolOption,
};

use super::super::search::{AppSearchSummary, SearchArea};
use super::super::state::ConfiguratorApp;
use super::{BuiltPage, PageBuilder, set_selected_blocked, set_text_blocked, validate_u32_range};

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::Presets);

    let counts: Vec<usize> = (PRESET_SLOTS_MIN..=PRESET_SLOTS_MAX).collect();
    let count_labels: Vec<String> = counts.iter().map(usize::to_string).collect();
    page.group_in_area("Preset Slots", SearchArea::PresetControls)
        .combo_row(
            "Visible slots",
            "How many preset slots the toolbar offers.",
            counts,
            count_labels,
            |app| app.draft.presets.slot_count,
            Message::PresetSlotCountChanged,
        );

    page.group("Slots").group_visible_when(|app| {
        let search = app.search_summary();
        (1..=PRESET_SLOTS_MAX).any(|slot| slot_visible(app, &search, slot))
    });
    for slot in 1..=PRESET_SLOTS_MAX {
        build_slot(&mut page, slot);
    }

    page.finish()
}

/// Whether one slot shows: within the configured count when the page is
/// unfiltered, and otherwise only when the search matched that slot.
fn slot_visible(app: &ConfiguratorApp, search: &AppSearchSummary, slot: usize) -> bool {
    let within_count = slot
        <= app
            .draft
            .presets
            .slot_count
            .clamp(PRESET_SLOTS_MIN, PRESET_SLOTS_MAX);
    if !search.is_active() {
        return within_count;
    }
    match search.tab(TabId::Presets) {
        Some(summary) if summary.show_all() => within_count,
        Some(summary) => summary.preset_slots().contains(&slot),
        None => false,
    }
}

fn build_slot(page: &mut PageBuilder, slot: usize) {
    let expander = adw::ExpanderRow::builder()
        .title(format!("Slot {slot} settings"))
        .show_enable_switch(true)
        .build();

    let enable_handler = {
        let sender = page.sender();
        expander.connect_enable_expansion_notify(move |row| {
            sender.input(Message::PresetSlotEnabledChanged(
                slot,
                row.enables_expansion(),
            ));
        })
    };
    let expanded_handler = {
        let sender = page.sender();
        expander.connect_expanded_notify(move |row| {
            // Switching a slot off collapses its row on the way; only a
            // click on an enabled expander is the user toggling collapse.
            if row.enables_expansion() {
                sender.input(Message::PresetCollapseToggled(slot));
            }
        })
    };

    let reset = slot_button(
        page,
        "Reset",
        "Restore this slot's default settings",
        Message::PresetResetSlot(slot),
    );
    expander.add_suffix(&reset);
    let duplicate = slot_button(
        page,
        "Duplicate",
        "Copy this slot into the next one",
        Message::PresetDuplicateSlot(slot),
    );
    // The last slot has nowhere to copy to, exactly as the Iced header's
    // Duplicate button was left unpressable there.
    duplicate.set_sensitive(slot < PRESET_SLOTS_MAX);
    expander.add_suffix(&duplicate);

    page.custom(&expander);
    {
        let expander = expander.clone();
        page.bind(move |app, search| {
            let visible = slot_visible(app, search, slot);
            if expander.is_visible() != visible {
                expander.set_visible(visible);
            }
            let Some(draft) = app.draft.presets.slot(slot) else {
                return;
            };

            let collapsed = app
                .preset_collapsed
                .get(slot - 1)
                .copied()
                .unwrap_or_default();
            let expanded = draft.enabled && !collapsed;
            // Both handlers blocked around both writes: `PresetCollapseToggled`
            // flips rather than sets, so an expansion the model asked for that
            // echoed back as a toggle would undo itself, and enabling a slot
            // expands it as a side effect. With the writes silent, only a click
            // is ever reported.
            if expander.enables_expansion() != draft.enabled {
                expander.block_signal(&enable_handler);
                expander.block_signal(&expanded_handler);
                expander.set_enable_expansion(draft.enabled);
                expander.unblock_signal(&expanded_handler);
                expander.unblock_signal(&enable_handler);
            }
            if expander.is_expanded() != expanded {
                expander.block_signal(&expanded_handler);
                expander.set_expanded(expanded);
                expander.unblock_signal(&expanded_handler);
            }

            let subtitle = if !draft.enabled {
                "Slot disabled. Enable to configure.".to_string()
            } else if draft.name.trim().is_empty() {
                draft.tool.label().to_string()
            } else {
                format!("{} — {}", draft.name.trim(), draft.tool.label())
            };
            if expander.subtitle().as_str() != subtitle {
                expander.set_subtitle(&subtitle);
            }
        });
    }

    let mut rows = SlotBuilder {
        page,
        expander,
        slot,
    };

    rows.entry_row(
        "Label",
        move |app| app.draft.presets.slot(slot).map(|draft| draft.name.clone()),
        move |slot, value| Message::PresetTextChanged(slot, PresetTextField::Name, value),
    );

    let tools = ToolOption::list();
    let tool_labels = labels_of(&tools, ToolOption::label);
    rows.combo_row(
        "Tool",
        tools,
        tool_labels,
        move |app| app.draft.presets.slot(slot).map(|draft| draft.tool),
        Message::PresetToolChanged,
    );

    build_color_rows(&mut rows, slot);

    rows.entry_row(
        "Size (px)",
        move |app| app.draft.presets.slot(slot).map(|draft| draft.size.clone()),
        move |slot, value| Message::PresetTextChanged(slot, PresetTextField::Size, value),
    );
    rows.entry_row(
        "Marker opacity (0.05-0.9)",
        move |app| {
            app.draft
                .presets
                .slot(slot)
                .map(|draft| draft.marker_opacity.clone())
        },
        move |slot, value| Message::PresetTextChanged(slot, PresetTextField::MarkerOpacity, value),
    );

    let kinds = PresetEraserKindOption::list();
    let kind_labels = labels_of(&kinds, PresetEraserKindOption::label);
    rows.combo_row(
        "Eraser kind",
        kinds,
        kind_labels,
        move |app| app.draft.presets.slot(slot).map(|draft| draft.eraser_kind),
        Message::PresetEraserKindChanged,
    );
    let modes = PresetEraserModeOption::list();
    let mode_labels = labels_of(&modes, PresetEraserModeOption::label);
    rows.combo_row(
        "Eraser mode",
        modes,
        mode_labels,
        move |app| app.draft.presets.slot(slot).map(|draft| draft.eraser_mode),
        Message::PresetEraserModeChanged,
    );

    rows.override_row("Fill enabled", PresetToggleField::FillEnabled, move |app| {
        app.draft.presets.slot(slot).map(|draft| draft.fill_enabled)
    });
    rows.override_row(
        "Text background",
        PresetToggleField::TextBackgroundEnabled,
        move |app| {
            app.draft
                .presets
                .slot(slot)
                .map(|draft| draft.text_background_enabled)
        },
    );

    rows.entry_row(
        "Font size (pt)",
        move |app| {
            app.draft
                .presets
                .slot(slot)
                .map(|draft| draft.font_size.clone())
        },
        move |slot, value| Message::PresetTextChanged(slot, PresetTextField::FontSize, value),
    );
    rows.entry_row(
        "Arrow length (px)",
        move |app| {
            app.draft
                .presets
                .slot(slot)
                .map(|draft| draft.arrow_length.clone())
        },
        move |slot, value| Message::PresetTextChanged(slot, PresetTextField::ArrowLength, value),
    );
    rows.entry_row(
        "Arrow angle (deg)",
        move |app| {
            app.draft
                .presets
                .slot(slot)
                .map(|draft| draft.arrow_angle.clone())
        },
        move |slot, value| Message::PresetTextChanged(slot, PresetTextField::ArrowAngle, value),
    );
    rows.override_row(
        "Arrow head at end",
        PresetToggleField::ArrowHeadAtEnd,
        move |app| {
            app.draft
                .presets
                .slot(slot)
                .map(|draft| draft.arrow_head_at_end)
        },
    );
    rows.override_row(
        "Show status bar",
        PresetToggleField::ShowStatusBar,
        move |app| {
            app.draft
                .presets
                .slot(slot)
                .map(|draft| draft.show_status_bar)
        },
    );
}

/// The slot's color block: a mode chooser with a live preview, then the
/// rows the chosen mode edits.
fn build_color_rows(rows: &mut SlotBuilder<'_>, slot: usize) {
    let mode_row = rows.combo_row(
        "Color mode",
        vec![ColorMode::Named, ColorMode::Rgb],
        vec!["Named".to_string(), "RGB".to_string()],
        move |app| app.draft.presets.slot(slot).map(|draft| draft.color.mode),
        Message::PresetColorModeChanged,
    );
    let swatch = color_swatch();
    mode_row.add_suffix(&swatch);
    // The color the swatch is currently drawing, owned by the binding that
    // draws it: a new draw function is what redraws the widget, so it is only
    // installed when the color actually moved.
    let mut shown: Option<[f64; 3]> = None;
    rows.page.bind(move |app, _search| {
        let color = app
            .draft
            .presets
            .slot(slot)
            .and_then(|draft| draft.color.preview_color())
            .map(|color| [color.r, color.g, color.b]);
        if shown == color {
            return;
        }
        shown = color;
        match color {
            Some(rgb) => swatch.set_draw_func(move |_, context, width, height| {
                context.set_source_rgb(rgb[0], rgb[1], rgb[2]);
                context.rectangle(0.0, 0.0, f64::from(width), f64::from(height));
                // A failed fill only leaves the swatch blank.
                let _ = context.fill();
            }),
            // A slot whose color resolves to nothing draws nothing.
            None => swatch.set_draw_func(|_, _, _, _| {}),
        }
    });

    let named = NamedColorOption::list();
    let named_labels = labels_of(&named, NamedColorOption::label);
    let named_row = rows.combo_row(
        "Named color",
        named,
        named_labels,
        move |app| {
            app.draft
                .presets
                .slot(slot)
                .map(|draft| draft.color.selected_named)
        },
        Message::PresetNamedColorSelected,
    );
    rows.visible_when(&named_row, move |app| {
        app.draft
            .presets
            .slot(slot)
            .is_some_and(|draft| draft.color.mode == ColorMode::Named)
    });

    let custom_name = rows.entry_row_validated(
        "Custom color name",
        move |app| {
            app.draft
                .presets
                .slot(slot)
                .map(|draft| draft.color.name.clone())
        },
        move |slot, value| Message::PresetTextChanged(slot, PresetTextField::ColorName, value),
        move |app| {
            let color = &app.draft.presets.slot(slot)?.color;
            if color.mode != ColorMode::Named {
                return None;
            }
            (color.preview_color().is_none() && !color.name.trim().is_empty())
                .then(|| "Unknown color name.".to_string())
        },
    );
    rows.visible_when(&custom_name, move |app| {
        app.draft.presets.slot(slot).is_some_and(|draft| {
            draft.color.mode == ColorMode::Named && draft.color.selected_named_is_custom()
        })
    });

    for (component, title) in [
        (0usize, "Red (0-255)"),
        (1, "Green (0-255)"),
        (2, "Blue (0-255)"),
    ] {
        let row = rows.entry_row_validated(
            title,
            move |app| {
                app.draft
                    .presets
                    .slot(slot)
                    .and_then(|draft| draft.color.rgb.get(component).cloned())
            },
            move |slot, value| Message::PresetColorComponentChanged(slot, component, value),
            move |app| {
                let color = &app.draft.presets.slot(slot)?.color;
                if color.mode != ColorMode::Rgb {
                    return None;
                }
                validate_u32_range(color.rgb.get(component)?, 0, 255)
            },
        );
        rows.visible_when(&row, move |app| {
            app.draft
                .presets
                .slot(slot)
                .is_some_and(|draft| draft.color.mode == ColorMode::Rgb)
        });
    }
}

/// Row builder for one slot.
///
/// Mirrors the [`PageBuilder`] row helpers, but adds rows to the slot's
/// expander and reads the slot out of the model, so a slot the draft does
/// not hold simply leaves its rows alone.
struct SlotBuilder<'a> {
    page: &'a mut PageBuilder,
    expander: adw::ExpanderRow,
    slot: usize,
}

impl SlotBuilder<'_> {
    /// A free-text row sending `to_message(slot, text)` on change.
    fn entry_row(
        &mut self,
        title: &str,
        get: impl Fn(&ConfiguratorApp) -> Option<String> + 'static,
        to_message: impl Fn(usize, String) -> Message + 'static,
    ) -> adw::EntryRow {
        self.entry_row_validated(title, get, to_message, |_app| None)
    }

    /// A free-text row with live validation: a non-`None` result marks the
    /// row `.error` and shows the text as its tooltip.
    fn entry_row_validated(
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
    fn combo_row<O>(
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
    fn override_row(
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
    fn visible_when(
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
fn slot_button(page: &PageBuilder, label: &str, tooltip: &str, message: Message) -> gtk::Button {
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
fn color_swatch() -> gtk::DrawingArea {
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

fn labels_of<O>(values: &[O], label: impl Fn(&O) -> &'static str) -> Vec<String> {
    values
        .iter()
        .map(|value| label(value).to_string())
        .collect()
}
