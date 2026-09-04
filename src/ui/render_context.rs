use super::help_overlay::HelpLayoutCache;
use super::radial_menu::RadialBaseCache;
use super::theme::Theme;

/// Paint resources retained by one overlay. Drawing caches remain independent.
#[derive(Default)]
pub(crate) struct UiRenderCaches {
    help: HelpLayoutCache,
    radial: RadialBaseCache,
}

impl UiRenderCaches {
    pub(in crate::ui) fn help_mut(&mut self) -> &mut HelpLayoutCache {
        &mut self.help
    }

    pub(in crate::ui) fn radial_mut(&mut self) -> &mut RadialBaseCache {
        &mut self.radial
    }
}

/// A short UI paint pass borrowing an explicit theme and its owner's resources.
pub(crate) struct UiRenderCtx<'c, 't, 'r> {
    pub cairo: &'c cairo::Context,
    pub theme: &'t Theme,
    pub caches: &'r mut UiRenderCaches,
}
