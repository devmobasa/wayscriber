//! Presenter, focus, and light-mode lifecycle state.
//!
//! This owner captures and restores transient chrome state. `InputState`
//! remains the coordinator for mutations that belong to the toolbar, UI
//! visibility, drawing style, feedback, and backend effect owners.

use super::ToolbarVisibility;
use crate::config::{PresenterModeConfig, PresenterToolBehavior, TopDisplayMode};
use crate::input::tool::Tool;

#[derive(Debug)]
pub(in crate::input::state) struct ChromeModes {
    presenter: bool,
    presenter_config: PresenterModeConfig,
    presenter_restore: Option<PresenterRestore>,
    focus_restore: Option<FocusModeRestore>,
    light: bool,
    light_drawing: bool,
    light_restore: Option<LightModeRestore>,
}

impl ChromeModes {
    pub(in crate::input::state) fn new(presenter_config: PresenterModeConfig) -> Self {
        Self {
            presenter: false,
            presenter_config,
            presenter_restore: None,
            focus_restore: None,
            light: false,
            light_drawing: false,
            light_restore: None,
        }
    }

    pub(in crate::input::state) const fn presenter_active(&self) -> bool {
        self.presenter
    }

    pub(in crate::input::state) const fn presenter_config(&self) -> &PresenterModeConfig {
        &self.presenter_config
    }

    pub(in crate::input::state) fn presenter_hides_toolbars(&self) -> bool {
        self.presenter && self.presenter_config.hide_toolbars
    }

    pub(in crate::input::state) fn begin_presenter(&mut self, restore: PresenterRestore) {
        self.presenter_restore = Some(restore);
        self.presenter = true;
    }

    pub(in crate::input::state) fn end_presenter(&mut self) -> Option<PresenterRestore> {
        if !self.presenter {
            return None;
        }
        self.presenter = false;
        self.presenter_restore.take()
    }

    #[cfg(test)]
    pub(in crate::input::state) fn presenter_restore_pending(&self) -> bool {
        self.presenter_restore.is_some()
    }

    pub(in crate::input::state) fn presenter_restored_status_bar(&self) -> Option<bool> {
        self.presenter_restore
            .as_ref()
            .and_then(PresenterRestore::show_status_bar)
    }

    pub(in crate::input::state) fn presenter_restored_tool_preview(&self) -> Option<bool> {
        self.presenter_restore
            .as_ref()
            .and_then(PresenterRestore::show_tool_preview)
    }

    pub(in crate::input::state) fn presenter_restored_click_highlight(&self) -> Option<bool> {
        self.presenter_restore
            .as_ref()
            .and_then(PresenterRestore::click_highlight_enabled)
    }

    pub(in crate::input::state) fn presenter_restored_top_display_mode(
        &self,
    ) -> Option<TopDisplayMode> {
        self.presenter_restore
            .as_ref()
            .and_then(PresenterRestore::toolbar_visibility)
            .map(ToolbarVisibility::top_display_mode)
    }

    pub(in crate::input::state) fn presenter_restores_visible_toolbar(&self) -> bool {
        self.presenter_restore
            .as_ref()
            .and_then(PresenterRestore::toolbar_visibility)
            .is_some_and(ToolbarVisibility::effectively_visible)
    }

    pub(in crate::input::state) fn retarget_presenter_toolbar_display_mode(
        &mut self,
        mode: TopDisplayMode,
    ) -> bool {
        let Some(toolbar) = self
            .presenter_restore
            .as_mut()
            .and_then(PresenterRestore::toolbar_visibility_mut)
        else {
            return false;
        };
        toolbar.set_top_display_mode(mode);
        true
    }

    pub(in crate::input::state) const fn focus_active(&self) -> bool {
        self.focus_restore.is_some()
    }

    pub(in crate::input::state) fn begin_focus(&mut self, restore: FocusModeRestore) {
        self.focus_restore = Some(restore);
    }

    pub(in crate::input::state) fn end_focus(&mut self) -> Option<FocusModeRestore> {
        self.focus_restore.take()
    }

    pub(in crate::input::state) fn cancel_focus(&mut self) {
        self.focus_restore = None;
    }

    pub(in crate::input::state) fn retarget_focus_status_bar(
        &mut self,
        show: bool,
    ) -> Option<bool> {
        let restore = self.focus_restore.as_mut()?;
        let changed = restore.show_status_bar != show;
        restore.show_status_bar = show;
        Some(changed)
    }

    pub(in crate::input::state) const fn light_active(&self) -> bool {
        self.light
    }

    pub(in crate::input::state) const fn light_drawing(&self) -> bool {
        self.light_drawing
    }

    pub(in crate::input::state) fn set_light_drawing(&mut self, drawing: bool) {
        self.light_drawing = drawing;
    }

