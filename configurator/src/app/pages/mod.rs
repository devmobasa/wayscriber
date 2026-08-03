//! Page construction: one module per sidebar tab.
//!
//! Every page is built once at startup and refreshed through the bindings
//! it registers: a binding reads the model (and the current search summary)
//! and writes the widget only when the value differs, which both avoids
//! redundant layout work and breaks the signal echo a programmatic set
//! would otherwise cause. User edits never write the model directly — each
//! row's signal sends the same [`Message`] the Iced view sent, so the
//! update layer is unchanged.

mod arrow;
mod boards;
mod capture;
pub(crate) mod color_rows;
mod daemon;
mod drawing;
mod history;
mod keybindings;
mod performance;
mod presets;
mod render_profiles;
mod session;
#[cfg(feature = "tablet-input")]
mod tablet;
mod ui;

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;
use gtk::glib::SignalHandlerId;

use crate::messages::Message;
use crate::models::TabId;

use super::search::{AppSearchSummary, SearchArea};
use super::state::ConfiguratorApp;

/// One view refresh: reads the model, writes a widget when it differs.
///
/// `FnMut`, not `Fn`, because `update_view` holds `&mut AppWidgets` and a
/// binding is therefore free to own state that has to survive between
/// refreshes. That is what lets a page whose rows are a user-sized list keep
/// the layout it last built and the typed refresh closure of every row it
/// built beside it: the two are replaced together, so a row and the values it
/// is handed cannot drift apart.
pub(crate) type Binding = Box<dyn FnMut(&ConfiguratorApp, &AppSearchSummary)>;

/// Writes editable text the model owns without reporting it as a user edit.
///
/// Every refresh that writes a control the user can also drive goes through a
/// helper like this one: without the block a programmatic write reports itself
/// as a user edit, which re-runs whatever the setter derives and clears the
/// status line the user is reading.
pub(crate) fn set_text_blocked(
    editable: &impl IsA<gtk::Editable>,
    handler: &SignalHandlerId,
    value: &str,
) {
    if editable.text() == value {
        return;
    }
    editable.block_signal(handler);
    editable.set_text(value);
    editable.unblock_signal(handler);
}

/// Moves a combo to the entry the model chose without reporting it as a pick.
pub(crate) fn set_selected_blocked(row: &adw::ComboRow, handler: &SignalHandlerId, index: u32) {
    if row.selected() == index {
        return;
    }
    row.block_signal(handler);
    row.set_selected(index);
    row.unblock_signal(handler);
}

pub(crate) struct BuiltPage {
    pub(crate) widget: gtk::Widget,
    pub(crate) bindings: Vec<Binding>,
}

/// Stable `GtkStack` child name for a tab (deep links and view sync key off
/// the model's `active_tab`, never off these strings).
pub(crate) fn stack_name(tab: TabId) -> &'static str {
    tab.title()
}

/// Builds every page in sidebar order.
pub(crate) fn build_all(sender: &ComponentSender<ConfiguratorApp>) -> Vec<(TabId, BuiltPage)> {
    TabId::ALL
        .into_iter()
        .map(|tab| {
            let page = match tab {
                TabId::Daemon => daemon::build(sender),
                TabId::Drawing => drawing::build(sender),
                TabId::Presets => presets::build(sender),
                TabId::Ui => ui::build(sender),
                TabId::Boards => boards::build(sender),
                TabId::RenderProfiles => render_profiles::build(sender),
                TabId::Performance => performance::build(sender),
                TabId::History => history::build(sender),
                TabId::Capture => capture::build(sender),
                TabId::Session => session::build(sender),
                TabId::Keybindings => keybindings::build(sender),
                TabId::Arrow => arrow::build(sender),
                #[cfg(feature = "tablet-input")]
                TabId::Tablet => tablet::build(sender),
            };
            (tab, page)
        })
        .collect()
}

/// Error text for a whole-number field constrained to `min..=max`, `None`
/// while the input is acceptable.
pub(crate) fn validate_u32_range(value: &str, min: u32, max: u32) -> Option<String> {
    match value.trim().parse::<u32>() {
        Ok(parsed) if (min..=max).contains(&parsed) => None,
        _ => Some(format!("Enter a whole number between {min} and {max}.")),
    }
}

