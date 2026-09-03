use crate::config::{
    ResolvedToolbarItems, ToolbarConfig, ToolbarItemId, ToolbarItemOrderGroup,
    ToolbarItemVisibilitySetting, ToolbarItemsConfig, ToolbarLayoutMode, ToolbarModeOverrides,
    ToolbarRebindModifier, ToolbarSectionFlag, ToolbarSectionVisibility, TopDisplayMode,
    fold_legacy_section_flags, resolve_section_visibility, set_section_visibility,
};
use crate::input::state::TopMenuState;
use crate::ui::toolbar::ToolbarItemCustomizeGroup;

/// Toolbar-owned visibility state captured by transient chrome modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolbarVisibility {
    visible: bool,
    top_visible: bool,
    top_display_mode: TopDisplayMode,
    top_minimized: bool,
}

impl ToolbarVisibility {
    pub(crate) fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        self.top_visible = visible;
    }

    pub(crate) const fn top_display_mode(self) -> TopDisplayMode {
        self.top_display_mode
    }

    pub(in crate::input::state) fn effectively_visible(self) -> bool {
        self.top_visible && self.top_display_mode != TopDisplayMode::Hidden
    }

    pub(crate) fn set_top_display_mode(&mut self, mode: TopDisplayMode) {
        self.top_display_mode = mode;
    }
}

/// Runtime toolbar preferences, resolved layout, and interaction lifecycle.
#[derive(Debug, Clone)]
pub(in crate::input::state) struct ToolbarInteraction {
    visible: bool,
    top_visible: bool,
    top_pinned: bool,
    use_icons: bool,
    scale: f64,
    layout_mode: ToolbarLayoutMode,
    mode_overrides: ToolbarModeOverrides,
    items: ToolbarItemsConfig,
    resolved_items: ResolvedToolbarItems,
    customize_drag: Option<(ToolbarItemOrderGroup, ToolbarItemId)>,
    customize_items_open: bool,
    customize_items_group: Option<ToolbarItemCustomizeGroup>,
    status_bar_contents_open: bool,
    rebind_modifier: ToolbarRebindModifier,
    top_menu: TopMenuState,
    top_popover_scroll: f64,
    top_minimized: bool,
    top_display_mode: TopDisplayMode,
}

impl Default for ToolbarInteraction {
    fn default() -> Self {
        let items = ToolbarItemsConfig::default();
        let resolved_items = items.resolved();
        Self {
            visible: true,
            top_visible: true,
            top_pinned: true,
            use_icons: true,
            scale: 1.0,
            layout_mode: ToolbarLayoutMode::Regular,
            mode_overrides: ToolbarModeOverrides::default(),
            items,
            resolved_items,
            customize_drag: None,
            customize_items_open: false,
            customize_items_group: None,
            status_bar_contents_open: false,
            rebind_modifier: ToolbarRebindModifier::default(),
            top_menu: TopMenuState::Closed,
            top_popover_scroll: 0.0,
            top_minimized: false,
            top_display_mode: TopDisplayMode::Full,
        }
    }
}

impl ToolbarInteraction {
    pub(in crate::input::state) fn from_config(
        config: &ToolbarConfig,
        legacy_flags: &ToolbarSectionVisibility,
    ) -> Self {
        let mut legacy_flags = *legacy_flags;
        let mut items = config.items.clone();
        legacy_flags.apply_mode_override(config.mode_overrides.for_mode(config.layout_mode));
        fold_legacy_section_flags(
            &legacy_flags,
            config.layout_mode,
            &config.mode_overrides,
            &mut items,
        );
        let resolved_items = items.resolved();
        let top_display_mode = config.top_display_mode.persisted();
        Self {
            visible: config.top_pinned,
            top_visible: config.top_pinned,
            top_pinned: config.top_pinned,
            use_icons: config.use_icons,
            scale: config.scale,
            layout_mode: config.layout_mode,
            mode_overrides: config.mode_overrides.clone(),
            items,
            resolved_items,
            customize_drag: None,
            customize_items_open: false,
            customize_items_group: None,
            status_bar_contents_open: false,
            rebind_modifier: config.rebind_modifier,
            top_menu: TopMenuState::Closed,
            top_popover_scroll: 0.0,
            top_minimized: config.top_minimized,
            top_display_mode,
        }
    }

    #[cfg(test)]
    pub(in crate::input::state) const fn visible(&self) -> bool {
        self.visible
    }

    #[cfg(test)]
    pub(in crate::input::state) const fn top_visible(&self) -> bool {
        self.top_visible
    }

