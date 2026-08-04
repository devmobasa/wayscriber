//! Session persistence settings and the saved-session catalog.

mod catalog;
mod settings;

use relm4::prelude::*;

use crate::models::TabId;

use super::super::state::ConfiguratorApp;
use super::{BuiltPage, PageBuilder};

/// The Session page keeps one small interface while its two independent
/// areas own their construction and refresh details.
pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let mut page = PageBuilder::new(sender, TabId::Session);
    settings::add(&mut page);
    catalog::add(&mut page);
    page.finish()
}
