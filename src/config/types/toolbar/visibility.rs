//! One resolver for section-level toolbar visibility.
//!
//! Effective visibility = explicit user overrides (`items.shown` /
//! `items.hidden`) layered over the layout-mode baseline
//! (`section_defaults()` adjusted by `mode_overrides`). The nine legacy
//! `show_*` booleans become derived mirrors of this resolver: they are
//! still read everywhere and still written to config so configs written by
//! this version keep working in older ones, but the overrides are the
//! source of truth — switching layout modes no longer erases hand-tuned
//! section settings.

use super::ids;
use super::items::{ResolvedToolbarItems, ToolbarItemId, ToolbarItemsConfig};
use super::mode::ToolbarLayoutMode;
use super::overrides::ToolbarModeOverrides;

/// The section-level visibility flags with a stable item id each, so an
/// explicit user choice survives layout-mode switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarSectionFlag {
    Actions,
    ActionsAdvanced,
    ZoomActions,
    Pages,
    Boards,
    Presets,
    StepSection,
    TextControls,
}

impl ToolbarSectionFlag {
    pub const ALL: [Self; 8] = [
        Self::Actions,
        Self::ActionsAdvanced,
        Self::ZoomActions,
        Self::Pages,
        Self::Boards,
        Self::Presets,
        Self::StepSection,
        Self::TextControls,
    ];

    /// The config item id that carries explicit overrides for this flag.
    pub fn item_id(self) -> ToolbarItemId {
        match self {
            Self::Actions => ids::SIDE_GROUP_ACTIONS,
            Self::ActionsAdvanced => ids::SIDE_GROUP_ACTIONS_ADVANCED,
            Self::ZoomActions => ids::SIDE_GROUP_ZOOM_ACTIONS,
            Self::Pages => ids::SIDE_GROUP_PAGES,
            Self::Boards => ids::SIDE_GROUP_BOARDS,
            Self::Presets => ids::SIDE_GROUP_PRESETS,
            Self::StepSection => ids::SIDE_GROUP_STEP_UNDO,
            Self::TextControls => ids::SIDE_GROUP_TEXT_CONTROLS,
        }
    }

    /// Baseline visibility for the flag under `mode`: the mode's defaults
    /// adjusted by the per-mode config overrides.
    pub fn baseline(self, mode: ToolbarLayoutMode, overrides: &ToolbarModeOverrides) -> bool {
        let defaults = mode.section_defaults();
        let over = overrides.for_mode(mode);
        match self {
            Self::Actions => over
                .show_actions_section
                .unwrap_or(defaults.show_actions_section),
            Self::ActionsAdvanced => over
                .show_actions_advanced
                .unwrap_or(defaults.show_actions_advanced),
            Self::ZoomActions => over.show_zoom_actions.unwrap_or(defaults.show_zoom_actions),
            Self::Pages => over
                .show_pages_section
                .unwrap_or(defaults.show_pages_section),
            Self::Boards => over
                .show_boards_section
                .unwrap_or(defaults.show_boards_section),
            Self::Presets => over.show_presets.unwrap_or(defaults.show_presets),
            Self::StepSection => over.show_step_section.unwrap_or(defaults.show_step_section),
            Self::TextControls => over
                .show_text_controls
                .unwrap_or(defaults.show_text_controls),
        }
    }
}

/// The flag carried by a config item id, if any.
pub fn section_flag_for_item(id: ToolbarItemId) -> Option<ToolbarSectionFlag> {
    ToolbarSectionFlag::ALL
        .into_iter()
        .find(|flag| flag.item_id() == id)
}

/// Effective values for the nine legacy section booleans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolbarSectionVisibility {
    pub show_actions_section: bool,
    pub show_actions_advanced: bool,
    pub show_zoom_actions: bool,
    pub show_pages_section: bool,
    pub show_boards_section: bool,
    pub show_presets: bool,
    pub show_step_section: bool,
    pub show_text_controls: bool,
    pub show_settings_section: bool,
}

impl ToolbarSectionVisibility {
    pub fn get(&self, flag: ToolbarSectionFlag) -> bool {
        match flag {
            ToolbarSectionFlag::Actions => self.show_actions_section,
            ToolbarSectionFlag::ActionsAdvanced => self.show_actions_advanced,
            ToolbarSectionFlag::ZoomActions => self.show_zoom_actions,
            ToolbarSectionFlag::Pages => self.show_pages_section,
            ToolbarSectionFlag::Boards => self.show_boards_section,
            ToolbarSectionFlag::Presets => self.show_presets,
            ToolbarSectionFlag::StepSection => self.show_step_section,
            ToolbarSectionFlag::TextControls => self.show_text_controls,
        }
    }
}

/// Resolve the effective section visibility: explicit item overrides win,
/// then the mode baseline.
pub fn resolve_section_visibility(
    mode: ToolbarLayoutMode,
    overrides: &ToolbarModeOverrides,
    items: &ResolvedToolbarItems,
) -> ToolbarSectionVisibility {
    let value = |flag: ToolbarSectionFlag| {
        let id = flag.item_id();
        if items.shown.contains(&id) {
            true
        } else if items.hidden.contains(&id) {
            false
        } else {
            flag.baseline(mode, overrides)
        }
    };
    ToolbarSectionVisibility {
        show_actions_section: value(ToolbarSectionFlag::Actions),
        show_actions_advanced: value(ToolbarSectionFlag::ActionsAdvanced),
        show_zoom_actions: value(ToolbarSectionFlag::ZoomActions),
        show_pages_section: value(ToolbarSectionFlag::Pages),
        show_boards_section: value(ToolbarSectionFlag::Boards),
        show_presets: value(ToolbarSectionFlag::Presets),
        show_step_section: value(ToolbarSectionFlag::StepSection),
        show_text_controls: value(ToolbarSectionFlag::TextControls),
        // Settings is navigation and the only route back to customization.
        // The serialized legacy key remains readable, but no longer hides it.
        show_settings_section: true,
    }
}