    pub(in crate::input::state) fn effectively_visible(&self) -> bool {
        self.top_visible && self.top_display_mode != TopDisplayMode::Hidden
    }

    pub(in crate::input::state) const fn top_pinned(&self) -> bool {
        self.top_pinned
    }

    pub(in crate::input::state) const fn use_icons(&self) -> bool {
        self.use_icons
    }

    pub(in crate::input::state) const fn scale(&self) -> f64 {
        self.scale
    }

    pub(in crate::input::state) const fn layout_mode(&self) -> ToolbarLayoutMode {
        self.layout_mode
    }

    #[cfg(test)]
    pub(in crate::input::state) const fn mode_overrides(&self) -> &ToolbarModeOverrides {
        &self.mode_overrides
    }

    #[cfg(test)]
    pub(in crate::input::state) const fn items(&self) -> &ToolbarItemsConfig {
        &self.items
    }

    pub(in crate::input::state) const fn resolved_items(&self) -> &ResolvedToolbarItems {
        &self.resolved_items
    }

    pub(in crate::input::state) const fn customize_drag(
        &self,
    ) -> Option<&(ToolbarItemOrderGroup, ToolbarItemId)> {
        self.customize_drag.as_ref()
    }

    pub(in crate::input::state) const fn customize_items_open(&self) -> bool {
        self.customize_items_open
    }

    pub(in crate::input::state) const fn customize_items_group(
        &self,
    ) -> Option<ToolbarItemCustomizeGroup> {
        self.customize_items_group
    }

    pub(in crate::input::state) const fn status_bar_contents_open(&self) -> bool {
        self.status_bar_contents_open
    }

    pub(in crate::input::state) const fn rebind_modifier(&self) -> ToolbarRebindModifier {
        self.rebind_modifier
    }

    pub(in crate::input::state) const fn top_menu(&self) -> TopMenuState {
        self.top_menu
    }

    pub(in crate::input::state) const fn top_popover_scroll(&self) -> f64 {
        self.top_popover_scroll
    }

    pub(in crate::input::state) const fn top_minimized(&self) -> bool {
        self.top_minimized
    }

    pub(in crate::input::state) const fn top_display_mode(&self) -> TopDisplayMode {
        self.top_display_mode
    }

    pub(in crate::input::state) fn visibility_snapshot(&self) -> ToolbarVisibility {
        ToolbarVisibility {
            visible: self.visible,
            top_visible: self.top_visible,
            top_display_mode: self.top_display_mode,
            top_minimized: self.top_minimized,
        }
    }

    pub(in crate::input::state) fn restore_visibility(&mut self, snapshot: ToolbarVisibility) {
        self.visible = snapshot.visible;
        self.top_visible = snapshot.top_visible;
        self.top_display_mode = snapshot.top_display_mode;
        self.top_minimized = snapshot.top_minimized;
    }

    pub(in crate::input::state) fn hide(&mut self) {
        self.visible = false;
        self.top_visible = false;
    }

    pub(in crate::input::state) fn show(&mut self) {
        self.visible = true;
        self.top_visible = true;
    }

    pub(in crate::input::state) fn derive_visibility_from_pins(&mut self) {
        self.top_visible = self.top_pinned;
        self.visible = self.top_visible;
    }

    pub(in crate::input::state) fn set_top_pinned(&mut self, pinned: bool) {
        self.top_pinned = pinned;
    }

    pub(in crate::input::state) fn set_visible(&mut self, visible: bool) -> bool {
        let unhide_top = visible && self.top_display_mode == TopDisplayMode::Hidden;
        let changed = unhide_top || self.visible != visible || self.top_visible != visible;
        if !changed {
            return false;
        }
        self.visible = visible;
        self.top_visible = visible;
        if unhide_top {
            self.top_display_mode = TopDisplayMode::Full;
        }
        true
    }

    pub(in crate::input::state) fn set_use_icons(&mut self, use_icons: bool) {
        self.use_icons = use_icons;
    }

    pub(in crate::input::state) fn set_top_minimized(&mut self, minimized: bool) -> bool {
        if self.top_minimized == minimized {
            return false;
        }
        self.top_minimized = minimized;
        if minimized {
            self.top_menu.close();
            self.customize_items_open = false;
            self.customize_items_group = None;
            self.status_bar_contents_open = false;
        }
        true
    }

    pub(in crate::input::state) fn set_top_display_mode(&mut self, mode: TopDisplayMode) -> bool {
        let changed = self.top_display_mode != mode;
        self.top_display_mode = mode;
        if matches!(mode, TopDisplayMode::Micro) {
            self.top_minimized = false;
        }
        if !matches!(mode, TopDisplayMode::Full) {
            self.top_menu.close();
        }
        if matches!(mode, TopDisplayMode::Hidden) {
            self.top_visible = false;
        } else {
            self.top_visible = true;
            self.visible = true;
        }
        changed
    }