    pub(in crate::input::state) fn begin_light(
        &mut self,
        drawing: bool,
        restore: LightModeRestore,
    ) {
        self.light_restore = Some(restore);
        self.light = true;
        self.light_drawing = drawing;
    }

    pub(in crate::input::state) fn end_light(&mut self) -> Option<LightModeRestore> {
        if !self.light {
            return None;
        }
        self.light = false;
        self.light_drawing = false;
        self.light_restore.take()
    }

    pub(in crate::input::state) fn light_restored_tool_override(&self) -> Option<Option<Tool>> {
        self.light_restore
            .as_ref()
            .map(|restore| restore.tool_override())
    }

    pub(in crate::input::state) fn retarget_visibility_from_pin(&mut self, visible: bool) -> bool {
        if let Some(restore) = self.focus_restore.as_mut() {
            restore.toolbar_visibility.set_visible(visible);
            return true;
        }
        if let Some(toolbar) = self
            .presenter_restore
            .as_mut()
            .and_then(PresenterRestore::toolbar_visibility_mut)
        {
            toolbar.set_visible(visible);
            return true;
        }
        if let Some(restore) = self.light_restore.as_mut() {
            restore.toolbar_visibility.set_visible(visible);
            return true;
        }
        false
    }

    #[cfg(test)]
    pub(in crate::input::state) fn presenter_config_mut_for_test(
        &mut self,
    ) -> &mut PresenterModeConfig {
        &mut self.presenter_config
    }