/// Incremental builder for a preferences-style page: groups of Adwaita rows
/// wired to messages, with per-group search visibility.
pub(crate) struct PageBuilder {
    sender: ComponentSender<ConfiguratorApp>,
    tab: TabId,
    page: adw::PreferencesPage,
    group: adw::PreferencesGroup,
    bindings: Vec<Binding>,
}

impl PageBuilder {
    pub(crate) fn new(sender: &ComponentSender<ConfiguratorApp>, tab: TabId) -> Self {
        let page = adw::PreferencesPage::new();
        let group = adw::PreferencesGroup::new();
        page.add(&group);
        Self {
            sender: sender.clone(),
            tab,
            page,
            group,
            bindings: Vec::new(),
        }
    }

    /// Starts a new titled group of rows.
    pub(crate) fn group(&mut self, title: &str) -> &mut Self {
        let group = adw::PreferencesGroup::builder().title(title).build();
        self.page.add(&group);
        self.group = group;
        self
    }

    /// Starts a new titled group whose visibility follows a search area:
    /// with an active search the group shows only when the area matched,
    /// mirroring the Iced views' section filtering.
    pub(crate) fn group_in_area(&mut self, title: &str, area: SearchArea) -> &mut Self {
        self.group(title);
        let group = self.group.clone();
        let tab = self.tab;
        self.bindings.push(Box::new(move |_app, summary| {
            let visible = !summary.is_active()
                || summary
                    .tab(tab)
                    .is_some_and(|tab_summary| tab_summary.area_matches(area));
            if group.is_visible() != visible {
                group.set_visible(visible);
            }
        }));
        self
    }

    /// Starts a new titled group visible only while its search area matches
    /// AND a model condition holds — one binding, so the two gates cannot
    /// overwrite each other the way stacked visibility bindings would.
    pub(crate) fn group_in_area_when(
        &mut self,
        title: &str,
        area: SearchArea,
        condition: impl Fn(&ConfiguratorApp) -> bool + 'static,
    ) -> &mut Self {
        self.group(title);
        let group = self.group.clone();
        let tab = self.tab;
        self.bindings.push(Box::new(move |app, summary| {
            let area_visible = !summary.is_active()
                || summary
                    .tab(tab)
                    .is_some_and(|tab_summary| tab_summary.area_matches(area));
            let visible = area_visible && condition(app);
            if group.is_visible() != visible {
                group.set_visible(visible);
            }
        }));
        self
    }

    /// Binds the current group's visibility to an arbitrary model condition
    /// (collapsed sections, feature-dependent rows).
    pub(crate) fn group_visible_when(
        &mut self,
        visible: impl Fn(&ConfiguratorApp) -> bool + 'static,
    ) -> &mut Self {
        let group = self.group.clone();
        self.bindings.push(Box::new(move |app, _summary| {
            let value = visible(app);
            if group.is_visible() != value {
                group.set_visible(value);
            }
        }));
        self
    }

    /// A boolean row: `AdwSwitchRow` sending `to_message(state)` on change.
    pub(crate) fn switch_row(
        &mut self,
        title: &str,
        subtitle: &str,
        get: impl Fn(&ConfiguratorApp) -> bool + 'static,
        to_message: impl Fn(bool) -> Message + 'static,
    ) -> &mut Self {
        let row = adw::SwitchRow::builder().title(title).build();
        if !subtitle.is_empty() {
            row.set_subtitle(subtitle);
        }
        let handler = {
            let sender = self.sender.clone();
            row.connect_active_notify(move |row| {
                sender.input(to_message(row.is_active()));
            })
        };
        self.group.add(&row);
        self.bindings.push(Box::new(move |app, _summary| {
            let value = get(app);
            if row.is_active() != value {
                // Blocked: a refresh that reported itself would be indistinct
                // from a user toggle, and not every toggle message is a plain
                // setter — the toolbar one pins an explicit visibility entry,
                // so an echo would dirty a config nobody edited.
                row.block_signal(&handler);
                row.set_active(value);
                row.unblock_signal(&handler);
            }
        }));
        self
    }