    pub(in crate::input::state) fn cycle_top_display_mode(
        &mut self,
        current: TopDisplayMode,
    ) -> TopDisplayMode {
        let next = match current {
            TopDisplayMode::Full => TopDisplayMode::Micro,
            TopDisplayMode::Micro => TopDisplayMode::Hidden,
            TopDisplayMode::Hidden => TopDisplayMode::Full,
        };
        self.set_top_display_mode(next);
        next
    }

    pub(in crate::input::state) fn top_menu_flyout_open(&self) -> bool {
        self.top_menu.is_flyout()
    }

    pub(in crate::input::state) fn set_top_menu_open(
        &mut self,
        target: TopMenuState,
        open: bool,
    ) -> bool {
        self.top_menu.set_open(target, open)
    }

    pub(in crate::input::state) fn close_top_menu(&mut self) -> bool {
        self.top_menu.close()
    }

    pub(in crate::input::state) fn reset_top_popover_scroll(&mut self) -> bool {
        let changed = self.top_popover_scroll != 0.0;
        self.top_popover_scroll = 0.0;
        changed
    }

    pub(in crate::input::state) fn set_top_popover_scroll(&mut self, scroll: f64) -> bool {
        if !self.top_menu.is_popover() {
            return false;
        }
        let scroll = scroll.max(0.0);
        if (self.top_popover_scroll - scroll).abs() < 0.5 {
            return false;
        }
        self.top_popover_scroll = scroll;
        true
    }

    pub(in crate::input::state) fn begin_customize_drag(
        &mut self,
        drag: (ToolbarItemOrderGroup, ToolbarItemId),
    ) {
        self.customize_drag = Some(drag);
    }

    pub(in crate::input::state) fn clear_customize_drag(&mut self) -> bool {
        self.customize_drag.take().is_some()
    }

    pub(in crate::input::state) fn set_customize_items_open(&mut self, open: bool) -> bool {
        if self.customize_items_open == open {
            return false;
        }
        self.customize_items_open = open;
        if open {
            self.status_bar_contents_open = false;
        } else {
            self.customize_items_group = None;
        }
        true
    }

    pub(in crate::input::state) fn set_customize_items_group(
        &mut self,
        group: Option<ToolbarItemCustomizeGroup>,
    ) -> bool {
        if self.customize_items_group == group && self.customize_items_open {
            return false;
        }
        self.customize_items_open = true;
        self.customize_items_group = group;
        self.status_bar_contents_open = false;
        true
    }

    pub(in crate::input::state) fn set_status_bar_contents_open(&mut self, open: bool) -> bool {
        if self.status_bar_contents_open == open {
            return false;
        }
        self.status_bar_contents_open = open;
        if open {
            self.customize_items_open = false;
            self.customize_items_group = None;
        }
        true
    }

    pub(in crate::input::state) fn apply_layout_mode(&mut self, mode: ToolbarLayoutMode) {
        self.layout_mode = mode;
        self.refresh_resolved_items();
    }

    pub(in crate::input::state) fn set_section_visibility(
        &mut self,
        flag: ToolbarSectionFlag,
        visible: bool,
    ) -> bool {
        let changed = set_section_visibility(&mut self.items, flag, visible);
        if changed {
            self.refresh_resolved_items();
        }
        changed
    }

    pub(in crate::input::state) fn set_item_hidden(
        &mut self,
        id: ToolbarItemId,
        hidden: bool,
    ) -> bool {
        let before = self.items.clone();
        self.items.set_hidden(id, hidden);
        let changed = self.items != before;
        if changed {
            self.refresh_resolved_items();
        }
        changed
    }

    pub(in crate::input::state) fn set_item_visibility_setting(
        &mut self,
        id: ToolbarItemId,
        setting: ToolbarItemVisibilitySetting,
    ) -> bool {
        let changed = self.items.set_visibility_setting(id, setting);
        if changed {
            self.refresh_resolved_items();
        }
        changed
    }

    pub(in crate::input::state) fn set_item_order(
        &mut self,
        group: ToolbarItemOrderGroup,
        order: &[ToolbarItemId],
    ) -> bool {
        let changed = self.items.set_known_order(group, order);
        if changed {
            self.refresh_resolved_items();
        }
        changed
    }

    pub(in crate::input::state) fn move_item_by(
        &mut self,
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
        delta: isize,
    ) -> bool {
        let changed = self.items.move_item_by(group, id, delta);
        if changed {
            self.refresh_resolved_items();
        }
        changed
    }

