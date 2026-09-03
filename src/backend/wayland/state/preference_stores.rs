use crate::{
    backend::wayland::{
        RuntimeWakeHandle,
        config_edits::ConfigEditWorker,
        runtime_ui_state::{ToolbarRuntimeState, UnavailablePersistencePreviews},
    },
    onboarding::OnboardingStore,
    palette_recents::PaletteRecentsWriter,
    ui::toolbar::RuntimeUiPersistenceSnapshot,
};

/// Runtime state for persisted UI preferences and degraded-mode previews.
pub(in crate::backend::wayland) struct RuntimeUiSlot {
    state: Option<ToolbarRuntimeState>,
    unavailable: Option<RuntimeUiPersistenceSnapshot>,
    unavailable_previews: UnavailablePersistencePreviews,
}

impl RuntimeUiSlot {
    fn new(
        state: Option<ToolbarRuntimeState>,
        unavailable: Option<RuntimeUiPersistenceSnapshot>,
    ) -> Self {
        Self {
            state,
            unavailable,
            unavailable_previews: UnavailablePersistencePreviews::default(),
        }
    }

    pub(in crate::backend::wayland) fn state(&self) -> Option<&ToolbarRuntimeState> {
        self.state.as_ref()
    }

    pub(in crate::backend::wayland) fn state_mut(&mut self) -> Option<&mut ToolbarRuntimeState> {
        self.state.as_mut()
    }

    pub(in crate::backend::wayland) fn unavailable(&self) -> Option<&RuntimeUiPersistenceSnapshot> {
        self.unavailable.as_ref()
    }

    pub(in crate::backend::wayland) fn unavailable_previews(
        &self,
    ) -> &UnavailablePersistencePreviews {
        &self.unavailable_previews
    }

    pub(in crate::backend::wayland) fn unavailable_previews_mut(
        &mut self,
    ) -> &mut UnavailablePersistencePreviews {
        &mut self.unavailable_previews
    }
}

/// Persistence workers and stores whose lifetimes match the Wayland runtime.
pub(in crate::backend::wayland) struct PreferenceStores {
    onboarding: OnboardingStore,
    palette_recents: PaletteRecentsWriter,
    config_edits: ConfigEditWorker,
    runtime_ui: RuntimeUiSlot,
}

impl PreferenceStores {
    pub(in crate::backend::wayland) fn new(
        onboarding: OnboardingStore,
        palette_recents: PaletteRecentsWriter,
        runtime_ui: Option<ToolbarRuntimeState>,
        runtime_ui_unavailable: Option<RuntimeUiPersistenceSnapshot>,
        wake: RuntimeWakeHandle,
    ) -> Self {
        Self {
            onboarding,
            palette_recents,
            config_edits: ConfigEditWorker::new(wake),
            runtime_ui: RuntimeUiSlot::new(runtime_ui, runtime_ui_unavailable),
        }
    }

    pub(in crate::backend::wayland) fn onboarding(&self) -> &OnboardingStore {
        &self.onboarding
    }

    pub(in crate::backend::wayland) fn onboarding_mut(&mut self) -> &mut OnboardingStore {
        &mut self.onboarding
    }

    pub(in crate::backend::wayland) fn palette_recents_mut(&mut self) -> &mut PaletteRecentsWriter {
        &mut self.palette_recents
    }

    pub(in crate::backend::wayland) fn config_edits_mut(&mut self) -> &mut ConfigEditWorker {
        &mut self.config_edits
    }

    pub(in crate::backend::wayland) fn runtime_ui(&self) -> &RuntimeUiSlot {
        &self.runtime_ui
    }

    pub(in crate::backend::wayland) fn runtime_ui_mut(&mut self) -> &mut RuntimeUiSlot {
        &mut self.runtime_ui
    }
}
