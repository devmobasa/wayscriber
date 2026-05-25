use std::collections::HashSet;

use super::Config;
use crate::config::RenderProfileExportMode;
use crate::render_profiles::{format_hex_rgb, normalize_profile_id, parse_hex_rgb};
use log::warn;

impl Config {
    pub(super) fn validate_render_profiles(&mut self) {
        let mut seen_ids = HashSet::new();
        for (index, profile) in self.render_profiles.profiles.iter_mut().enumerate() {
            let raw_id = profile.id.clone();
            let mut id = normalize_profile_id(&raw_id);
            if id.is_empty() {
                id = format!("profile-{}", index + 1);
                warn!("Render profile id was empty; using '{}'", id);
            } else if id != raw_id.trim() {
                warn!("Render profile id '{}' normalized to '{}'", raw_id, id);
            }

            let base = id.clone();
            let mut suffix = 2;
            while seen_ids.contains(&id) {
                id = format!("{base}-{suffix}");
                suffix += 1;
            }
            if id != base {
                warn!("Render profile id '{}' deduplicated to '{}'", base, id);
            }
            seen_ids.insert(id.clone());
            profile.id = id;

            if profile.name.trim().is_empty() {
                profile.name = format!("Profile {}", index + 1);
                warn!(
                    "Render profile '{}' had empty name; using '{}'",
                    profile.id, profile.name
                );
            } else {
                profile.name = profile.name.trim().to_string();
            }

            let mut seen_sources = HashSet::new();
            let mut normalized = Vec::with_capacity(profile.mappings.len());
            for mapping in profile.mappings.iter().rev() {
                let Some(from) = parse_hex_rgb(&mapping.from) else {
                    warn!(
                        "Render profile '{}' has invalid source color '{}'; dropping mapping",
                        profile.id, mapping.from
                    );
                    continue;
                };
                let Some(to) = parse_hex_rgb(&mapping.to) else {
                    warn!(
                        "Render profile '{}' has invalid target color '{}'; dropping mapping",
                        profile.id, mapping.to
                    );
                    continue;
                };
                if !seen_sources.insert(from) {
                    warn!(
                        "Render profile '{}' has duplicate source color {}; keeping the last mapping",
                        profile.id,
                        format_hex_rgb(from)
                    );
                    continue;
                }
                normalized.push(crate::config::RenderColorMappingConfig {
                    from: format_hex_rgb(from),
                    to: format_hex_rgb(to),
                });
            }
            normalized.reverse();
            profile.mappings = normalized;
        }

        if let Some(active) = self.render_profiles.active.as_mut() {
            *active = normalize_profile_id(active);
            if active.is_empty()
                || !self
                    .render_profiles
                    .profiles
                    .iter()
                    .any(|profile| profile.id == *active)
            {
                warn!(
                    "Active render profile '{}' not found; starting with render profiles off",
                    active
                );
                self.render_profiles.active = None;
            }
        }

        if matches!(
            self.render_profiles.export,
            RenderProfileExportMode::Profile
        ) {
            if let Some(export_profile) = self.render_profiles.export_profile.as_mut() {
                *export_profile = normalize_profile_id(export_profile);
                if export_profile.is_empty()
                    || !self
                        .render_profiles
                        .profiles
                        .iter()
                        .any(|profile| profile.id == *export_profile)
                {
                    warn!(
                        "Export render profile '{}' not found; disabling canvas export remapping",
                        export_profile
                    );
                    self.render_profiles.export = RenderProfileExportMode::Off;
                    self.render_profiles.export_profile = None;
                }
            } else {
                warn!(
                    "render_profiles.export is profile but export_profile is empty; disabling export remapping"
                );
                self.render_profiles.export = RenderProfileExportMode::Off;
            }
        } else if let Some(export_profile) = self.render_profiles.export_profile.as_mut() {
            *export_profile = normalize_profile_id(export_profile);
            if export_profile.is_empty() {
                self.render_profiles.export_profile = None;
            }
        }
    }
}
