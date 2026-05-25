use iced::Task;

use crate::messages::Message;
use crate::models::{
    ColorPickerId, RenderProfileExportOption, RenderProfileMappingDraft, RenderProfileMappingSide,
    RenderProfileTextField,
};

use super::super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn handle_render_profile_add(&mut self) -> Task<Message> {
        self.status = StatusMessage::idle();
        let profile = self.draft.render_profiles.new_profile();
        self.draft.render_profiles.profiles.push(profile);
        self.sync_render_profile_color_picker_hex();
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_render_profile_remove(&mut self, index: usize) -> Task<Message> {
        self.status = StatusMessage::idle();
        if index < self.draft.render_profiles.profiles.len() {
            self.draft.render_profiles.profiles.remove(index);
            self.clear_render_profile_color_pickers();
            self.draft.render_profiles.ensure_selections_exist();
            self.refresh_dirty_flag();
        }
        Task::none()
    }

    pub(super) fn handle_render_profile_duplicate(&mut self, index: usize) -> Task<Message> {
        self.status = StatusMessage::idle();
        if let Some(profile) = self.draft.render_profiles.duplicate_profile(index) {
            self.draft
                .render_profiles
                .profiles
                .insert(index + 1, profile);
            self.clear_render_profile_color_pickers();
            self.refresh_dirty_flag();
        }
        Task::none()
    }

    pub(super) fn handle_render_profile_text_changed(
        &mut self,
        index: usize,
        field: RenderProfileTextField,
        value: String,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        let old_id = self
            .draft
            .render_profiles
            .profile_ids()
            .get(index)
            .cloned()
            .unwrap_or_default();
        if let Some(profile) = self.draft.render_profiles.profiles.get_mut(index) {
            match field {
                RenderProfileTextField::Id => {
                    let next_id = wayscriber::render_profiles::normalize_profile_id(&value);
                    profile.id = value;
                    if self.draft.render_profiles.active == old_id {
                        self.draft.render_profiles.active = next_id.clone();
                    }
                    if self.draft.render_profiles.export_profile == old_id {
                        self.draft.render_profiles.export_profile = next_id;
                    }
                }
                RenderProfileTextField::Name => profile.name = value,
            }
        }
        self.draft.render_profiles.ensure_selections_exist();
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_render_profile_active_changed(&mut self, value: String) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.render_profiles.active = value;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_render_profile_export_changed(
        &mut self,
        value: RenderProfileExportOption,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.render_profiles.export = value;
        if value == RenderProfileExportOption::Profile
            && self.draft.render_profiles.export_profile.is_empty()
            && let Some(first) = self.draft.render_profiles.profile_ids().first()
        {
            self.draft.render_profiles.export_profile = first.clone();
        }
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_render_profile_export_profile_changed(
        &mut self,
        value: String,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.render_profiles.export_profile = value;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_render_profile_apply_canvas_changed(
        &mut self,
        value: bool,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.render_profiles.apply_to_canvas = value;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_render_profile_apply_ui_changed(&mut self, value: bool) -> Task<Message> {
        self.status = StatusMessage::idle();
        self.draft.render_profiles.apply_to_ui = value;
        self.refresh_dirty_flag();
        Task::none()
    }

    pub(super) fn handle_render_profile_mapping_add(
        &mut self,
        profile_index: usize,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        if let Some(profile) = self.draft.render_profiles.profiles.get_mut(profile_index) {
            profile.mappings.push(RenderProfileMappingDraft {
                from: "#000000".to_string(),
                to: "#FFFFFF".to_string(),
            });
            self.sync_render_profile_color_picker_hex();
            self.refresh_dirty_flag();
        }
        Task::none()
    }

    pub(super) fn handle_render_profile_mapping_remove(
        &mut self,
        profile_index: usize,
        mapping_index: usize,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        if let Some(profile) = self.draft.render_profiles.profiles.get_mut(profile_index)
            && mapping_index < profile.mappings.len()
        {
            profile.mappings.remove(mapping_index);
            self.clear_render_profile_color_pickers();
            self.refresh_dirty_flag();
        }
        Task::none()
    }

    pub(super) fn handle_render_profile_mapping_color_changed(
        &mut self,
        profile_index: usize,
        mapping_index: usize,
        side: RenderProfileMappingSide,
        value: String,
    ) -> Task<Message> {
        self.status = StatusMessage::idle();
        if let Some(mapping) = self
            .draft
            .render_profiles
            .profiles
            .get_mut(profile_index)
            .and_then(|profile| profile.mappings.get_mut(mapping_index))
        {
            let picker_id = match side {
                RenderProfileMappingSide::From => {
                    ColorPickerId::RenderProfileMappingFrom(profile_index, mapping_index)
                }
                RenderProfileMappingSide::To => {
                    ColorPickerId::RenderProfileMappingTo(profile_index, mapping_index)
                }
            };
            self.color_picker_hex.insert(picker_id, value.clone());
            match side {
                RenderProfileMappingSide::From => mapping.from = value,
                RenderProfileMappingSide::To => mapping.to = value,
            }
            self.refresh_dirty_flag();
        }
        Task::none()
    }

    fn clear_render_profile_color_pickers(&mut self) {
        if matches!(
            self.color_picker_open,
            Some(
                ColorPickerId::RenderProfileMappingFrom(_, _)
                    | ColorPickerId::RenderProfileMappingTo(_, _)
            )
        ) {
            self.color_picker_open = None;
        }
        self.color_picker_hex.retain(|id, _| {
            !matches!(
                id,
                ColorPickerId::RenderProfileMappingFrom(_, _)
                    | ColorPickerId::RenderProfileMappingTo(_, _)
            )
        });
        self.color_picker_advanced.retain(|id| {
            !matches!(
                id,
                ColorPickerId::RenderProfileMappingFrom(_, _)
                    | ColorPickerId::RenderProfileMappingTo(_, _)
            )
        });
        self.sync_render_profile_color_picker_hex();
    }
}