/// Record one explicit section choice in the item override store shared by
/// the overlay and configurator. Returns whether the serialized store changed.
pub fn set_section_visibility(
    items: &mut ToolbarItemsConfig,
    flag: ToolbarSectionFlag,
    visible: bool,
) -> bool {
    let before = items.clone();
    items.set_hidden(flag.item_id(), !visible);
    *items != before
}

/// Fold the legacy `show_*` booleans of an existing config into explicit
/// item overrides: wherever the legacy effective value disagrees with the
/// mode baseline and no override exists yet, record one. Ids that already
/// carry an override are left alone, so the fold is idempotent and never
/// fights a hand-edited `layout_mode`. Returns true when `items` changed.
pub fn fold_legacy_section_flags(
    legacy: &ToolbarSectionVisibility,
    mode: ToolbarLayoutMode,
    overrides: &ToolbarModeOverrides,
    items: &mut ToolbarItemsConfig,
) -> bool {
    let resolved = items.resolved();
    let mut changed = false;
    for flag in ToolbarSectionFlag::ALL {
        let id = flag.item_id();
        if resolved.shown.contains(&id) || resolved.hidden.contains(&id) {
            continue;
        }
        let legacy_value = legacy.get(flag);
        if legacy_value != flag.baseline(mode, overrides) {
            items.set_hidden(id, !legacy_value);
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_follows_mode_defaults_and_overrides() {
        let overrides = ToolbarModeOverrides::default();
        assert!(!ToolbarSectionFlag::Presets.baseline(ToolbarLayoutMode::Simple, &overrides));
        assert!(ToolbarSectionFlag::Presets.baseline(ToolbarLayoutMode::Regular, &overrides));
        assert!(
            ToolbarSectionFlag::ActionsAdvanced.baseline(ToolbarLayoutMode::Advanced, &overrides)
        );

        let mut overrides = ToolbarModeOverrides::default();
        overrides.simple.show_presets = Some(true);
        assert!(ToolbarSectionFlag::Presets.baseline(ToolbarLayoutMode::Simple, &overrides));
    }

    #[test]
    fn explicit_overrides_beat_the_baseline_in_both_directions() {
        let overrides = ToolbarModeOverrides::default();
        let mut items = ToolbarItemsConfig::default();

        // Simple hides presets by default; an explicit show wins.
        items.set_hidden(ToolbarSectionFlag::Presets.item_id(), false);
        // Regular shows zoom actions by default; an explicit hide wins.
        items.set_hidden(ToolbarSectionFlag::ZoomActions.item_id(), true);

        let resolved = items.resolved();
        let simple = resolve_section_visibility(ToolbarLayoutMode::Simple, &overrides, &resolved);
        assert!(simple.show_presets);
        assert!(!simple.show_zoom_actions);

        // The same overrides survive a mode switch untouched.
        let regular = resolve_section_visibility(ToolbarLayoutMode::Regular, &overrides, &resolved);
        assert!(regular.show_presets);
        assert!(!regular.show_zoom_actions);
    }

    #[test]
    fn fold_records_only_disagreements_and_is_idempotent() {
        let overrides = ToolbarModeOverrides::default();
        let mut items = ToolbarItemsConfig::default();
        // A legacy Regular config where the user turned zoom actions off and
        // presets stayed at the default.
        let legacy = ToolbarSectionVisibility {
            show_actions_section: true,
            show_actions_advanced: false,
            show_zoom_actions: false,
            show_pages_section: true,
            show_boards_section: true,
            show_presets: true,
            show_step_section: false,
            show_text_controls: true,
            show_settings_section: true,
        };

        let changed =
            fold_legacy_section_flags(&legacy, ToolbarLayoutMode::Regular, &overrides, &mut items);
        assert!(changed);
        let resolved = items.resolved();
        assert!(
            resolved
                .hidden
                .contains(&ToolbarSectionFlag::ZoomActions.item_id())
        );
        assert!(
            !resolved
                .hidden
                .contains(&ToolbarSectionFlag::Presets.item_id())
        );
        assert!(
            !resolved
                .shown
                .contains(&ToolbarSectionFlag::Presets.item_id())
        );

        // Effective visibility is bit-identical to the legacy booleans.
        let effective =
            resolve_section_visibility(ToolbarLayoutMode::Regular, &overrides, &resolved);
        assert_eq!(effective, legacy);

        // Running the fold again changes nothing.
        assert!(!fold_legacy_section_flags(
            &legacy,
            ToolbarLayoutMode::Regular,
            &overrides,
            &mut items
        ));
    }

    #[test]
    fn unknown_ids_survive_shown_round_trips() {
        let mut items = ToolbarItemsConfig::default();
        items.shown.push("future.mystery-id".to_string());
        items.set_hidden(ToolbarSectionFlag::Presets.item_id(), false);
        items.set_hidden(ToolbarSectionFlag::Presets.item_id(), true);
        assert!(items.shown.contains(&"future.mystery-id".to_string()));
        let resolved = items.resolved();
        assert_eq!(
            resolved.unknown_shown,
            vec!["future.mystery-id".to_string()]
        );
    }

    #[test]
    fn settings_visibility_key_is_compatibility_only() {
        let mut items = ToolbarItemsConfig::default();
        items.hidden.push(ids::SIDE_GROUP_SETTINGS.to_string());

        let visibility = resolve_section_visibility(
            ToolbarLayoutMode::Simple,
            &ToolbarModeOverrides::default(),
            &items.resolved(),
        );
        assert!(visibility.show_settings_section);
    }
}
