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
//! A twin may render nothing. The confirmation controls below are that case:
//! the modern channel answers in a dialog, so its inline half builds no
//! widget at all rather than leaving a second way to answer the same
//! question behind the dialog. The record and its methods stay
//! always-compiled either way, so the caller reveals, presents, and closes
//! without asking which channel it is on. *Which* question is up is not
//! decided here at all — [`super::dialog`] owns that, channel-neutrally.
//!
//! When the baseline floor reaches the modern floor, delete every
//! `cfg(not(feature = "adw-modern"))` half and collapse what is left.

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;

use crate::messages::Message;

use super::dialog::{CANCEL_RESPONSE, CONFIRM_RESPONSE, Confirmation};
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

// ---------------------------------------------------------------------------
// Confirmations
// ---------------------------------------------------------------------------
//
// A destructive action asks before it acts, and the two channels ask
// differently: the baseline floor has no dialog worth using, so the answer is
// a pair of buttons beside the control that asked, while the modern floor has
// `AdwAlertDialog` and uses it. That makes a twin pair with an unusual shape —
// one channel's affordance is *nothing* — so the pair is written as two
// constructors over one always-compiled record, and the caller reveals,
// presents, and closes without ever asking which channel it is on.
//
// Which of the two is showing is not decided here: [`super::dialog`] owns
// that, channel-neutrally, and both constructors below are what its
// `Present`/`Close` outputs land on.

/// The inline Confirm/Cancel controls a channel answers with, if it has any.
///
/// Baseline: the pair of buttons, hidden until the question is up. Modern:
/// nothing at all — the dialog is the whole affordance, and a second set of
/// controls sitting behind it would be a second way to answer one question.
pub(crate) struct ConfirmationControls {
    /// `None` in the modern channel.
    row: Option<gtk::Widget>,
    /// The control keyboard focus lands on when the question comes up, kept
    /// beside the row because the row is a container and focus belongs on one
    /// button in it. `None` in the modern channel.
    confirm: Option<gtk::Button>,
}

impl ConfirmationControls {
    /// Adds the controls to the container the consumer answers in.
    ///
    /// Nothing is added in the modern channel, which is what keeps the
    /// caller's container free of an empty box it would have to reason about.
    pub(crate) fn attach(&self, parent: &gtk::Box) {
        if let Some(row) = &self.row {
            parent.append(row);
        }
    }

    /// Puts the controls on screen, or takes them off.
    ///
    /// This is the baseline channel's whole reading of `Present`/`Close`. In
    /// the modern channel it is a no-op: the dialog was already presented and
    /// closed by the twin below.
    pub(crate) fn set_presented(&self, presented: bool) {
        let Some(row) = &self.row else {
            return;
        };
        // The widget's own flag, never `is_visible`: a row inside a hidden
        // card reports invisible while its own flag still says otherwise, and
        // skipping the write there would leak the stale state the moment the
        // card comes back.
        if row.get_visible() != presented {
            row.set_visible(presented);
        }
    }

    /// Whether the question is currently up in this channel's own layout.
    ///
    /// Read before [`Self::set_presented`] writes, this is what tells a caller
    /// that has no transition in hand whether the next write reveals the
    /// controls or merely repeats what is already on screen. Always `false` in
    /// the modern channel, which puts nothing in the caller's layout.
    pub(crate) fn is_presented(&self) -> bool {
        self.row.as_ref().is_some_and(|row| row.get_visible())
    }

    /// Moves keyboard focus onto Confirm.
    ///
    /// Call it on the write that reveals the controls, and only there. Arming
    /// hides the control that asked, and hiding the focused widget leaves the
    /// window with no focus at all: the question would be on screen with no
    /// way to answer it from the keyboard, which is a regression against the
    /// relabel-in-place control this pair replaced. Focusing on every refresh
    /// instead would be its own bug — it would drag focus off Cancel while the
    /// user was reaching for it.
    ///
    /// Order matters: GTK refuses focus to a widget that is not on screen, so
    /// this runs after the reveal, never in the same breath as the decision to
    /// reveal. A refusal is not actionable here — there is no focus to restore
    /// and nothing to undo — so the answer is dropped.
    ///
    /// No-op in the modern channel: `AdwAlertDialog` is modal and focuses its
    /// own default response, and there is no inline control to focus anyway.
    pub(crate) fn focus_confirm(&self) {
        if let Some(confirm) = &self.confirm {
            confirm.grab_focus();
        }
    }

    /// Whether this channel answers in the caller's own layout.
    ///
    /// The control that asks steps aside only for controls that take its
    /// place. A dialog takes no space in the row it came from, so in the
    /// modern channel the control that asks stays exactly where it was and
    /// the caller reads `false` here to say so.
    pub(crate) fn is_inline(&self) -> bool {
        self.row.is_some()
    }
}

/// Inline Confirm/Cancel controls — baseline twin.
///
/// Built hidden, because the question is not up yet: the owner reveals them
/// when [`super::dialog::reconcile`] says to and hides them again on the
/// close row, so a rebuilt card never flashes an armed state it does not
/// have.
#[cfg(not(feature = "adw-modern"))]
pub(crate) fn confirmation_controls(
    sender: &ComponentSender<ConfiguratorApp>,
    confirmation: &Confirmation,
) -> ConfirmationControls {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .visible(false)
        .build();

    let confirm = response_button(
        sender,
        confirmation,
        CONFIRM_RESPONSE,
        confirmation.confirm_label(),
    );
    confirm.add_css_class("destructive-action");
    row.append(&confirm);

    let cancel = response_button(sender, confirmation, CANCEL_RESPONSE, "Cancel");
    cancel.add_css_class("flat");
    row.append(&cancel);

    ConfirmationControls {
        row: Some(row.upcast()),
        confirm: Some(confirm),
    }
}

