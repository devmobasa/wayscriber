use std::cell::{Cell, RefCell};
use std::rc::Rc;

use relm4::{ComponentSender, gtk};

use gtk::accessible::Property;
use gtk::glib::object::IsA;
use gtk::glib::{ControlFlow, SignalHandlerId};
use gtk::prelude::{AccessibleExt, *};

use crate::messages::Message;
use crate::models::KeybindingField;

use super::super::super::state::ConfiguratorApp;

pub(super) fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .has_frame(false)
        .css_classes(["flat"])
        .build();
    set_accessible_label(&button, tooltip);
    button
}

pub(super) fn set_accessible_label(widget: &impl AccessibleExt, label: &str) {
    widget.update_property(&[Property::Label(label)]);
}

/// Writes the widget's own visibility flag, never `is_visible`: a widget
/// inside a hidden parent reports invisible while its own flag still says
/// otherwise, and skipping the write there would leak the stale state the
/// moment the parent comes back.
pub(super) fn set_visible(widget: &impl IsA<gtk::Widget>, visible: bool) {
    if widget.get_visible() != visible {
        widget.set_visible(visible);
    }
}

pub(super) fn set_sensitive(widget: &impl IsA<gtk::Widget>, sensitive: bool) {
    if widget.is_sensitive() != sensitive {
        widget.set_sensitive(sensitive);
    }
}

pub(super) fn set_label(label: &gtk::Label, text: &str) {
    if label.label() != text {
        label.set_label(text);
    }
}

pub(super) fn set_tooltip(widget: &impl IsA<gtk::Widget>, tooltip: Option<&str>) {
    if widget.tooltip_text().as_deref() != tooltip {
        widget.set_tooltip_text(tooltip);
    }
}

pub(super) const COMPACT_WIDTH: i32 = 560;

pub(super) fn watch_compact(row: &impl IsA<gtk::Widget>, apply: impl Fn(bool) + 'static) {
    let widget = row.as_ref().clone();
    widget.connect_notify_local(Some("width"), move |widget, _| {
        let width = widget.width();
        if width <= 0 {
            return;
        }
        apply(width < COMPACT_WIDTH);
    });
}

pub(super) fn connect_clicked(
    button: &gtk::Button,
    sender: &ComponentSender<ConfiguratorApp>,
    message: Message,
) {
    let sender = sender.clone();
    button.connect_clicked(move |_| {
        sender.input(message.clone());
    });
}

/// `GtkPopover` widgets created with `set_parent` are not ordinary children.
/// If they are still attached when the parent is destroyed, GTK warns on
/// finalize. Unparent in the destroy handler so shutdown stays quiet.
pub(super) fn unparent_on_destroy(parent: &impl IsA<gtk::Widget>, popover: &gtk::Popover) {
    let popover = popover.clone();
    parent.connect_destroy(move |_| {
        if popover.parent().is_some() {
            popover.unparent();
        }
    });
}

/// Opens an editor popover only where GTK can place it.
///
/// GTK closes a popover whose row is off screen or has no layout, and
/// [`field_canceled`] reports that close as the user canceling the edit. So an
/// open edit's popover waits while rebuilt pages settle, which ends with their
/// own `PagesLaidOut`. After that, a row without layout, or one a translation
/// moved out of the restored scroll, gets its layout or is scrolled into view
/// first, and one `PagesLaidOut` after that layout brings the refresh back to
/// open the popover. A row that is not showing, on another page or in a hidden
/// category, waits until it is shown and laid out the same way.
pub(super) struct PopoverPresenter {
    sender: ComponentSender<ConfiguratorApp>,
    retry_pending: Rc<Cell<bool>>,
}

impl PopoverPresenter {
    pub(super) fn new(sender: &ComponentSender<ConfiguratorApp>) -> Self {
        Self {
            sender: sender.clone(),
            retry_pending: Rc::new(Cell::new(false)),
        }
    }

    /// Opens `popover` and focuses `focus`, or leaves it closed for a later
    /// refresh. `may_open` is false while a Leave confirmation is pending: the
    /// popover's focus would land on its row, and selecting the row withdraws
    /// the confirmation.
    pub(super) fn present(
        &self,
        popover: &gtk::Popover,
        focus: &impl IsA<gtk::Widget>,
        may_open: bool,
    ) {
        if popover.is_visible()
            || !may_open
            || !super::super::pages_settled()
            || self.retry_pending.get()
        {
            return;
        }
        let Some(anchor) = popover.parent() else {
            return;
        };
        if !anchor.is_mapped() {
            self.retry_when_shown(&anchor);
            return;
        }

        if anchor.height() == 0 {
            self.retry_after_layout(&anchor);
            return;
        }
        match reveal(&anchor) {
            Reveal::Shown | Reveal::Unreachable => {
                popover.popup();
                focus.grab_focus();
                // Focus and caret from before a rebuild, when this opening resumes that edit.
                super::super::run_deferred_focus(popover);
            }
            Reveal::Scrolled => self.retry_after_layout(&anchor),
        }
    }

