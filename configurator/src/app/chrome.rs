//! Chrome: the controls whose widget depends on the libadwaita API floor.
//!
//! A page says what a control *does* — which model value it shows and which
//! [`Message`] a user choice sends. When the widget that says it best is
//! newer than the baseline floor, the page asks chrome for the control
//! instead of building it inline, and chrome hands back a row plus the
//! binding that refreshes it. That is what keeps page bodies free of feature
//! `cfg`s: the branch lives here, once, for every page that needs it.
//!
//! # The twin contract
//!
//! Each constructor exists twice, as `cfg` blocks *inside this file*:
//! `cfg(not(feature = "adw-modern"))` builds what the `v1_4` floor
//! guarantees, `cfg(feature = "adw-modern")` builds the newer control. Three
//! rules hold the halves to the same behavior.
//!
//! - **Message parity.** A user choice sends the same message with the same
//!   payload in either channel, so `messages.rs`, `update/`, `state.rs`, and
//!   every model test stay feature-independent. Only the widget differs.
//! - **Blocked programmatic writes.** A refresh writes the widget with the
//!   handler that reports user choices blocked, and only when the value
//!   actually changed. Without both guards a refresh arrives at the update
//!   layer as a user pick: it clears the status line the user is reading and
//!   marks a draft nobody edited dirty, and the resulting write-notify-write
//!   loop can ping-pong. Each twin owns that write, because the widgets do
//!   not share a setter — the combo moves by `set_selected`, the toggle
//!   group by `set_active` — so the `AdwComboRow`-typed helper in `pages` is
//!   not force-fit onto the toggle group.
//! - **One always-compiled file.** The twins are `cfg` blocks in this file,
//!   never a `cfg`-gated module file, and no Rust source anywhere may be
//!   reachable only under `adw-modern`. The source-coverage matrix has no
//!   modern lane — Ubuntu cannot compile one — so a modern-only file would
//!   be invisible to every lane that walks the tree, and the checker would
//!   report it as unreachable source.
//!
//! When the baseline floor reaches the modern floor, delete every
//! `cfg(not(feature = "adw-modern"))` half and collapse what is left.

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;

use crate::messages::Message;

use super::pages::Binding;
use super::state::ConfiguratorApp;

#[cfg(not(feature = "adw-modern"))]
use super::pages::set_selected_blocked;

#[cfg(feature = "adw-modern")]
use gtk::glib::SignalHandlerId;

// ---------------------------------------------------------------------------
// Mode toggle
// ---------------------------------------------------------------------------

/// A single-choice row over a small fixed option set — baseline twin.
///
/// An `AdwComboRow` over `values`, labeled by `labels`, sending
/// `to_message(value)` on selection: the control every plain `combo_row`
/// builds, and the one the `v1_4` floor guarantees.
///
/// Returns the row and its refresh binding. The row is returned rather than
/// added here because membership in the caller's group is what gives it the
/// group's search visibility; the binding comes with it because only this
/// twin knows which write its widget has to block.
#[cfg(not(feature = "adw-modern"))]
pub(crate) fn mode_toggle<O>(
    sender: ComponentSender<ConfiguratorApp>,
    title: &str,
    subtitle: &str,
    values: Vec<O>,
    labels: Vec<String>,
    get: impl Fn(&ConfiguratorApp) -> O + 'static,
    to_message: impl Fn(O) -> Message + 'static,
) -> (adw::PreferencesRow, Binding)
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
        let values = values.clone();
        row.connect_selected_notify(move |row| {
            if let Some(value) = values.get(row.selected() as usize) {
                sender.input(to_message(*value));
            }
        })
    };

    let refreshed = row.clone();
    let binding: Binding = Box::new(move |app, _summary| {
        let current = get(app);
        if let Some(index) = values.iter().position(|value| *value == current) {
            set_selected_blocked(&refreshed, &handler, index as u32);
        }
    });

    (row.upcast(), binding)
}

/// A single-choice row over a small fixed option set — modern twin.
///
/// An `AdwToggleGroup` over `values`, labeled by `labels`, sending
/// `to_message(value)` on selection: every option is visible at once, which
/// is what makes it worth having for a two- or three-way mode.
///
/// The group is not an `AdwPreferencesRow`, so it cannot sit in a
/// preferences group by itself. It rides as the suffix of an `AdwActionRow`
/// carrying the same title and subtitle, which keeps the control inside the
/// group's boxed list with its label where the combo put it.
///
/// Returns the row and its refresh binding, on the same terms as the
/// baseline twin.
#[cfg(feature = "adw-modern")]
pub(crate) fn mode_toggle<O>(
    sender: ComponentSender<ConfiguratorApp>,
    title: &str,
    subtitle: &str,
    values: Vec<O>,
    labels: Vec<String>,
    get: impl Fn(&ConfiguratorApp) -> O + 'static,
    to_message: impl Fn(O) -> Message + 'static,
) -> (adw::PreferencesRow, Binding)
where
    O: Copy + PartialEq + 'static,
{
    let toggles = adw::ToggleGroup::builder()
        .valign(gtk::Align::Center)
        .build();
    for label in &labels {
        toggles.add(adw::Toggle::builder().label(label.as_str()).build());
    }

    let row = adw::ActionRow::builder().title(title).build();
    if !subtitle.is_empty() {
        row.set_subtitle(subtitle);
    }
    row.add_suffix(&toggles);

    let handler = {
        let values = values.clone();
        toggles.connect_active_notify(move |toggles| {
            if let Some(value) = values.get(toggles.active() as usize) {
                sender.input(to_message(*value));
            }
        })
    };

    let binding: Binding = Box::new(move |app, _summary| {
        let current = get(app);
        if let Some(index) = values.iter().position(|value| *value == current) {
            set_active_blocked(&toggles, &handler, index as u32);
        }
    });

    (row.upcast(), binding)
}

/// Moves a toggle group to the option the model chose without reporting it
/// as a pick — the toggle group's half of the blocked-write rule.
///
/// `active()` also answers with an out-of-range position while no toggle is
/// selected, so the equality check both skips redundant writes and lets the
/// first refresh seed the group.
#[cfg(feature = "adw-modern")]
fn set_active_blocked(toggles: &adw::ToggleGroup, handler: &SignalHandlerId, index: u32) {
    if toggles.active() == index {
        return;
    }
    toggles.block_signal(handler);
    toggles.set_active(index);
    toggles.unblock_signal(handler);
}
