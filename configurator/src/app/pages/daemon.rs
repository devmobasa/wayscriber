//! Daemon page: background service, shortcut, and light-control setup.
//!
//! The one page that is a setup wizard rather than a list of preferences,
//! so its rows are custom widgets inside `AdwPreferencesGroup`s instead of
//! `PageBuilder` rows. Structure is built once; every label, button state,
//! and section visibility is a binding that reads the same
//! `DaemonRuntimeStatus` fields the Iced view read, and every button sends
//! the `DaemonAction` that view sent.

mod groups;
mod status;
#[cfg(test)]
mod tests;
mod widgets;

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;

use crate::messages::Message;
use crate::models::{
    DaemonAction, DaemonRuntimeStatus, LightShortcutApplyCapability, ShortcutApplyCapability, TabId,
};

use super::super::search::{AppSearchSummary, SearchArea};
use super::super::state::ConfiguratorApp;
use super::{Binding, BuiltPage, set_text_blocked};
use status::*;
use widgets::*;

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let page = adw::PreferencesPage::new();
    let mut bindings: Vec<Binding> = Vec::new();

    page.add(&groups::overview_group(sender, &mut bindings));

    for section in DaemonSection::ALL {
        let group = match section {
            DaemonSection::Install => groups::install_group(sender, &mut bindings),
            DaemonSection::Shortcut => groups::shortcut_group(sender, &mut bindings),
            DaemonSection::LightControls => groups::light_controls_group(sender, &mut bindings),
            DaemonSection::Start => groups::start_group(sender, &mut bindings),
            DaemonSection::TechnicalDetails => groups::details_group(sender, &mut bindings),
        };
        page.add(&group);
        bindings.push(Box::new(move |app, summary| {
            // The Iced view answered a status load with the details section
            // alone: until the environment is known there is nothing for the
            // setup steps to say.
            let visible = daemon_section_visible(section, shown_areas(summary))
                && (section == DaemonSection::TechnicalDetails || app.daemon_status.is_some());
            set_visible(&group, visible);
        }));
    }

    BuiltPage {
        widget: page.upcast(),
        bindings,
    }
}

// ---- Sections ----------------------------------------------------------

/// The setup steps, in the order the Iced view pushed them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DaemonSection {
    Install,
    Shortcut,
    LightControls,
    Start,
    TechnicalDetails,
}

impl DaemonSection {
    const ALL: [Self; 5] = [
        Self::Install,
        Self::Shortcut,
        Self::LightControls,
        Self::Start,
        Self::TechnicalDetails,
    ];
}

/// Which daemon search areas the current query left on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ShownAreas {
    status: bool,
    service: bool,
    shortcut: bool,
    light: bool,
}

fn daemon_section_visible(section: DaemonSection, shown: ShownAreas) -> bool {
    match section {
        DaemonSection::Install | DaemonSection::Start => shown.service,
        DaemonSection::Shortcut => shown.shortcut,
        DaemonSection::LightControls => shown.light,
        DaemonSection::TechnicalDetails => shown.status || shown.service,
    }
}

fn shown_areas(summary: &AppSearchSummary) -> ShownAreas {
    let matches = |area| {
        !summary.is_active()
            || summary
                .tab(TabId::Daemon)
                .is_some_and(|tab| tab.area_matches(area))
    };
    ShownAreas {
        status: matches(SearchArea::DaemonStatus),
        service: matches(SearchArea::DaemonService),
        shortcut: matches(SearchArea::DaemonShortcut),
        light: matches(SearchArea::DaemonLightControls),
    }
}