    /// A free-text row: `AdwEntryRow` sending `to_message(text)` on change.
    pub(crate) fn entry_row(
        &mut self,
        title: &str,
        get: impl Fn(&ConfiguratorApp) -> String + 'static,
        to_message: impl Fn(String) -> Message + 'static,
    ) -> &mut Self {
        self.entry_row_validated(title, get, to_message, |_| None)
    }

    /// A free-text row with live validation: a non-`None` result marks the
    /// row `.error` and shows the text as its tooltip.
    pub(crate) fn entry_row_validated(
        &mut self,
        title: &str,
        get: impl Fn(&ConfiguratorApp) -> String + 'static,
        to_message: impl Fn(String) -> Message + 'static,
        validate: impl Fn(&ConfiguratorApp) -> Option<String> + 'static,
    ) -> &mut Self {
        let row = adw::EntryRow::builder().title(title).build();
        let handler = {
            let sender = self.sender.clone();
            row.connect_changed(move |row| {
                sender.input(to_message(row.text().to_string()));
            })
        };
        self.group.add(&row);
        self.bindings.push(Box::new(move |app, _summary| {
            let value = get(app);
            if row.text() != value {
                // Blocked: the model owns this text, and reporting its own
                // value back as a user edit clears the status line the user
                // is reading and re-runs whatever the setter derives.
                row.block_signal(&handler);
                row.set_text(&value);
                row.unblock_signal(&handler);
            }
            let error = validate(app);
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
        }));
        self
    }

    /// A single-choice row: `AdwComboRow` over `values`, labeled by
    /// `labels`, sending `to_message(value)` on selection.
    pub(crate) fn combo_row<O>(
        &mut self,
        title: &str,
        subtitle: &str,
        values: Vec<O>,
        labels: Vec<String>,
        get: impl Fn(&ConfiguratorApp) -> O + 'static,
        to_message: impl Fn(O) -> Message + 'static,
    ) -> &mut Self
    where
        O: Copy + PartialEq + 'static,
    {
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let row = adw::ComboRow::builder()
            .title(title)
            .model(&gtk::StringList::new(&label_refs))
            .build();
        if !subtitle.is_empty() {
            row.set_subtitle(subtitle);
        }
        let handler = {
            let sender = self.sender.clone();
            let values = values.clone();
            row.connect_selected_notify(move |row| {
                if let Some(value) = values.get(row.selected() as usize) {
                    sender.input(to_message(*value));
                }
            })
        };
        self.group.add(&row);
        self.bindings.push(Box::new(move |app, _summary| {
            let current = get(app);
            if let Some(index) = values.iter().position(|value| *value == current) {
                let index = index as u32;
                if row.selected() != index {
                    // Blocked: the model chose this, so reporting it back as
                    // a user pick only clears the status line.
                    row.block_signal(&handler);
                    row.set_selected(index);
                    row.unblock_signal(&handler);
                }
            }
        }));
        self
    }

    /// Adds a fully custom row or widget to the current group.
    pub(crate) fn custom(&mut self, widget: &impl IsA<gtk::Widget>) -> &mut Self {
        self.group.add(widget);
        self
    }

    /// Registers an arbitrary refresh binding.
    ///
    /// `FnMut`, so a binding may own the state its section needs between
    /// refreshes — the dynamic lists keep their built rows this way.
    pub(crate) fn bind(
        &mut self,
        binding: impl FnMut(&ConfiguratorApp, &AppSearchSummary) + 'static,
    ) -> &mut Self {
        self.bindings.push(Box::new(binding));
        self
    }

    /// A clone of the sender for wiring custom widgets.
    pub(crate) fn sender(&self) -> ComponentSender<ConfiguratorApp> {
        self.sender.clone()
    }

    pub(crate) fn finish(self) -> BuiltPage {
        BuiltPage {
            widget: self.page.upcast(),
            bindings: self.bindings,
        }
    }
}