    pub(in crate::input::state) fn move_dragged_item_to_index(
        &mut self,
        group: ToolbarItemOrderGroup,
        target_index: usize,
    ) -> bool {
        let Some((source_group, id)) = self.customize_drag else {
            return false;
        };
        if source_group != group {
            return false;
        }
        let changed = self.items.move_item_to_index(group, id, target_index);
        if changed {
            self.refresh_resolved_items();
        }
        changed
    }

    pub(in crate::input::state) fn reset_item_order(
        &mut self,
        group: ToolbarItemOrderGroup,
    ) -> bool {
        let changed = self.items.reset_known_order_to_defaults(group);
        if changed {
            self.refresh_resolved_items();
        }
        changed
    }

    pub(in crate::input::state) fn reset_individual_item_visibility(&mut self) -> bool {
        let mut changed = false;
        for (&id, &setting) in crate::config::factory_individual_toolbar_item_visibility_settings()
        {
            changed |= self.items.set_visibility_setting(id, setting);
        }
        if changed {
            self.refresh_resolved_items();
        }
        changed
    }

    pub(in crate::input::state) fn section_visibility(&self) -> ToolbarSectionVisibility {
        resolve_section_visibility(self.layout_mode, &self.mode_overrides, &self.resolved_items)
    }

    fn refresh_resolved_items(&mut self) {
        self.resolved_items = self.items.resolved();
    }

    #[cfg(test)]
    pub(in crate::input::state) fn override_visibility_for_test(
        &mut self,
        visible: bool,
        top_visible: bool,
        top_pinned: bool,
    ) {
        self.visible = visible;
        self.top_visible = top_visible;
        self.top_pinned = top_pinned;
    }

    #[cfg(test)]
    pub(in crate::input::state) fn override_appearance_for_test(
        &mut self,
        use_icons: bool,
        scale: f64,
    ) {
        self.use_icons = use_icons;
        self.scale = scale;
    }

    #[cfg(test)]
    pub(in crate::input::state) fn override_display_for_test(
        &mut self,
        mode: TopDisplayMode,
        minimized: bool,
    ) {
        self.top_display_mode = mode;
        self.top_minimized = minimized;
    }

    #[cfg(test)]
    pub(in crate::input::state) fn override_menu_for_test(
        &mut self,
        menu: TopMenuState,
        scroll: f64,
    ) {
        self.top_menu = menu;
        self.top_popover_scroll = scroll;
    }

    #[cfg(test)]
    pub(in crate::input::state) fn override_items_for_test(&mut self, items: ToolbarItemsConfig) {
        self.items = items;
        self.refresh_resolved_items();
    }

    #[cfg(all(test, feature = "toolbar-gtk"))]
    pub(in crate::input::state) fn override_layout_for_test(
        &mut self,
        mode: ToolbarLayoutMode,
        overrides: ToolbarModeOverrides,
    ) {
        self.layout_mode = mode;
        self.mode_overrides = overrides;
        self.refresh_resolved_items();
    }

    #[cfg(test)]
    pub(in crate::input::state) fn override_customization_for_test(
        &mut self,
        items_open: bool,
        group: Option<ToolbarItemCustomizeGroup>,
        status_bar_contents_open: bool,
    ) {
        self.customize_items_open = items_open;
        self.customize_items_group = group;
        self.status_bar_contents_open = status_bar_contents_open;
    }