    /// Closes `popover` once its edit has ended, and forgets any focus a rebuild
    /// kept for it.
    pub(super) fn close(&self, popover: &gtk::Popover) {
        super::super::drop_deferred_focus(popover);
        if popover.is_visible() {
            popover.popdown();
        }
    }

    /// Waits for `anchor` to be shown, which the refresh that shows it may do
    /// only after this row's binding ran, then for its layout.
    fn retry_when_shown(&self, anchor: &gtk::Widget) {
        if self.retry_pending.replace(true) {
            return;
        }

        let sender = self.sender.clone();
        let retry_pending = self.retry_pending.clone();
        let handler = Rc::new(RefCell::new(None::<SignalHandlerId>));
        let id = anchor.connect_map({
            let handler = handler.clone();
            move |anchor| {
                if let Some(id) = handler.borrow_mut().take() {
                    anchor.disconnect(id);
                }
                refresh_after_layout(anchor, sender.clone(), retry_pending.clone());
            }
        });
        *handler.borrow_mut() = Some(id);
    }

    fn retry_after_layout(&self, anchor: &gtk::Widget) {
        if self.retry_pending.replace(true) {
            return;
        }
        refresh_after_layout(anchor, self.sender.clone(), self.retry_pending.clone());
    }
}

/// Sends `PagesLaidOut` once `anchor` has a layout that reflects the latest
/// change: the tick that first sees the row laid out runs before that frame's
/// layout, so the message waits one more frame. A row that stops showing ends
/// the wait instead; the next refresh waits for it to be shown again.
fn refresh_after_layout(
    anchor: &gtk::Widget,
    sender: ComponentSender<ConfiguratorApp>,
    retry_pending: Rc<Cell<bool>>,
) {
    let seen_laid_out = Cell::new(false);
    anchor.add_tick_callback(move |anchor, _clock| {
        if !anchor.is_mapped() {
            retry_pending.set(false);
            return ControlFlow::Break;
        }
        if anchor.height() == 0 || !seen_laid_out.replace(true) {
            return ControlFlow::Continue;
        }
        retry_pending.set(false);
        sender.input(Message::PagesLaidOut);
        ControlFlow::Break
    });
}

/// What [`reveal`] did with a row.
enum Reveal {
    /// Already on screen, or not inside a scrolled page.
    Shown,
    /// Moved toward the view; its new position needs a layout first.
    Scrolled,
    /// Off screen, but the page cannot scroll any closer.
    Unreachable,
}

/// Scrolls the page around `anchor` until all of it is on screen, or its top
/// when it is taller than the view.
fn reveal(anchor: &gtk::Widget) -> Reveal {
    let Some(scrolled) = anchor
        .ancestor(gtk::ScrolledWindow::static_type())
        .and_then(|widget| widget.downcast::<gtk::ScrolledWindow>().ok())
    else {
        return Reveal::Shown;
    };
    let Some(bounds) = anchor.compute_bounds(&scrolled) else {
        return Reveal::Unreachable;
    };

    let view = f64::from(scrolled.height());
    let top = f64::from(bounds.y());
    let bottom = top + f64::from(bounds.height());
    let shift = if top < 0.0 {
        top
    } else if bottom > view {
        (bottom - view).min(top)
    } else {
        return Reveal::Shown;
    };

    let adjustment = scrolled.vadjustment();
    let before = adjustment.value();
    adjustment.set_value(before + shift);
    if (adjustment.value() - before).abs() > f64::EPSILON {
        Reveal::Scrolled
    } else {
        Reveal::Unreachable
    }
}

pub(super) fn field_canceled(
    popover: &gtk::Popover,
    field: KeybindingField,
    sender: &ComponentSender<ConfiguratorApp>,
    to_message: fn(KeybindingField) -> Message,
) {
    let sender = sender.clone();
    popover.connect_closed(move |_| {
        // Closed by a page rebuild rather than by the user: the edit is
        // still the model's, and the rebuilt page reopens it.
        if super::super::replacing_pages() {
            return;
        }
        sender.input(to_message(field));
    });
}
