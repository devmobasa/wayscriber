use wayscriber::config::{
    Config, RenderColorMappingConfig, RenderProfileConfig, RenderProfileExportMode,
};
use wayscriber::render_profiles::{format_hex_rgb, normalize_profile_id, parse_hex_rgb};

use super::super::error::FormError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderProfileTextField {
    Id,
    Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderProfileMappingSide {
    From,
    To,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderProfileExportOption {
    Off,
    Active,
    Profile,
}

impl RenderProfileExportOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Off, Self::Active, Self::Profile]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Active => "Active profile",
            Self::Profile => "Named profile",
        }
    }

    fn from_mode(mode: RenderProfileExportMode) -> Self {
        match mode {
            RenderProfileExportMode::Off => Self::Off,
            RenderProfileExportMode::Active => Self::Active,
            RenderProfileExportMode::Profile => Self::Profile,
        }
    }

    fn to_mode(self) -> RenderProfileExportMode {
        match self {
            Self::Off => RenderProfileExportMode::Off,
            Self::Active => RenderProfileExportMode::Active,
            Self::Profile => RenderProfileExportMode::Profile,
        }
    }
}

impl std::fmt::Display for RenderProfileExportOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderProfileSelectionOption {
    Off,
    Profile(String),
}

impl RenderProfileSelectionOption {
    pub fn list(profile_ids: &[String]) -> Vec<Self> {
        std::iter::once(Self::Off)
            .chain(profile_ids.iter().cloned().map(Self::Profile))
            .collect()
    }

    pub fn from_active(value: &str, profile_ids: &[String]) -> Self {
        if !value.is_empty() && profile_ids.iter().any(|id| id == value) {
            Self::Profile(value.to_string())
        } else {
            Self::Off
        }
    }

    pub fn profile_id(self) -> String {
        match self {
            Self::Off => String::new(),
            Self::Profile(id) => id,
        }
    }
}

impl std::fmt::Display for RenderProfileSelectionOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => f.write_str("Off"),
            Self::Profile(id) => f.write_str(id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderProfileMappingDraft {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderProfileDraft {
    pub id: String,
    pub name: String,
    pub mappings: Vec<RenderProfileMappingDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderProfilesDraft {
    pub active: String,
    pub apply_to_canvas: bool,
    pub apply_to_ui: bool,
    pub export: RenderProfileExportOption,
    pub export_profile: String,
    pub profiles: Vec<RenderProfileDraft>,
}

impl RenderProfilesDraft {
    pub fn from_config(config: &Config) -> Self {
        Self {
            active: config.render_profiles.active.clone().unwrap_or_default(),
            apply_to_canvas: config.render_profiles.apply_to_canvas,
            apply_to_ui: config.render_profiles.apply_to_ui,
            export: RenderProfileExportOption::from_mode(config.render_profiles.export),
            export_profile: config
                .render_profiles
                .export_profile
                .clone()
                .unwrap_or_default(),
            profiles: config
                .render_profiles
                .profiles
                .iter()
                .map(|profile| RenderProfileDraft {
                    id: profile.id.clone(),
                    name: profile.name.clone(),
                    mappings: profile
                        .mappings
                        .iter()
                        .map(|mapping| RenderProfileMappingDraft {
                            from: mapping.from.clone(),
                            to: mapping.to.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    pub fn apply_to_config(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        config.render_profiles.active = non_empty_normalized(&self.active);
        config.render_profiles.apply_to_canvas = self.apply_to_canvas;
        config.render_profiles.apply_to_ui = self.apply_to_ui;
        config.render_profiles.export = self.export.to_mode();
        config.render_profiles.export_profile = non_empty_normalized(&self.export_profile);
        config.render_profiles.profiles = self
            .profiles
            .iter()
            .enumerate()
            .map(|(profile_index, profile)| RenderProfileConfig {
                id: profile.id.clone(),
                name: profile.name.clone(),
                mappings: profile
                    .mappings
                    .iter()
                    .enumerate()
                    .filter_map(|(mapping_index, mapping)| {
                        let from = normalized_hex(
                            &mapping.from,
                            profile_index,
                            mapping_index,
                            RenderProfileMappingSide::From,
                            errors,
                        )?;
                        let to = normalized_hex(
                            &mapping.to,
                            profile_index,
                            mapping_index,
                            RenderProfileMappingSide::To,
                            errors,
                        )?;
                        Some(RenderColorMappingConfig { from, to })
                    })
                    .collect(),
            })
            .collect();
    }

    pub fn profile_ids(&self) -> Vec<String> {
        self.profiles
            .iter()
            .enumerate()
            .map(|(index, profile)| effective_profile_id(&profile.id, index))
            .collect()
    }

    pub fn ensure_selections_exist(&mut self) {
        let ids = self.profile_ids();
        if !self.active.is_empty() && !ids.contains(&self.active) {
            self.active.clear();
        }
        if !self.export_profile.is_empty() && !ids.contains(&self.export_profile) {
            self.export_profile.clear();
            if self.export == RenderProfileExportOption::Profile {
                self.export = RenderProfileExportOption::Off;
            }
        }
    }

    pub fn new_profile(&self) -> RenderProfileDraft {
        let next = self.profiles.len() + 1;
        RenderProfileDraft {
            id: self.next_profile_id(),
            name: format!("Profile {next}"),
            mappings: vec![RenderProfileMappingDraft {
                from: "#000000".to_string(),
                to: "#FFFFFF".to_string(),
            }],
        }
    }

    pub fn duplicate_profile(&self, index: usize) -> Option<RenderProfileDraft> {
        let mut duplicate = self.profiles.get(index)?.clone();
        duplicate.id = self.next_profile_id();
        if !duplicate.name.trim().is_empty() {
            duplicate.name = format!("{} Copy", duplicate.name.trim());
        }
        Some(duplicate)
    }

    fn next_profile_id(&self) -> String {
        let existing = self.profile_ids();
        let mut index = self.profiles.len() + 1;
        loop {
            let id = format!("profile-{index}");
            if !existing.contains(&id) {
                return id;
            }
            index += 1;
        }
    }
}

fn effective_profile_id(value: &str, index: usize) -> String {
    let normalized = normalize_profile_id(value);
    if normalized.is_empty() {
        format!("profile-{}", index + 1)
    } else {
        normalized
    }
}

fn non_empty_normalized(value: &str) -> Option<String> {
    let normalized = normalize_profile_id(value);
    (!normalized.is_empty()).then_some(normalized)
}

fn normalized_hex(
    value: &str,
    profile_index: usize,
    mapping_index: usize,
    side: RenderProfileMappingSide,
    errors: &mut Vec<FormError>,
) -> Option<String> {
    let field = format!(
        "render_profiles.profiles[{profile_index}].mappings[{mapping_index}].{}",
        match side {
            RenderProfileMappingSide::From => "from",
            RenderProfileMappingSide::To => "to",
        }
    );
    match parse_hex_rgb(value) {
        Some(color) => Some(format_hex_rgb(color)),
        None => {
            errors.push(FormError::new(field, "Expected #RRGGBB hex color"));
            None
        }
    }
}
