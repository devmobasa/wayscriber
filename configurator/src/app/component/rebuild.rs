//! Redrawing the window in a new UI language.
//!
//! Every caption on a page is translated while the page is built, so a
//! language change builds the pages again from the model instead of
//! retitling thousands of labels one by one; the persistent chrome around
//! them is retitled in place. The model owns everything the user entered —
//! draft text, invalid input, open shortcut edits, active tabs, search — and
//! the first refresh writes it into the new pages. What the model does not
//! own is view state: which control has focus, the caret and selection in
//! it, and how far the visible page is scrolled. This module carries those
//! across.

use std::iter;

use relm4::{ComponentSender, gtk};

use gtk::glib;
use gtk::glib::GString;
use gtk::prelude::{GtkWindowExt, *};
use wayscriber::i18n;

use super::super::pages;
use super::super::state::ConfiguratorApp;
use super::{AppWidgets, shell};

/// A widget's position below a page: at each level, its index among its
/// parent's children and its type. A rebuilt page has the same shape while
/// the model gives it the same rows, so the same path finds the same control;
/// a type mismatch on the way means the control is gone.
#[derive(Debug, Clone, PartialEq)]
struct WidgetPath(Vec<(usize, glib::Type)>);

impl WidgetPath {
    fn of(widget: &gtk::Widget, page: &gtk::Widget) -> Option<Self> {
        let mut steps = Vec::new();
        let mut current = widget.clone();
        while &current != page {
            let parent = current.parent()?;
            let index = children(&parent).position(|child| child == current)?;
            steps.push((index, current.type_()));
            current = parent;
        }
        steps.reverse();

        Some(Self(steps))
    }

    fn resolve(&self, page: &gtk::Widget) -> Option<gtk::Widget> {
        let mut current = page.clone();
        for (index, kind) in &self.0 {
            let child = children(&current).nth(*index)?;
            if child.type_() != *kind {
                return None;
            }
            current = child;
        }

        Some(current)
    }
}

fn children(widget: &gtk::Widget) -> impl Iterator<Item = gtk::Widget> {
    iter::successors(widget.first_child(), gtk::Widget::next_sibling)
}

/// The caret and selection of an editable control, in characters.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Caret {
    position: i32,
    selection: Option<(i32, i32)>,
}

/// View state from the pages being replaced, applied to the new ones.
pub(super) struct ViewState {
    /// The stack name of the page the state was read from.
    page: Option<GString>,
    /// Set when focus was inside the visible page; focus elsewhere sits in
    /// persistent chrome, which the rebuild leaves alone.
    focus: Option<(Option<WidgetPath>, Option<Caret>)>,
    scrolls: Vec<(WidgetPath, f64, f64)>,
    transition: gtk::StackTransitionType,
}

/// Rebuilds the pages when the active language differs from the one they
/// were built in, and returns the view state to restore once the refresh
/// has written the model into them.
pub(super) fn rebuild_if_language_changed(
    widgets: &mut AppWidgets,
    sender: &ComponentSender<ConfiguratorApp>,
) -> Option<ViewState> {
    let language = i18n::language();
    if widgets.language == language {
        return None;
    }
    widgets.language = language;
    let state = capture(widgets);

    {
        // Closing the old pages' popovers is the rebuild's doing, not a
        // cancel from the user.
        let _replacing = pages::ReplacingPages::begin();
        // No crossfade: the new page simply takes the old one's place.
        widgets
            .stack
            .set_transition_type(gtk::StackTransitionType::None);
        while let Some(child) = widgets.stack.first_child() {
            widgets.stack.remove(&child);
        }
        // Dropping the bindings drops the last references to the old rows.
        widgets.bindings.clear();
        widgets.bindings = shell::install_pages(&widgets.stack, &widgets.sidebar_rows, sender);
    }
    shell::retitle(widgets);

    Some(state)
}

fn capture(widgets: &AppWidgets) -> ViewState {
    let transition = widgets.stack.transition_type();
    let page_name = widgets.stack.visible_child_name();
    let Some(page) = widgets.stack.visible_child() else {
        return ViewState {
            page: page_name,
            focus: None,
            scrolls: Vec::new(),
            transition,
        };
    };

    let focus = GtkWindowExt::focus(&widgets.root)
        .filter(|focused| focused.is_ancestor(&page))
        .map(|focused| {
            let caret = focused
                .downcast_ref::<gtk::Editable>()
                .map(|editable| Caret {
                    position: editable.position(),
                    selection: editable.selection_bounds(),
                });
            (WidgetPath::of(&focused, &page), caret)
        });
    let scrolls = descendants(&page)
        .filter_map(|widget| {
            let scrolled = widget.downcast_ref::<gtk::ScrolledWindow>()?;
            let path = WidgetPath::of(&widget, &page)?;
            Some((
                path,
                scrolled.hadjustment().value(),
                scrolled.vadjustment().value(),
            ))
        })
        .filter(|(_, horizontal, vertical)| *horizontal > 0.0 || *vertical > 0.0)
        .collect();

    ViewState {
        page: page_name,
        focus,
        scrolls,
        transition,
    }
}