    #[cfg(test)]
    pub(in crate::input::state) fn override_rebind_modifier_for_test(
        &mut self,
        modifier: ToolbarRebindModifier,
    ) {
        self.rebind_modifier = modifier;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_visibility() -> ToolbarSectionVisibility {
        ToolbarSectionVisibility {
            show_actions_section: true,
            show_actions_advanced: false,
            show_zoom_actions: true,
            show_pages_section: true,
            show_boards_section: true,
            show_presets: true,
            show_step_section: false,
            show_text_controls: true,
        }
    }

    #[test]
    fn from_config_resolves_legacy_flags_and_normalizes_hidden_display_mode() {
        let config = ToolbarConfig {
            layout_mode: ToolbarLayoutMode::Simple,
            top_pinned: false,
            top_display_mode: TopDisplayMode::Hidden,
            show_presets: true,
            ..ToolbarConfig::default()
        };

        let toolbar = ToolbarInteraction::from_config(&config, &legacy_visibility());

        assert!(!toolbar.visible());
        assert!(!toolbar.top_visible());
        assert!(!toolbar.top_pinned());
        assert_eq!(toolbar.top_display_mode(), TopDisplayMode::Full);
        assert!(
            toolbar
                .section_visibility()
                .get(ToolbarSectionFlag::Presets)
        );
    }

    #[test]
    fn visibility_snapshot_restores_the_complete_toolbar_state() {
        let mut toolbar = ToolbarInteraction::default();
        toolbar.set_top_display_mode(TopDisplayMode::Micro);
        toolbar.set_top_minimized(true);
        let snapshot = toolbar.visibility_snapshot();

        toolbar.hide();
        toolbar.set_top_display_mode(TopDisplayMode::Full);
        toolbar.restore_visibility(snapshot);

        assert!(toolbar.visible());
        assert!(toolbar.top_visible());
        assert_eq!(toolbar.top_display_mode(), TopDisplayMode::Micro);
        assert!(toolbar.top_minimized());
    }

    #[test]
    fn display_cycle_preserves_full_micro_hidden_semantics() {
        let mut toolbar = ToolbarInteraction::default();

        assert_eq!(
            toolbar.cycle_top_display_mode(TopDisplayMode::Full),
            TopDisplayMode::Micro
        );
        assert_eq!(
            toolbar.cycle_top_display_mode(TopDisplayMode::Micro),
            TopDisplayMode::Hidden
        );
        assert!(!toolbar.top_visible());
        assert_eq!(
            toolbar.cycle_top_display_mode(TopDisplayMode::Hidden),
            TopDisplayMode::Full
        );
        assert!(toolbar.top_visible());
        assert!(toolbar.top_pinned());
    }

    #[test]
    fn customization_drag_is_one_owned_lifecycle() {
        let mut toolbar = ToolbarInteraction::default();
        let drag = (
            ToolbarItemOrderGroup::TopTools,
            crate::config::toolbar_item_ids::TOP_TOOL_PEN,
        );

        toolbar.begin_customize_drag(drag);

        assert_eq!(toolbar.customize_drag(), Some(&drag));
        assert!(toolbar.clear_customize_drag());
        assert!(toolbar.customize_drag().is_none());
        assert!(!toolbar.clear_customize_drag());
    }

    #[test]
    fn dragging_an_item_reorders_only_its_group() {
        let mut toolbar = ToolbarInteraction::default();
        let group = ToolbarItemOrderGroup::TopTools;
        let pen = crate::config::toolbar_item_ids::TOP_TOOL_PEN;
        let before = toolbar.resolved_items().order.ordered_ids(group).to_vec();
        toolbar.begin_customize_drag((group, pen));

        assert!(toolbar.move_dragged_item_to_index(group, 3));
        assert_eq!(toolbar.resolved_items().order.ordered_ids(group)[3], pen);
        assert_ne!(toolbar.resolved_items().order.ordered_ids(group), before);
        assert!(toolbar.clear_customize_drag());
    }

    #[test]
    fn opening_a_top_menu_closes_the_previous_one() {
        let mut toolbar = ToolbarInteraction::default();

        assert!(toolbar.set_top_menu_open(TopMenuState::ShapePicker, true));
        assert_eq!(toolbar.top_menu(), TopMenuState::ShapePicker);
        assert!(toolbar.set_top_menu_open(TopMenuState::CanvasPopover, true));
        assert_eq!(toolbar.top_menu(), TopMenuState::CanvasPopover);
        assert!(toolbar.set_top_menu_open(TopMenuState::SettingsPopover, true));
        assert_eq!(toolbar.top_menu(), TopMenuState::SettingsPopover);
    }

    #[test]
    fn showing_transient_visibility_does_not_change_the_persisted_pin() {
        let mut toolbar = ToolbarInteraction::default();
        toolbar.set_top_pinned(false);
        toolbar.hide();

        toolbar.show();

        assert!(toolbar.visible());
        assert!(toolbar.top_visible());
        assert!(!toolbar.top_pinned());
    }

    #[test]
    fn deriving_visibility_from_pins_updates_both_live_flags() {
        let mut toolbar = ToolbarInteraction::default();
        toolbar.set_top_pinned(false);

        toolbar.derive_visibility_from_pins();

        assert!(!toolbar.visible());
        assert!(!toolbar.top_visible());
    }

    #[test]
    fn item_mutations_rebuild_resolved_items_and_section_visibility() {
        let mut toolbar = ToolbarInteraction::default();
        let flag = ToolbarSectionFlag::Presets;

        assert!(toolbar.set_section_visibility(flag, false));
        assert!(!toolbar.section_visibility().get(flag));
        assert!(toolbar.set_section_visibility(flag, true));
        assert!(toolbar.section_visibility().get(flag));
    }
}
