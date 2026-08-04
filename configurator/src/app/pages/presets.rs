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

mod color;
mod rows;

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
use color::build_color_rows;
use rows::{SlotBuilder, labels_of, slot_button};

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
