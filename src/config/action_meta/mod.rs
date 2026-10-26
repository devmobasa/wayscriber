use crate::i18n;

use super::Action;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionCategory {
    Core,
    Drawing,
    Tools,
    Colors,
    UI,
    Board,
    Zoom,
    Capture,
    Selection,
    History,
    Presets,
}

/// Semantic glyph identity; rendering owners choose the Cairo painter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionIcon {
    Text,
    StickyNote,
    Select,
    Pen,
    Line,
    Rect,
    Ellipse,
    FreeformPolygon,
    Arrow,
    Blur,
    Marker,
    StepMarker,
    Eraser,
    Undo,
}

#[derive(Debug, Clone, Copy)]
pub struct ActionMeta {
    pub action: Action,
    pub label: &'static str,
    pub short_label: Option<&'static str>,
    pub description: &'static str,
    pub search_aliases: &'static [&'static str],
    #[allow(dead_code)]
    pub category: ActionCategory,
    pub in_command_palette: bool,
    pub in_help: bool,
    pub in_toolbar: bool,
    /// Semantic icon for the radial menu and command palette.
    pub icon: Option<ActionIcon>,
}

impl ActionMeta {
    #[must_use]
    pub fn short_label(self) -> &'static str {
        self.short_label.unwrap_or(self.label)
    }
}

macro_rules! meta {
    (
        $action:ident,
        $label:expr,
        $short:expr,
        $desc:expr,
        $category:ident,
        $in_palette:expr,
        $in_help:expr,
        $in_toolbar:expr,
        $aliases:expr,icon:
        $icon:expr
    ) => {
        ActionMeta {
            action: crate::config::Action::$action,
            label: $label,
            short_label: $short,
            description: $desc,
            search_aliases: $aliases,
            category: crate::config::action_meta::ActionCategory::$category,
            in_command_palette: $in_palette,
            in_help: $in_help,
            in_toolbar: $in_toolbar,
            icon: Some($icon),
        }
    };
    (
        $action:ident,
        $label:expr,
        $short:expr,
        $desc:expr,
        $category:ident,
        $in_palette:expr,
        $in_help:expr,
        $in_toolbar:expr,icon:
        $icon:expr
    ) => {
        ActionMeta {
            action: crate::config::Action::$action,
            label: $label,
            short_label: $short,
            description: $desc,
            search_aliases: &[],
            category: crate::config::action_meta::ActionCategory::$category,
            in_command_palette: $in_palette,
            in_help: $in_help,
            in_toolbar: $in_toolbar,
            icon: Some($icon),
        }
    };
    (
        $action:ident,
        $label:expr,
        $short:expr,
        $desc:expr,
        $category:ident,
        $in_palette:expr,
        $in_help:expr,
        $in_toolbar:expr,
        $aliases:expr
    ) => {
        ActionMeta {
            action: crate::config::Action::$action,
            label: $label,
            short_label: $short,
            description: $desc,
            search_aliases: $aliases,
            category: crate::config::action_meta::ActionCategory::$category,
            in_command_palette: $in_palette,
            in_help: $in_help,
            in_toolbar: $in_toolbar,
            icon: None,
        }
    };
    (
        $action:ident,
        $label:expr,
        $short:expr,
        $desc:expr,
        $category:ident,
        $in_palette:expr,
        $in_help:expr,
        $in_toolbar:expr
    ) => {
        ActionMeta {
            action: crate::config::Action::$action,
            label: $label,
            short_label: $short,
            description: $desc,
            search_aliases: &[],
            category: crate::config::action_meta::ActionCategory::$category,
            in_command_palette: $in_palette,
            in_help: $in_help,
            in_toolbar: $in_toolbar,
            icon: None,
        }
    };
}

mod entries;

const ACTION_META_SECTIONS: &[&[ActionMeta]] = &[
    entries::core::ENTRIES,
    entries::history::ENTRIES,
    entries::tools::ENTRIES,
    entries::drawing::ENTRIES,
    entries::board::ENTRIES,
    entries::ui::ENTRIES,
    entries::colors::ENTRIES,
    entries::capture::ENTRIES,
    entries::zoom::ENTRIES,
    entries::selection::ENTRIES,
    entries::presets::ENTRIES,
];

pub fn action_meta_iter() -> impl Iterator<Item = &'static ActionMeta> {
    ACTION_META_SECTIONS
        .iter()
        .flat_map(|entries| entries.iter())
}

#[must_use]
pub fn action_meta(action: Action) -> Option<&'static ActionMeta> {
    action_meta_iter().find(|meta| meta.action == action)
}

/// The untranslated label from the metadata table, falling back to a word for
/// actions that have no entry.
fn source_label(action: Action) -> &'static str {
    action_meta(action).map_or("Action", |meta| meta.label)
}

#[must_use]
pub fn action_label(action: Action) -> &'static str {
    i18n::tr(source_label(action))
}

#[must_use]
pub fn action_short_label(action: Action) -> &'static str {
    i18n::tr(action_meta(action).map_or("Action", |meta| meta.short_label()))
}

/// The compact spelling used where a full label is too wide.
///
/// Derivation happens on the English source label so the suffixes this UI
/// abbreviates stay English; only the result is translated.
#[must_use]
pub fn action_display_label(action: Action) -> &'static str {
    if matches!(action, Action::SelectEllipseTool) {
        return action_short_label(action);
    }
    let label = source_label(action);
    if let Some(stripped) = label.strip_suffix(" Tool") {
        return i18n::tr(stripped);
    }
    if let Some(stripped) = label.strip_suffix(" Mode") {
        return i18n::tr(stripped);
    }
    if let Some(stripped) = label.strip_prefix("Toggle ") {
        return i18n::tr(stripped);
    }
    action_label(action)
}

#[allow(dead_code)]
#[must_use]
pub fn action_description(action: Action) -> &'static str {
    i18n::tr(action_meta(action).map_or("", |meta| meta.description))
}

#[cfg(test)]
mod tests;
