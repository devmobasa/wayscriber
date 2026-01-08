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

#[derive(Debug, Clone, Copy)]
pub struct ActionMeta {
    pub action: Action,
    pub label: &'static str,
    pub short_label: Option<&'static str>,
    pub description: &'static str,
    #[allow(dead_code)]
    pub category: ActionCategory,
    pub in_command_palette: bool,
    pub in_help: bool,
    pub in_toolbar: bool,
}

impl ActionMeta {
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
        $in_toolbar:expr
    ) => {
        ActionMeta {
            action: crate::config::Action::$action,
            label: $label,
            short_label: $short,
            description: $desc,
            category: crate::config::action_meta::ActionCategory::$category,
            in_command_palette: $in_palette,
            in_help: $in_help,
            in_toolbar: $in_toolbar,
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

pub fn action_meta(action: Action) -> Option<&'static ActionMeta> {
    action_meta_iter().find(|meta| meta.action == action)
}

pub fn action_label(action: Action) -> &'static str {
    action_meta(action)
        .map(|meta| meta.label)
        .unwrap_or("Action")
}

pub fn action_short_label(action: Action) -> &'static str {
    action_meta(action)
        .map(|meta| meta.short_label())
        .unwrap_or("Action")
}

pub fn action_display_label(action: Action) -> &'static str {
    if matches!(action, Action::SelectEllipseTool) {
        return action_short_label(action);
    }
    let label = action_label(action);
    if let Some(stripped) = label.strip_suffix(" Tool") {
        return stripped;
    }
    if let Some(stripped) = label.strip_suffix(" Mode") {
        return stripped;
    }
    if let Some(stripped) = label.strip_prefix("Toggle ") {
        return stripped;
    }
    label
}

#[allow(dead_code)]
pub fn action_description(action: Action) -> &'static str {
    action_meta(action)
        .map(|meta| meta.description)
        .unwrap_or("")
}

#[cfg(test)]
mod tests;