    #[cfg(test)]
    pub(in crate::input::state) fn override_presenter_for_test(&mut self, active: bool) {
        self.presenter = active;
        if !active {
            self.presenter_restore = None;
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::input::state) struct PresenterRestore {
    show_status_bar: Option<bool>,
    show_tool_preview: Option<bool>,
    toolbar_visibility: Option<ToolbarVisibility>,
    click_highlight_enabled: Option<bool>,
    input_hud_enabled: Option<bool>,
    tool_override: Option<Option<Tool>>,
}

impl PresenterRestore {
    pub(in crate::input::state) fn capture(
        config: &PresenterModeConfig,
        show_status_bar: bool,
        show_tool_preview: bool,
        toolbar_visibility: ToolbarVisibility,
        click_highlight_enabled: bool,
        input_hud_enabled: bool,
        tool_override: Option<Tool>,
    ) -> Self {
        Self {
            show_status_bar: config.hide_status_bar.then_some(show_status_bar),
            show_tool_preview: config.hide_tool_preview.then_some(show_tool_preview),
            toolbar_visibility: config.hide_toolbars.then_some(toolbar_visibility),
            click_highlight_enabled: config
                .enable_click_highlight
                .then_some(click_highlight_enabled),
            input_hud_enabled: config.enable_input_hud.then_some(input_hud_enabled),
            tool_override: (!matches!(config.tool_behavior, PresenterToolBehavior::Keep))
                .then_some(tool_override),
        }
    }

    pub(in crate::input::state) const fn show_status_bar(&self) -> Option<bool> {
        self.show_status_bar
    }

    pub(in crate::input::state) const fn show_tool_preview(&self) -> Option<bool> {
        self.show_tool_preview
    }

    pub(in crate::input::state) const fn toolbar_visibility(&self) -> Option<ToolbarVisibility> {
        self.toolbar_visibility
    }

    fn toolbar_visibility_mut(&mut self) -> Option<&mut ToolbarVisibility> {
        self.toolbar_visibility.as_mut()
    }

    pub(in crate::input::state) const fn click_highlight_enabled(&self) -> Option<bool> {
        self.click_highlight_enabled
    }

    pub(in crate::input::state) const fn input_hud_enabled(&self) -> Option<bool> {
        self.input_hud_enabled
    }

    pub(in crate::input::state) const fn tool_override(&self) -> Option<Option<Tool>> {
        self.tool_override
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::input::state) struct FocusModeRestore {
    show_status_bar: bool,
    toolbar_visibility: ToolbarVisibility,
    show_floating_badge: bool,
    show_zoom_chip: bool,
}

impl FocusModeRestore {
    pub(in crate::input::state) const fn capture(
        show_status_bar: bool,
        toolbar_visibility: ToolbarVisibility,
        show_floating_badge: bool,
        show_zoom_chip: bool,
    ) -> Self {
        Self {
            show_status_bar,
            toolbar_visibility,
            show_floating_badge,
            show_zoom_chip,
        }
    }

    pub(in crate::input::state) const fn show_status_bar(self) -> bool {
        self.show_status_bar
    }

    pub(in crate::input::state) const fn toolbar_visibility(self) -> ToolbarVisibility {
        self.toolbar_visibility
    }

    pub(in crate::input::state) const fn show_floating_badge(self) -> bool {
        self.show_floating_badge
    }

    pub(in crate::input::state) const fn show_zoom_chip(self) -> bool {
        self.show_zoom_chip
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::input::state) struct LightModeRestore {
    show_status_bar: bool,
    show_tool_preview: bool,
    toolbar_visibility: ToolbarVisibility,
    click_highlight_enabled: bool,
    tool_override: Option<Tool>,
}

impl LightModeRestore {
    pub(in crate::input::state) const fn capture(
        show_status_bar: bool,
        show_tool_preview: bool,
        toolbar_visibility: ToolbarVisibility,
        click_highlight_enabled: bool,
        tool_override: Option<Tool>,
    ) -> Self {
        Self {
            show_status_bar,
            show_tool_preview,
            toolbar_visibility,
            click_highlight_enabled,
            tool_override,
        }
    }

    pub(in crate::input::state) const fn show_status_bar(self) -> bool {
        self.show_status_bar
    }

    pub(in crate::input::state) const fn show_tool_preview(self) -> bool {
        self.show_tool_preview
    }

    pub(in crate::input::state) const fn toolbar_visibility(self) -> ToolbarVisibility {
        self.toolbar_visibility
    }

    pub(in crate::input::state) const fn click_highlight_enabled(self) -> bool {
        self.click_highlight_enabled
    }

    pub(in crate::input::state) const fn tool_override(self) -> Option<Tool> {
        self.tool_override
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PresenterModeConfig, PresenterToolbarMode};
    use crate::input::state::core::ToolbarInteraction;
    use crate::input::tool::Tool;

    fn toolbar_visibility() -> crate::input::state::core::ToolbarVisibility {
        ToolbarInteraction::default().visibility_snapshot()
    }

    #[test]
    fn presenter_snapshot_round_trips_complete_toolbar_visibility_for_micro_mapping() {
        let config = PresenterModeConfig {
            hide_toolbars: true,
            toolbar_mode: PresenterToolbarMode::Micro,
            ..PresenterModeConfig::default()
        };
        let visibility = toolbar_visibility();
        let restore = PresenterRestore::capture(
            &config,
            true,
            true,
            visibility,
            false,
            false,
            Some(Tool::Arrow),
        );
        let mut modes = ChromeModes::new(config);

        modes.begin_presenter(restore);
        let restored = modes.end_presenter().expect("presenter restore");

        assert_eq!(restored.toolbar_visibility(), Some(visibility));
    }

    #[test]
    fn presenter_without_toolbar_hiding_has_no_toolbar_retarget() {
        let config = PresenterModeConfig {
            hide_toolbars: false,
            ..PresenterModeConfig::default()
        };
        let restore = PresenterRestore::capture(
            &config,
            true,
            true,
            toolbar_visibility(),
            false,
            false,
            None,
        );
        let mut modes = ChromeModes::new(config);

        modes.begin_presenter(restore);

        assert!(
            !modes.retarget_presenter_toolbar_display_mode(crate::config::TopDisplayMode::Micro)
        );
        assert_eq!(modes.end_presenter().unwrap().toolbar_visibility(), None);
    }

    #[test]
    fn ending_inactive_modes_returns_no_restore() {
        let mut modes = ChromeModes::new(PresenterModeConfig::default());

        assert!(modes.end_presenter().is_none());
        assert!(modes.end_focus().is_none());
        assert!(modes.end_light().is_none());
        assert!(!modes.presenter_active());
        assert!(!modes.focus_active());
        assert!(!modes.light_active());
    }

    #[test]
    fn focus_and_light_snapshots_round_trip_their_values() {
        let visibility = toolbar_visibility();
        let focus = FocusModeRestore::capture(false, visibility, true, false);
        let light = LightModeRestore::capture(false, true, visibility, true, Some(Tool::Pen));
        let mut modes = ChromeModes::new(PresenterModeConfig::default());

        modes.begin_focus(focus);
        let focus = modes.end_focus().expect("focus restore");
        assert!(!focus.show_status_bar());
        assert_eq!(focus.toolbar_visibility(), visibility);
        assert!(focus.show_floating_badge());
        assert!(!focus.show_zoom_chip());

        modes.begin_light(true, light);
        assert!(modes.light_active());
        assert!(modes.light_drawing());
        let light = modes.end_light().expect("light restore");
        assert!(!light.show_status_bar());
        assert!(light.show_tool_preview());
        assert_eq!(light.toolbar_visibility(), visibility);
        assert!(light.click_highlight_enabled());
        assert_eq!(light.tool_override(), Some(Tool::Pen));
        assert!(!modes.light_active());
        assert!(!modes.light_drawing());
    }
}
