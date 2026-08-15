//! Pure bulk-manager view of the keybinding draft.
//!
//! Filtering, sorting, source badges, and conflict membership are computed
//! from [`KeybindingsDraft`] identity, not from widgets. Search still lives in
//! the app shell; callers intersect that visibility with this summary.

use wayscriber::config::{Action, Shortcut, ShortcutTrigger};

use super::conflicts::{
    ShortcutClaim, claimants_for, field_has_internal_duplicate, other_claimants,
};
use super::draft::KeybindingsDraft;
use super::edit::field_matches_defaults;
use super::field::KeybindingField;
use super::parse::parse_keybindings;
use crate::models::KeybindingsTabId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShortcutManagerFilter {
    #[default]
    All,
    Changed,
    Conflicts,
    Unbound,
    Device,
    Sequences,
}

impl ShortcutManagerFilter {
    pub const ALL: [Self; 6] = [
        Self::All,
        Self::Changed,
        Self::Conflicts,
        Self::Unbound,
        Self::Device,
        Self::Sequences,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Changed => "Changed",
            Self::Conflicts => "Conflicts",
            Self::Unbound => "Unbound",
            Self::Device => "Device",
            Self::Sequences => "Sequences",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShortcutManagerSort {
    #[default]
    Category,
    Name,
    Changed,
}

impl ShortcutManagerSort {
    pub const ALL: [Self; 3] = [Self::Category, Self::Name, Self::Changed];

    pub fn title(self) -> &'static str {
        match self {
            Self::Category => "Category",
            Self::Name => "Name",
            Self::Changed => "Changed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutSourceBadge {
    Default,
    Authored,
    LegacyTablet,
    Unavailable,
}

impl ShortcutSourceBadge {
    pub fn title(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Authored => "Authored",
            Self::LegacyTablet => "Legacy Tablet",
            Self::Unavailable => "Unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutRowStatus {
    Default,
    Changed,
    Unbound,
    Conflict,
    Invalid,
}

impl ShortcutRowStatus {
    pub fn title(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Changed => "Changed",
            Self::Unbound => "Unbound",
            Self::Conflict => "Conflict",
            Self::Invalid => "Invalid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutManagerRow {
    pub field: KeybindingField,
    pub label: &'static str,
    pub default_label: String,
    pub status: ShortcutRowStatus,
    pub sources: Vec<ShortcutSourceBadge>,
    pub changed: bool,
    pub unbound: bool,
    pub has_conflict: bool,
    pub has_device: bool,
    pub has_sequence: bool,
    pub has_unavailable: bool,
    pub parse_error: bool,
}

impl ShortcutManagerRow {
    pub fn badge_titles(&self) -> Vec<&'static str> {
        let mut titles = Vec::new();
        if matches!(
            self.status,
            ShortcutRowStatus::Conflict | ShortcutRowStatus::Invalid | ShortcutRowStatus::Unbound
        ) {
            titles.push(self.status.title());
        }
        for source in &self.sources {
            let title = source.title();
            if !titles.contains(&title) {
                titles.push(title);
            }
        }
        if titles.is_empty() {
            titles.push(self.status.title());
        }
        titles
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutManagerSummary {
    rows: Vec<ShortcutManagerRow>,
}

impl ShortcutManagerSummary {
    pub fn from_drafts(draft: &KeybindingsDraft, defaults: &KeybindingsDraft) -> Self {
        let contested = contested_fields(draft);
        let rows = KeybindingField::all()
            .into_iter()
            .map(|field| row_for(draft, defaults, field, contested.contains(&field)))
            .collect();
        Self { rows }
    }

    #[cfg(test)]
    pub fn rows(&self) -> &[ShortcutManagerRow] {
        &self.rows
    }

    pub fn row(&self, field: KeybindingField) -> Option<&ShortcutManagerRow> {
        self.rows.iter().find(|row| row.field == field)
    }

    pub fn has_conflicts(&self) -> bool {
        self.rows.iter().any(|row| row.has_conflict)
    }

    pub fn visible_fields(
        &self,
        filter: ShortcutManagerFilter,
        sort: ShortcutManagerSort,
        scope: Option<KeybindingsTabId>,
        search_visible: impl Fn(KeybindingField) -> bool,
    ) -> Vec<KeybindingField> {
        let mut fields: Vec<KeybindingField> = self
            .rows
            .iter()
            .filter(|row| scope.is_none_or(|tab| row.field.tab() == tab))
            .filter(|row| search_visible(row.field))
            .filter(|row| row_matches_filter(row, filter))
            .map(|row| row.field)
            .collect();
        sort_fields(&mut fields, self, sort);
        fields
    }
}

/// Exact destination/search identity: the opened config key or the action label.
pub fn field_matching_search_term(term: &str) -> Option<KeybindingField> {
    let needle = compact_term(term);
    if needle.is_empty() {
        return None;
    }
    let mut key_match = None;
    let mut label_match = None;
    for field in KeybindingField::all() {
        if compact_term(&field.field_key().replace('_', " ")) == needle {
            key_match = Some(field);
        }
        if compact_term(field.label()) == needle {
            label_match = Some(field);
        }
    }
    key_match.or(label_match)
}

pub fn next_review_conflict(
    draft: &KeybindingsDraft,
) -> Option<(
    KeybindingField,
    Shortcut,
    Vec<super::conflicts::ShortcutClaim>,
)> {
    for field in KeybindingField::all() {
        let Some(value) = draft.value_for(field) else {
            continue;
        };
        let Ok(parsed) = parse_keybindings(value) else {
            continue;
        };
        for binding in &parsed {
            let mut claimants = other_claimants(draft, field, binding);
            if claimants.is_empty() {
                let extras = parsed.iter().filter(|other| *other == binding).count();
                if extras <= 1 {
                    continue;
                }
                claimants.push(ShortcutClaim::from_field(field, binding.clone()));
            }
            return Some((field, binding.clone(), claimants));
        }
    }
    None
}

fn row_for(
    draft: &KeybindingsDraft,
    defaults: &KeybindingsDraft,
    field: KeybindingField,
    has_conflict: bool,
) -> ShortcutManagerRow {
    let value = draft.value_for(field).unwrap_or_default();
    let default_value = defaults.value_for(field).unwrap_or_default();
    let parsed = parse_keybindings(value);
    let parse_error = parsed.is_err();
    let changed = !field_matches_defaults(draft, defaults, field);
    let (unbound, has_device, has_sequence, has_unavailable) = match &parsed {
        Ok(bindings) => flags_for_bindings(bindings),
        Err(_) => (false, false, false, false),
    };
    let has_legacy = field_has_legacy_tablet(draft, field);
    let status = if parse_error {
        ShortcutRowStatus::Invalid
    } else if has_conflict {
        ShortcutRowStatus::Conflict
    } else if unbound {
        ShortcutRowStatus::Unbound
    } else if changed {
        ShortcutRowStatus::Changed
    } else {
        ShortcutRowStatus::Default
    };
    let mut sources = Vec::new();
    if changed {
        sources.push(ShortcutSourceBadge::Authored);
    } else {
        sources.push(ShortcutSourceBadge::Default);
    }
    if has_legacy {
        sources.push(ShortcutSourceBadge::LegacyTablet);
    }
    if has_unavailable {
        sources.push(ShortcutSourceBadge::Unavailable);
    }
    let default_label = if default_value.trim().is_empty() {
        "Unbound".to_string()
    } else {
        default_value.to_string()
    };
    ShortcutManagerRow {
        field,
        label: field.label(),
        default_label,
        status,
        sources,
        changed,
        unbound,
        has_conflict,
        has_device,
        has_sequence,
        has_unavailable,
        parse_error,
    }
}

fn flags_for_bindings(bindings: &[Shortcut]) -> (bool, bool, bool, bool) {
    let unbound = bindings.is_empty();
    let has_device = bindings.iter().any(is_device_shortcut);
    let has_sequence = bindings
        .iter()
        .any(|binding| matches!(binding, Shortcut::Sequence(_)));
    let has_unavailable = bindings.iter().any(|binding| !binding.is_deliverable());
    (unbound, has_device, has_sequence, has_unavailable)
}

fn is_device_shortcut(binding: &Shortcut) -> bool {
    matches!(
        binding.as_trigger(),
        Some(ShortcutTrigger::Pointer(_) | ShortcutTrigger::Stylus(_))
    )
}

fn field_has_legacy_tablet(draft: &KeybindingsDraft, field: KeybindingField) -> bool {
    let Some(action) = field.action() else {
        return false;
    };
    draft.legacy_tablet.stylus_primary == Some(action)
        || draft.legacy_tablet.stylus_secondary == Some(action)
}

fn contested_fields(draft: &KeybindingsDraft) -> Vec<KeybindingField> {
    let mut contested = Vec::new();
    let mut mark = |field: KeybindingField| {
        if !contested.contains(&field) {
            contested.push(field);
        }
    };
    for field in KeybindingField::all() {
        if field_has_internal_duplicate(draft, field) {
            mark(field);
        }
        let Some(value) = draft.value_for(field) else {
            continue;
        };
        let Ok(parsed) = parse_keybindings(value) else {
            continue;
        };
        for binding in parsed {
            if !other_claimants(draft, field, &binding).is_empty() {
                mark(field);
            }
        }
    }
    mark_legacy_claimants(draft, &mut mark);
    contested
}

fn mark_legacy_claimants(draft: &KeybindingsDraft, mark: &mut impl FnMut(KeybindingField)) {
    for (action, name) in [
        (draft.legacy_tablet.stylus_primary, "StylusPrimary"),
        (draft.legacy_tablet.stylus_secondary, "StylusSecondary"),
    ] {
        let Some(action) = action else {
            continue;
        };
        let Ok(binding) = Shortcut::parse(name) else {
            continue;
        };
        let claims = claimants_for(draft, &binding);
        if claims.len() < 2 {
            continue;
        }
        if let Some(field) = field_for_action(action) {
            mark(field);
        }
    }
}

fn field_for_action(action: Action) -> Option<KeybindingField> {
    KeybindingField::all()
        .into_iter()
        .find(|field| field.action() == Some(action))
}

fn row_matches_filter(row: &ShortcutManagerRow, filter: ShortcutManagerFilter) -> bool {
    match filter {
        ShortcutManagerFilter::All => true,
        ShortcutManagerFilter::Changed => row.changed,
        ShortcutManagerFilter::Conflicts => row.has_conflict,
        ShortcutManagerFilter::Unbound => row.unbound,
        ShortcutManagerFilter::Device => row.has_device,
        ShortcutManagerFilter::Sequences => row.has_sequence,
    }
}

fn sort_fields(
    fields: &mut [KeybindingField],
    summary: &ShortcutManagerSummary,
    sort: ShortcutManagerSort,
) {
    match sort {
        ShortcutManagerSort::Category => {}
        ShortcutManagerSort::Name => fields.sort_by_key(|field| field.label()),
        ShortcutManagerSort::Changed => fields.sort_by_key(|field| {
            let changed = summary.row(*field).is_some_and(|row| row.changed);
            (!changed, field.label())
        }),
    }
}

fn compact_term(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use wayscriber::config::keybindings::KeybindingsConfig;
    use wayscriber::config::{Action, Shortcut};

    use super::*;
    use crate::models::keybindings::{apply_recorded_replace, other_claimants};

    fn drafts() -> (KeybindingsDraft, KeybindingsDraft) {
        let defaults = KeybindingsDraft::from_config(&KeybindingsConfig::default());
        (defaults.clone(), defaults)
    }

    #[test]
    fn every_configurable_action_appears_exactly_once() {
        let (draft, defaults) = drafts();
        let summary = ShortcutManagerSummary::from_drafts(&draft, &defaults);
        let fields: Vec<_> = summary.rows().iter().map(|row| row.field).collect();
        assert_eq!(fields, KeybindingField::all());
        let mut seen = Vec::new();
        for field in fields {
            assert!(
                !seen.contains(&field),
                "{field:?} listed twice in the manager"
            );
            seen.push(field);
        }
    }

    #[test]
    fn changed_compares_parsed_values_not_whitespace() {
        let (mut draft, defaults) = drafts();
        draft.set(KeybindingField::Redo, "Ctrl+Shift+Z,Ctrl+Y".to_string());
        let summary = ShortcutManagerSummary::from_drafts(&draft, &defaults);
        let row = summary.row(KeybindingField::Redo).expect("redo row");
        assert!(!row.changed);
        assert_eq!(row.status, ShortcutRowStatus::Default);
        assert!(row.sources.contains(&ShortcutSourceBadge::Default));

        draft.set(KeybindingField::Redo, "F4".to_string());
        let summary = ShortcutManagerSummary::from_drafts(&draft, &defaults);
        let row = summary.row(KeybindingField::Redo).expect("redo row");
        assert!(row.changed);
        assert!(row.sources.contains(&ShortcutSourceBadge::Authored));
    }

    #[test]
    fn conflict_filter_includes_every_claimant() {
        let (mut draft, defaults) = drafts();
        draft.set(KeybindingField::ClearCanvas, "Ctrl+Shift+X".to_string());
        draft.set(KeybindingField::ToggleToolbar, "Ctrl+Shift+X".to_string());
        let summary = ShortcutManagerSummary::from_drafts(&draft, &defaults);
        let visible = summary.visible_fields(
            ShortcutManagerFilter::Conflicts,
            ShortcutManagerSort::Category,
            None,
            |_| true,
        );
        assert!(visible.contains(&KeybindingField::ClearCanvas));
        assert!(visible.contains(&KeybindingField::ToggleToolbar));
    }

    #[test]
    fn device_and_sequence_filters_keep_field_identity() {
        let (mut draft, defaults) = drafts();
        draft.set(KeybindingField::Undo, "MouseBack".to_string());
        draft.set(
            KeybindingField::CopySelection,
            "Ctrl+Alt+Shift+K > Ctrl+Alt+Shift+C".to_string(),
        );
        let summary = ShortcutManagerSummary::from_drafts(&draft, &defaults);
        assert_eq!(
            summary.visible_fields(
                ShortcutManagerFilter::Device,
                ShortcutManagerSort::Category,
                None,
                |_| true,
            ),
            vec![KeybindingField::Undo]
        );
        assert_eq!(
            summary.visible_fields(
                ShortcutManagerFilter::Sequences,
                ShortcutManagerSort::Category,
                None,
                |_| true,
            ),
            vec![KeybindingField::CopySelection]
        );
    }

    #[test]
    fn unbound_filter_is_empty_parsed_lists_only() {
        let (mut draft, defaults) = drafts();
        draft.set(KeybindingField::ToggleFloatingBadge, "".to_string());
        draft.set(KeybindingField::Exit, "Ctrl+Shift".to_string());
        let summary = ShortcutManagerSummary::from_drafts(&draft, &defaults);
        let visible = summary.visible_fields(
            ShortcutManagerFilter::Unbound,
            ShortcutManagerSort::Category,
            None,
            |_| true,
        );
        assert!(visible.contains(&KeybindingField::ToggleFloatingBadge));
        assert!(!visible.contains(&KeybindingField::Exit));
        assert!(
            summary
                .row(KeybindingField::Exit)
                .is_some_and(|row| row.parse_error && row.status == ShortcutRowStatus::Invalid)
        );
    }

    #[test]
    fn sort_orders_by_name_and_changed_status() {
        let (mut draft, defaults) = drafts();
        draft.set(KeybindingField::Undo, "F9".to_string());
        let summary = ShortcutManagerSummary::from_drafts(&draft, &defaults);
        let subset = |sort| {
            summary.visible_fields(ShortcutManagerFilter::All, sort, None, |field| {
                matches!(
                    field,
                    KeybindingField::ClearCanvas | KeybindingField::Undo | KeybindingField::Redo
                )
            })
        };
        assert_eq!(
            subset(ShortcutManagerSort::Name),
            vec![
                KeybindingField::ClearCanvas,
                KeybindingField::Redo,
                KeybindingField::Undo,
            ]
        );
        let changed_first = subset(ShortcutManagerSort::Changed);
        assert_eq!(changed_first[0], KeybindingField::Undo);
    }

    #[test]
    fn category_scope_keeps_the_filtered_identity_set() {
        let (mut draft, defaults) = drafts();
        draft.set(KeybindingField::Undo, "".to_string());
        draft.set(KeybindingField::ClearCanvas, "".to_string());
        let summary = ShortcutManagerSummary::from_drafts(&draft, &defaults);
        let visible = summary.visible_fields(
            ShortcutManagerFilter::Unbound,
            ShortcutManagerSort::Category,
            Some(KeybindingsTabId::History),
            |_| true,
        );
        assert!(visible.contains(&KeybindingField::Undo));
        assert!(!visible.contains(&KeybindingField::ClearCanvas));
        assert!(
            visible
                .iter()
                .all(|field| field.tab() == KeybindingsTabId::History)
        );
    }

    #[test]
    fn destination_terms_select_the_action_row() {
        assert_eq!(
            field_matching_search_term("clear canvas"),
            Some(KeybindingField::ClearCanvas)
        );
        assert_eq!(
            field_matching_search_term("Clear Canvas"),
            Some(KeybindingField::ClearCanvas)
        );
        assert_eq!(
            field_matching_search_term("undo"),
            Some(KeybindingField::Undo)
        );
        assert_eq!(field_matching_search_term("not a shortcut"), None);
    }

    #[test]
    fn source_badges_survive_explicit_legacy_migration() {
        let (mut draft, defaults) = drafts();
        draft.legacy_tablet.stylus_primary = Some(Action::ToggleRadialMenu);
        let summary = ShortcutManagerSummary::from_drafts(&draft, &defaults);
        let radial = summary
            .row(KeybindingField::ToggleRadialMenu)
            .expect("radial row");
        assert!(radial.sources.contains(&ShortcutSourceBadge::LegacyTablet));

        let binding = Shortcut::parse("StylusPrimary").expect("parses");
        let claimants = other_claimants(&draft, KeybindingField::Undo, &binding);
        apply_recorded_replace(&mut draft, KeybindingField::Undo, &binding, &claimants)
            .expect("move");
        let summary = ShortcutManagerSummary::from_drafts(&draft, &defaults);
        let radial = summary
            .row(KeybindingField::ToggleRadialMenu)
            .expect("radial row");
        assert!(!radial.sources.contains(&ShortcutSourceBadge::LegacyTablet));
        let undo = summary.row(KeybindingField::Undo).expect("undo row");
        assert!(undo.has_device);
        assert!(undo.sources.contains(&ShortcutSourceBadge::Authored));
    }

    #[test]
    fn unavailable_badge_marks_undeliverable_keys() {
        let (mut draft, defaults) = drafts();
        draft.set(KeybindingField::Exit, "Escpae".to_string());
        let summary = ShortcutManagerSummary::from_drafts(&draft, &defaults);
        let row = summary.row(KeybindingField::Exit).expect("exit row");
        assert!(row.has_unavailable);
        assert!(row.sources.contains(&ShortcutSourceBadge::Unavailable));
        assert!(row.changed);
    }

    #[test]
    fn next_review_conflict_names_the_other_claimant() {
        let (mut draft, _defaults) = drafts();
        draft.set(KeybindingField::ClearCanvas, "Ctrl+Shift+X".to_string());
        draft.set(KeybindingField::ToggleToolbar, "Ctrl+Shift+X".to_string());
        let (field, binding, claimants) = next_review_conflict(&draft).expect("conflict");
        assert_eq!(binding.to_string(), "Ctrl+Shift+X");
        assert!(claimants.iter().any(|claim| claim.field == Some(field)
            || matches!(
                claim.field,
                Some(KeybindingField::ClearCanvas | KeybindingField::ToggleToolbar)
            )));
        assert!(field == KeybindingField::ClearCanvas || field == KeybindingField::ToggleToolbar);
    }

    #[test]
    fn next_review_conflict_includes_same_field_duplicates() {
        let (mut draft, _defaults) = drafts();
        draft.set(KeybindingField::ClearCanvas, "E, e".to_string());
        let (field, binding, claimants) = next_review_conflict(&draft).expect("duplicate");
        assert_eq!(field, KeybindingField::ClearCanvas);
        assert_eq!(binding.to_string(), "E");
        assert!(
            claimants
                .iter()
                .any(|claim| claim.field == Some(KeybindingField::ClearCanvas))
        );
    }
}
