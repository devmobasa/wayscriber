//! UI page: general chrome settings plus the seven UI sub-pages.
//!
//! This page is bigger than one preferences page, so it composes its own
//! root: a "General UI" preferences page above a stack holding one
//! preferences page per [`UiTabId`]. Which sub-page shows is model state —
//! the switcher only sends [`Message::UiTabSelected`], and a binding drives
//! the visible child from `active_ui_tab`, so a deep link and the search
//! realignment that moves that field both land on the right sub-page.

mod click_highlight;
mod general;
mod help_overlay;
mod input_hud;
mod presenter_mode;
mod status_bar;
mod toolbar;
mod toolbar_visibility;

use relm4::gtk;
use relm4::prelude::*;

use gtk::prelude::*;

use crate::messages::Message;
use crate::models::color::parse_quad_values;
use crate::models::{TabId, UiTabId};

use super::super::search::SearchArea;
use super::super::state::ConfiguratorApp;
use super::color_rows::ResolvedColor;
use super::{Binding, BuiltPage};

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut bindings: Vec<Binding> = Vec::new();

    let general = general::build(sender);
    bindings.extend(general.bindings);
    let general_widget = general.widget;
    // Natural height inside the shared scroller: the general section and the
    // active sub-page scroll together the way the Iced column did.
    general_widget.set_vexpand(false);
    {
        let widget = general_widget.clone();
        bindings.push(Box::new(move |_app, summary| {
            let visible = !summary.is_active()
                || summary
                    .tab(TabId::Ui)
                    .is_some_and(|tab| tab.area_matches(SearchArea::UiGeneral));
            if widget.is_visible() != visible {
                widget.set_visible(visible);
            }
        }));
    }

    // Homogeneous sizing would make every sub-page as tall as the tallest
    // one (toolbar visibility lists every item), so measure the visible
    // child only.
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vhomogeneous(false)
        .vexpand(true)
        .build();

    let mut stack_pages: Vec<(UiTabId, gtk::StackPage)> = Vec::new();
    for tab in UiTabId::ALL {
        let built = build_ui_tab(sender, tab);
        bindings.extend(built.bindings);
        built.widget.set_vexpand(false);
        let stack_page = stack.add_titled(&built.widget, Some(tab.title()), tab.title());
        stack_pages.push((tab, stack_page));
    }
    {
        let sender = sender.clone();
        stack.connect_visible_child_name_notify(move |stack| {
            let Some(name) = stack.visible_child_name() else {
                return;
            };
            if let Some(tab) = UiTabId::ALL.into_iter().find(|tab| tab.title() == name) {
                sender.input(Message::UiTabSelected(tab));
            }
        });
    }

    let switcher = gtk::StackSwitcher::builder()
        .stack(&stack)
        .halign(gtk::Align::Center)
        .build();
    // Seven sub-tabs are wider than a narrow window: scrolling the row keeps
    // the switcher off the page's minimum width, so the sections below stay
    // centered instead of being pushed sideways.
    let switcher_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .margin_top(6)
        .margin_start(12)
        .margin_end(12)
        .child(&switcher)
        .build();

    {
        let stack = stack.clone();
        let switcher = switcher_scroll.clone();
        bindings.push(Box::new(move |app, summary| {
            let ui_summary = summary.tab(TabId::Ui);
            let searching = summary.is_active();
            let mut any_visible = false;
            // Reveal before switching and hide after: the stack drops a
            // visible child that goes invisible, and picking the section the
            // model asks for first keeps that fallback out of the way.
            for (tab, stack_page) in &stack_pages {
                let visible =
                    !searching || ui_summary.is_some_and(|summary| summary.ui_tab_visible(*tab));
                if visible && !stack_page.is_visible() {
                    stack_page.set_visible(true);
                }
                any_visible |= visible;
            }
            if switcher.is_visible() != any_visible {
                switcher.set_visible(any_visible);
            }
            if stack.is_visible() != any_visible {
                stack.set_visible(any_visible);
            }

            let name = app.active_ui_tab.title();
            if stack.visible_child_name().as_deref() != Some(name) {
                stack.set_visible_child_name(name);
            }
            for (tab, stack_page) in &stack_pages {
                let visible =
                    !searching || ui_summary.is_some_and(|summary| summary.ui_tab_visible(*tab));
                if !visible && stack_page.is_visible() {
                    stack_page.set_visible(false);
                }
            }
        }));
    }

    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.append(&general_widget);
    content.append(&switcher_scroll);
    content.append(&stack);

    // The preferences pages carry scrollers of their own, whose minimum
    // height is nearly zero, so a viewport on the default minimum policy
    // would squeeze them instead of scrolling. Scrolling the natural height
    // gives each section its full size and one scrollbar for the page.
    let viewport = gtk::Viewport::builder()
        .vscroll_policy(gtk::ScrollablePolicy::Natural)
        .child(&content)
        .build();
    let root = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&viewport)
        .build();

    BuiltPage {
        widget: root.upcast(),
        bindings,
    }
}

fn build_ui_tab(sender: &ComponentSender<ConfiguratorApp>, tab: UiTabId) -> BuiltPage {
    match tab {
        UiTabId::Toolbar => toolbar::build(sender),
        UiTabId::ToolbarVisibility => toolbar_visibility::build(sender),
        UiTabId::StatusBar => status_bar::build(sender),
        UiTabId::HelpOverlay => help_overlay::build(sender),
        UiTabId::ClickHighlight => click_highlight::build(sender),
        UiTabId::InputHud => input_hud::build(sender),
        UiTabId::PresenterMode => presenter_mode::build(sender),
    }
}

// ---- Shared helpers --------------------------------------------------------

/// A combo row's values with their labels, in one call.
fn options<O: Copy>(values: Vec<O>, label: impl Fn(&O) -> &'static str) -> (Vec<O>, Vec<String>) {
    let labels = values
        .iter()
        .map(|value| label(value).to_string())
        .collect();
    (values, labels)
}

/// Explanatory text a row cannot carry: `AdwEntryRow` has no subtitle, and a
/// preferences group renders a plain widget under its rows.
fn note(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .margin_top(6)
        .css_classes(["caption", "dim-label"])
        .build()
}

fn quad_color(components: &[String; 4]) -> ResolvedColor {
    let values = parse_quad_values(components);
    Some((values[0], values[1], values[2], values[3]))
}

/// Error text for a decimal field constrained to `min..=max`, `None` while
/// the input is acceptable.
fn validate_f64_range(value: &str, min: f64, max: f64) -> Option<String> {
    match value.trim().parse::<f64>() {
        Ok(parsed) if parsed.is_finite() && (min..=max).contains(&parsed) => None,
        _ => Some(format!("Enter a number between {min} and {max}.")),
    }
}
