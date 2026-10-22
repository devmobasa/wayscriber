use std::collections::hash_map::DefaultHasher;

use crate::config::{QuickColorPalette, RadialMenuMouseBinding};
use crate::input::InputState;
use std::hash::{Hash, Hasher};

use super::sections::{HelpContentOptions, HelpOverlayBindings, SectionSets, build_section_sets};

#[derive(Clone, PartialEq)]
struct HelpContentKey {
    shortcut_revision: u64,
    radial_menu_mouse_binding: RadialMenuMouseBinding,
    quick_colors: QuickColorPalette,
    options: HelpContentOptions,
}

impl HelpContentKey {
    fn from_input(input: &InputState, options: HelpContentOptions) -> Self {
        Self {
            shortcut_revision: input.keymap_revision(),
            radial_menu_mouse_binding: input.radial_menu.mouse_binding(),
            quick_colors: input.style.quick_colors.clone(),
            options,
        }
    }
}

pub(crate) struct HelpContentSnapshot {
    pub(super) revision: u64,
    pub(super) bindings: HelpOverlayBindings,
    pub(super) sections: SectionSets,
}

impl HelpContentSnapshot {
    pub(super) fn from_bindings(
        bindings: &HelpOverlayBindings,
        options: HelpContentOptions,
    ) -> Self {
        let mut hasher = DefaultHasher::new();
        bindings.cache_key().hash(&mut hasher);
        options.hash(&mut hasher);
        Self {
            revision: hasher.finish(),
            bindings: bindings.clone(),
            sections: build_section_sets(bindings, options),
        }
    }
}

struct CachedHelpContent {
    key: HelpContentKey,
    snapshot: HelpContentSnapshot,
}

/// Owner-scoped help content assembled from the canonical runtime shortcut
/// snapshot and the runtime capabilities that decide which rows exist.
#[derive(Default)]
pub(crate) struct HelpContentCache {
    entry: Option<CachedHelpContent>,
    next_revision: u64,
    #[cfg(test)]
    builds: usize,
}

impl HelpContentCache {
    pub(crate) fn get_or_build(
        &mut self,
        input: &InputState,
        options: HelpContentOptions,
    ) -> &HelpContentSnapshot {
        let key = HelpContentKey::from_input(input, options);
        let rebuild = self.entry.as_ref().is_none_or(|entry| entry.key != key);
        if rebuild {
            let bindings = HelpOverlayBindings::from_input_state(input);
            let sections = build_section_sets(&bindings, options);
            self.next_revision = self.next_revision.wrapping_add(1);
            self.entry = Some(CachedHelpContent {
                key,
                snapshot: HelpContentSnapshot {
                    revision: self.next_revision,
                    bindings,
                    sections,
                },
            });
            #[cfg(test)]
            {
                self.builds += 1;
            }
        }
        &self
            .entry
            .as_ref()
            .expect("help content was built")
            .snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Action, Shortcut};
    use crate::input::state::test_support::make_test_input_state;
    use std::collections::HashMap;

    #[test]
    fn content_rebuilds_only_when_its_canonical_inputs_change() {
        let mut cache = HelpContentCache::default();
        let mut input = make_test_input_state();

        let single_board = HelpContentOptions {
            frozen_enabled: true,
            context_filter: true,
            board_enabled: false,
            capture_enabled: true,
        };
        let first = cache.get_or_build(&input, single_board).revision;
        assert_eq!(cache.get_or_build(&input, single_board).revision, first);
        assert_eq!(cache.builds, 1);

        input.help_overlay.open(false);
        input.help_overlay.next_page();
        input.help_overlay.insert_search("zoom");
        input.help_overlay.scroll_by(12.0);
        assert_eq!(
            cache.get_or_build(&input, single_board).revision,
            first,
            "page, search, and scroll are display state, not content inputs"
        );
        assert_eq!(cache.builds, 1);

        input.set_action_bindings(HashMap::from([(
            Action::ToggleHelp,
            vec![Shortcut::parse("F10").unwrap()],
        )]));
        let rebound = cache.get_or_build(&input, single_board).revision;
        assert_ne!(rebound, first);
        assert_eq!(cache.builds, 2);

        let capability_content = cache.get_or_build(
            &input,
            HelpContentOptions {
                board_enabled: true,
                ..single_board
            },
        );
        let capability_change = capability_content.revision;
        assert!(
            capability_content
                .sections
                .all
                .iter()
                .any(|section| section.title == "Boards")
        );
        assert_ne!(capability_change, rebound);
        assert_eq!(cache.builds, 3);
    }
}