/// One button that answers with a named response.
///
/// The response id is the same vocabulary the dialog uses, and the message it
/// sends comes from the same [`Confirmation::message_for`]: the two channels
/// differ in what the user presses, never in what the press means.
#[cfg(not(feature = "adw-modern"))]
fn response_button(
    sender: &ComponentSender<ConfiguratorApp>,
    confirmation: &Confirmation,
    response: &'static str,
    label: &str,
) -> gtk::Button {
    let button = gtk::Button::builder()
        .label(label)
        .valign(gtk::Align::Center)
        .build();
    let sender = sender.clone();
    let confirmation = confirmation.clone();
    button.connect_clicked(move |_| sender.input(confirmation.message_for(response)));
    button
}

/// Inline Confirm/Cancel controls — modern twin, which renders none.
#[cfg(feature = "adw-modern")]
pub(crate) fn confirmation_controls(
    _sender: &ComponentSender<ConfiguratorApp>,
    _confirmation: &Confirmation,
) -> ConfirmationControls {
    ConfirmationControls {
        row: None,
        confirm: None,
    }
}

/// The confirmation a channel currently has on screen — baseline twin.
///
/// There is nothing to hold: the inline controls belong to the card or the
/// header that built them, and revealing them is all presenting means here.
/// The record is the fact that a question is up, and no more.
#[cfg(not(feature = "adw-modern"))]
pub(crate) struct PresentedConfirmation;

/// The confirmation a channel currently has on screen — modern twin.
///
/// The dialog and the handler that turns a user answer into a message are
/// held together because closing on the reconcile path has to silence the
/// handler before it closes the dialog: `AdwAlertDialog` reports a close it
/// got no answer for as its close response, which would otherwise arrive as
/// a Cancel the user never gave.
#[cfg(feature = "adw-modern")]
pub(crate) struct PresentedConfirmation {
    dialog: adw::AlertDialog,
    response_handler_id: SignalHandlerId,
}

/// Puts a confirmation on screen — baseline twin.
///
/// The heading and body have no place in this channel: the model already put
/// the same warning on the status line, and the answer is the pair of buttons
/// the caller revealed. So this only records that the question is up.
#[cfg(not(feature = "adw-modern"))]
pub(crate) fn present_confirmation(
    _sender: &ComponentSender<ConfiguratorApp>,
    _parent: &impl IsA<gtk::Widget>,
    _heading: &str,
    _body: &str,
    _confirmation: &Confirmation,
) -> PresentedConfirmation {
    PresentedConfirmation
}

/// Puts a confirmation on screen — modern twin.
///
/// An `AdwAlertDialog` over `parent`'s window, with Cancel as both the close
/// response and the default, so Escape and the system close control land on
/// the answer that withdraws a destructive question. Every way the dialog can
/// end reaches the same [`Confirmation::message_for`] the baseline buttons
/// use, so the update layer sees one protocol.
#[cfg(feature = "adw-modern")]
pub(crate) fn present_confirmation(
    sender: &ComponentSender<ConfiguratorApp>,
    parent: &impl IsA<gtk::Widget>,
    heading: &str,
    body: &str,
    confirmation: &Confirmation,
) -> PresentedConfirmation {
    let dialog = adw::AlertDialog::new(Some(heading), Some(body));
    dialog.add_response(CANCEL_RESPONSE, "Cancel");
    dialog.add_response(CONFIRM_RESPONSE, confirmation.confirm_label());
    dialog.set_response_appearance(CONFIRM_RESPONSE, adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some(CANCEL_RESPONSE));
    dialog.set_close_response(CANCEL_RESPONSE);

    let response_handler_id = {
        let sender = sender.clone();
        let confirmation = confirmation.clone();
        dialog.connect_response(None, move |_, response| {
            sender.input(confirmation.message_for(response));
        })
    };

    dialog.present(Some(parent));
    PresentedConfirmation {
        dialog,
        response_handler_id,
    }
}

/// Takes a confirmation off screen because the model no longer holds it —
/// baseline twin.
///
/// Nothing to close: hiding the inline controls is the owner's next write,
/// driven by the same close row that got it here.
#[cfg(not(feature = "adw-modern"))]
pub(crate) fn close_confirmation(_presented: PresentedConfirmation) {}

/// Takes a confirmation off screen because the model no longer holds it —
/// modern twin.
///
/// The handler is blocked *before* the dialog closes. This close is
/// reconciliation, not an answer — the model already acted on, or withdrew,
/// the question — and `AdwAlertDialog` would otherwise report the close
/// response, sending a Cancel that answers a question nobody is asking.
#[cfg(feature = "adw-modern")]
pub(crate) fn close_confirmation(presented: PresentedConfirmation) {
    presented
        .dialog
        .block_signal(&presented.response_handler_id);
    presented.dialog.force_close();
}