fn descendants(widget: &gtk::Widget) -> impl Iterator<Item = gtk::Widget> {
    let mut pending = vec![widget.clone()];
    iter::from_fn(move || {
        let next = pending.pop()?;
        pending.extend(children(&next));
        Some(next)
    })
}

/// Puts focus, caret and scroll back on the rebuilt page. Called after the
/// refresh, so the page is the visible one and holds the model's values.
pub(super) fn restore(widgets: &AppWidgets, state: ViewState) {
    let page = widgets.stack.visible_child();
    // A path means something only on the page it was read from.
    let same_page = widgets.stack.visible_child_name() == state.page;

    if let (Some(page), Some((path, caret))) = (page.as_ref(), state.focus) {
        let target = path
            .filter(|_| same_page)
            .and_then(|path| path.resolve(page))
            .filter(|widget| widget.is_visible() && (widget.is_focusable() || widget.can_focus()));
        match target {
            Some(widget) if widget.grab_focus() => {
                if let (Some(editable), Some(caret)) =
                    (widget.downcast_ref::<gtk::Editable>(), caret)
                {
                    // `select_region` puts the caret on its second argument,
                    // and the bounds come back ordered: a selection made
                    // backwards keeps its active end where the caret was.
                    if let Some((start, end)) = caret.selection {
                        let (anchor, active) = if caret.position == start {
                            (end, start)
                        } else {
                            (start, end)
                        };
                        editable.select_region(anchor, active);
                    } else {
                        editable.select_region(caret.position, caret.position);
                        editable.set_position(caret.position);
                    }
                }
            }
            // The control did not survive the transition (a load or Defaults
            // changed the rows, or another page is showing now): the page's
            // first control takes focus.
            _ => {
                page.child_focus(gtk::DirectionType::TabForward);
            }
        }
    }

    if let Some(page) = page.as_ref().filter(|_| same_page) {
        for (path, horizontal, vertical) in state.scrolls {
            if let Some(scrolled) = path
                .resolve(page)
                .and_then(|widget| widget.downcast::<gtk::ScrolledWindow>().ok())
            {
                restore_scroll(&scrolled, horizontal, vertical);
            }
        }
    }

    widgets.stack.set_transition_type(state.transition);
}

/// Scrolls once the new page has been laid out: before its first
/// allocation an adjustment's range is empty and clamps every value to 0.
fn restore_scroll(scrolled: &gtk::ScrolledWindow, horizontal: f64, vertical: f64) {
    scrolled.add_tick_callback(move |scrolled, _clock| {
        if scrolled.height() == 0 {
            return glib::ControlFlow::Continue;
        }
        scrolled.hadjustment().set_value(horizontal);
        scrolled.vadjustment().set_value(vertical);
        glib::ControlFlow::Break
    });
}

/// A path finds the control at the same place in a tree of the same shape,
/// and nothing once the shape on its way has changed: that is what sends
/// focus to the page's first control instead of to a stranger.
#[cfg(test)]
pub(super) fn assert_widget_paths_find_only_the_same_shape() {
    fn page_with(second: Option<&gtk::Widget>) -> gtk::Box {
        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let group = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.append(&group);
        group.append(&gtk::Label::new(Some("title")));
        if let Some(second) = second {
            group.append(second);
        }
        page
    }

    let old_entry = gtk::Entry::new();
    let old_page = page_with(Some(old_entry.upcast_ref()));
    let path = WidgetPath::of(old_entry.upcast_ref(), old_page.upcast_ref()).expect("path");

    let new_entry = gtk::Entry::new();
    let same = page_with(Some(new_entry.upcast_ref()));
    assert_eq!(
        path.resolve(same.upcast_ref()),
        Some(new_entry.upcast::<gtk::Widget>())
    );

    let replaced = page_with(Some(gtk::Button::new().upcast_ref()));
    assert_eq!(path.resolve(replaced.upcast_ref()), None);
    let removed = page_with(None);
    assert_eq!(path.resolve(removed.upcast_ref()), None);
}
