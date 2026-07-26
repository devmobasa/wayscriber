//! Background persistence for small runtime-authored config preferences.
//!
//! The Wayland dispatch thread only queues typed mutations. A single worker
//! batches nearby edits, reloads the latest config document, and performs the
//! durable atomic write so an fsync cannot delay input feedback.

use crate::config::{
    Config, ConfigDocument, ConfigStore, QuickColorWrite, StatusBarItem, ToolPresetConfig,
    ToolbarItemId, ToolbarItemVisibilitySetting, ToolbarLayoutMode, ToolbarSectionFlag,
    ToolbarSectionVisibility, TopDisplayMode,
};
use crate::draw::Color;
use crate::input::boards::PendingBoardConfigUpdate;
use anyhow::Result;
use log::{debug, warn};
#[cfg(test)]
use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const WRITE_DEBOUNCE: Duration = Duration::from_millis(75);
const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(250);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub(in crate::backend::wayland) enum ConfigMutation {
    ToolbarLayout {
        mode: ToolbarLayoutMode,
        sections: ToolbarSectionVisibility,
    },
    ToolbarSectionVisibility {
        id: ToolbarItemId,
        setting: ToolbarItemVisibilitySetting,
        flag: ToolbarSectionFlag,
        visible: bool,
    },
    ToolbarTopDisplayMode(TopDisplayMode),
    ToolbarUseIcons(bool),
    ToolbarShowMoreColors(bool),
    ToolbarContextAwareUi(bool),
    ToolbarPresetToasts(bool),
    ToolbarToolPreview(bool),
    ToolbarDelaySliders(bool),
    ToolbarTopPosition {
        x: f64,
        y: f64,
    },
    ToolbarSidePosition {
        top_x: f64,
        side_x: f64,
        side_y: f64,
    },
    ShowStatusBar(bool),
    StatusBarInteractive(bool),
    StatusBarItem {
        item: StatusBarItem,
        visible: bool,
    },
    StatusBoardBadge(bool),
    StatusPageBadge(bool),
    FloatingBadgeAlways(bool),
    FloatingBadge(bool),
    ZoomChip(bool),
    HistoryCustomSection(bool),
    ClickHighlight {
        enabled: Option<bool>,
        show_on_highlight_tool: bool,
    },
    BoardConfig(Box<PendingBoardConfigUpdate>),
    PresetSlot {
        slot: usize,
        preset: Option<Box<ToolPresetConfig>>,
    },
    QuickColor {
        index: usize,
        color: Color,
    },
}

impl ConfigMutation {
    /// Apply one typed edit to a loaded config. A false return means the
    /// mutation's externally editable target disappeared before persistence.
    pub(in crate::backend::wayland) fn apply(&self, config: &mut Config) -> bool {
        match self {
            Self::ToolbarLayout { mode, sections } => {
                config.ui.toolbar.layout_mode = *mode;
                apply_section_visibility(config, *sections);
            }
            Self::ToolbarSectionVisibility {
                id,
                setting,
                flag,
                visible,
            } => {
                config
                    .ui
                    .toolbar
                    .items
                    .set_visibility_setting(*id, *setting);
                apply_section_compatibility_mirror(config, *flag, *visible);
            }
            Self::ToolbarTopDisplayMode(mode) => config.ui.toolbar.top_display_mode = *mode,
            Self::ToolbarUseIcons(value) => config.ui.toolbar.use_icons = *value,
            Self::ToolbarShowMoreColors(value) => config.ui.toolbar.show_more_colors = *value,
            Self::ToolbarContextAwareUi(value) => config.ui.toolbar.context_aware_ui = *value,
            Self::ToolbarPresetToasts(value) => config.ui.toolbar.show_preset_toasts = *value,
            Self::ToolbarToolPreview(value) => config.ui.toolbar.show_tool_preview = *value,
            Self::ToolbarDelaySliders(value) => config.ui.toolbar.show_delay_sliders = *value,
            Self::ToolbarTopPosition { x, y } => {
                config.ui.toolbar.top_offset = *x;
                config.ui.toolbar.top_offset_y = *y;
            }
            Self::ToolbarSidePosition {
                top_x,
                side_x,
                side_y,
            } => {
                config.ui.toolbar.top_offset = *top_x;
                config.ui.toolbar.side_offset_x = *side_x;
                config.ui.toolbar.side_offset = *side_y;
            }
            Self::ShowStatusBar(value) => config.ui.show_status_bar = *value,
            Self::StatusBarInteractive(value) => config.ui.status_bar_interactive = *value,
            Self::StatusBarItem { item, visible } => {
                config.ui.set_status_bar_item_visible(*item, *visible);
            }
            Self::StatusBoardBadge(value) => config.ui.show_status_board_badge = *value,
            Self::StatusPageBadge(value) => config.ui.show_status_page_badge = *value,
            Self::FloatingBadgeAlways(value) => config.ui.show_floating_badge_always = *value,
            Self::FloatingBadge(value) => config.ui.show_floating_badge = *value,
            Self::ZoomChip(value) => config.ui.toolbar.show_zoom_chip = *value,
            Self::HistoryCustomSection(value) => config.history.custom_section_enabled = *value,
            Self::ClickHighlight {
                enabled,
                show_on_highlight_tool,
            } => {
                if let Some(enabled) = enabled {
                    config.ui.click_highlight.enabled = *enabled;
                }
                config.ui.click_highlight.show_on_highlight_tool = *show_on_highlight_tool;
            }
            Self::BoardConfig(update) => {
                crate::backend::wayland::state::apply_board_config_update_to_config(
                    config,
                    update.as_ref().clone(),
                );
            }
            Self::PresetSlot { slot, preset } => {
                config.presets.set_slot(*slot, preset.as_deref().cloned());
            }
            Self::QuickColor { index, color } => {
                return !matches!(
                    config.drawing.quick_colors.set_color_at(*index, *color),
                    QuickColorWrite::SlotMissing
                );
            }
        }
        true
    }

    fn key(&self) -> Option<ConfigMutationKey> {
        let key = match *self {
            Self::ToolbarLayout { .. } => ConfigMutationKey::ToolbarLayout,
            Self::ToolbarSectionVisibility { id, .. } => {
                ConfigMutationKey::ToolbarSectionVisibility(id)
            }
            Self::ToolbarTopDisplayMode(_) => ConfigMutationKey::ToolbarTopDisplayMode,
            Self::ToolbarUseIcons(_) => ConfigMutationKey::ToolbarUseIcons,
            Self::ToolbarShowMoreColors(_) => ConfigMutationKey::ToolbarShowMoreColors,
            Self::ToolbarContextAwareUi(_) => ConfigMutationKey::ToolbarContextAwareUi,
            Self::ToolbarPresetToasts(_) => ConfigMutationKey::ToolbarPresetToasts,
            Self::ToolbarToolPreview(_) => ConfigMutationKey::ToolbarToolPreview,
            Self::ToolbarDelaySliders(_) => ConfigMutationKey::ToolbarDelaySliders,
            Self::ToolbarTopPosition { .. } => ConfigMutationKey::ToolbarTopPosition,
            Self::ToolbarSidePosition { .. } => ConfigMutationKey::ToolbarSidePosition,
            Self::ShowStatusBar(_) => ConfigMutationKey::ShowStatusBar,
            Self::StatusBarInteractive(_) => ConfigMutationKey::StatusBarInteractive,
            Self::StatusBarItem { item, .. } => ConfigMutationKey::StatusBarItem(item),
            Self::StatusBoardBadge(_) => ConfigMutationKey::StatusBoardBadge,
            Self::StatusPageBadge(_) => ConfigMutationKey::StatusPageBadge,
            Self::FloatingBadgeAlways(_) => ConfigMutationKey::FloatingBadgeAlways,
            Self::FloatingBadge(_) => ConfigMutationKey::FloatingBadge,
            Self::ZoomChip(_) => ConfigMutationKey::ZoomChip,
            Self::HistoryCustomSection(_) => ConfigMutationKey::HistoryCustomSection,
            // `enabled: None` deliberately leaves one field untouched, so
            // replacing an earlier request could discard that field's edit.
            Self::ClickHighlight { .. } => return None,
            // Board updates carry merge metadata and must remain ordered.
            Self::BoardConfig(_) => return None,
            Self::PresetSlot { slot, .. } => ConfigMutationKey::PresetSlot(slot),
            Self::QuickColor { index, .. } => ConfigMutationKey::QuickColor(index),
        };
        Some(key)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConfigMutationKey {
    ToolbarLayout,
    ToolbarSectionVisibility(ToolbarItemId),
    ToolbarTopDisplayMode,
    ToolbarUseIcons,
    ToolbarShowMoreColors,
    ToolbarContextAwareUi,
    ToolbarPresetToasts,
    ToolbarToolPreview,
    ToolbarDelaySliders,
    ToolbarTopPosition,
    ToolbarSidePosition,
    ShowStatusBar,
    StatusBarInteractive,
    StatusBarItem(StatusBarItem),
    StatusBoardBadge,
    StatusPageBadge,
    FloatingBadgeAlways,
    FloatingBadge,
    ZoomChip,
    HistoryCustomSection,
    PresetSlot(usize),
    QuickColor(usize),
}

fn apply_section_visibility(config: &mut Config, sections: ToolbarSectionVisibility) {
    config.ui.toolbar.show_actions_section = sections.show_actions_section;
    config.ui.toolbar.show_actions_advanced = sections.show_actions_advanced;
    config.ui.toolbar.show_zoom_actions = sections.show_zoom_actions;
    config.ui.toolbar.show_pages_section = sections.show_pages_section;
    config.ui.toolbar.show_boards_section = sections.show_boards_section;
    config.ui.toolbar.show_presets = sections.show_presets;
    config.ui.toolbar.show_step_section = sections.show_step_section;
    config.ui.toolbar.show_text_controls = sections.show_text_controls;
    config.ui.toolbar.show_settings_section = sections.show_settings_section;
}

fn apply_section_compatibility_mirror(
    config: &mut Config,
    flag: ToolbarSectionFlag,
    visible: bool,
) {
    match flag {
        ToolbarSectionFlag::Actions => config.ui.toolbar.show_actions_section = visible,
        ToolbarSectionFlag::ActionsAdvanced => {
            config.ui.toolbar.show_actions_advanced = visible;
        }
        ToolbarSectionFlag::ZoomActions => config.ui.toolbar.show_zoom_actions = visible,
        ToolbarSectionFlag::Pages => config.ui.toolbar.show_pages_section = visible,
        ToolbarSectionFlag::Boards => config.ui.toolbar.show_boards_section = visible,
        ToolbarSectionFlag::Presets => config.ui.toolbar.show_presets = visible,
        ToolbarSectionFlag::StepSection => config.ui.toolbar.show_step_section = visible,
        ToolbarSectionFlag::TextControls => config.ui.toolbar.show_text_controls = visible,
    }
}

enum WriterCommand {
    Apply(ConfigMutation),
    Shutdown,
}

type PersistMutations = Box<dyn FnMut(&[ConfigMutation]) -> Result<()> + Send>;

/// Event-loop facade for the channel-owned config writer.
pub(in crate::backend::wayland) struct ConfigWriter {
    sender: Option<Sender<WriterCommand>>,
    worker: Option<JoinHandle<()>>,
}

impl ConfigWriter {
    pub(in crate::backend::wayland) fn new(config_store: ConfigStore) -> Self {
        Self::for_store(config_store)
    }

    fn for_store(config_store: ConfigStore) -> Self {
        Self::spawn(Box::new(move |mutations| {
            persist_mutations(&config_store, mutations)
        }))
    }

    fn spawn(persist: PersistMutations) -> Self {
        let (sender, receiver) = channel();
        let worker = thread::Builder::new()
            .name("wayscriber-config-writer".to_string())
            .spawn(move || run_writer(receiver, persist));

        match worker {
            Ok(worker) => Self {
                sender: Some(sender),
                worker: Some(worker),
            },
            Err(error) => {
                warn!("Failed to start runtime config writer: {error}");
                Self::unavailable()
            }
        }
    }

    fn unavailable() -> Self {
        Self {
            sender: None,
            worker: None,
        }
    }

    /// Queue a mutation without doing filesystem work on the caller.
    #[must_use = "a false return means the preference was not queued"]
    pub(in crate::backend::wayland) fn request(&self, mutation: &ConfigMutation) -> bool {
        self.sender
            .as_ref()
            .is_some_and(|sender| sender.send(WriterCommand::Apply(mutation.clone())).is_ok())
    }

    /// Flush queued mutations and wait for the writer to finish.
    pub(in crate::backend::wayland) fn shutdown(&mut self) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(WriterCommand::Shutdown);
        }
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            warn!("Runtime config writer thread panicked");
        }
    }
}

impl Drop for ConfigWriter {
    fn drop(&mut self) {
        self.shutdown();
    }
}

enum WorkerEvent {
    Command(WriterCommand),
    Timeout,
    Disconnected,
}

fn receive_worker_event(
    receiver: &Receiver<WriterCommand>,
    timeout: Option<Duration>,
) -> WorkerEvent {
    match timeout {
        Some(timeout) => match receiver.recv_timeout(timeout) {
            Ok(command) => WorkerEvent::Command(command),
            Err(RecvTimeoutError::Timeout) => WorkerEvent::Timeout,
            Err(RecvTimeoutError::Disconnected) => WorkerEvent::Disconnected,
        },
        None => match receiver.recv() {
            Ok(command) => WorkerEvent::Command(command),
            Err(_) => WorkerEvent::Disconnected,
        },
    }
}

fn run_writer(receiver: Receiver<WriterCommand>, mut persist: PersistMutations) {
    let mut pending = Vec::new();
    let mut write_after = None;
    let mut retry_delay = INITIAL_RETRY_DELAY;

    loop {
        match receive_worker_event(&receiver, write_after) {
            WorkerEvent::Command(WriterCommand::Apply(mutation)) => {
                if let Some(key) = mutation.key() {
                    pending.retain(|queued: &ConfigMutation| queued.key() != Some(key));
                }
                pending.push(mutation);
                write_after = Some(WRITE_DEBOUNCE);
            }
            WorkerEvent::Command(WriterCommand::Shutdown) | WorkerEvent::Disconnected => {
                persist_before_shutdown(&mut persist, &pending);
                return;
            }
            WorkerEvent::Timeout => match persist(&pending) {
                Ok(()) => {
                    debug!("Processed {} runtime config edit(s)", pending.len());
                    pending.clear();
                    write_after = None;
                    retry_delay = INITIAL_RETRY_DELAY;
                }
                Err(error) => {
                    warn!(
                        "Failed to persist {} runtime config edit(s); retrying: {error:#}",
                        pending.len()
                    );
                    write_after = Some(retry_delay);
                    retry_delay = retry_delay.saturating_mul(2).min(MAX_RETRY_DELAY);
                }
            },
        }
    }
}

fn persist_before_shutdown(persist: &mut PersistMutations, pending: &[ConfigMutation]) {
    if pending.is_empty() {
        return;
    }
    match persist(pending) {
        Ok(()) => debug!(
            "Processed {} runtime config edit(s) during shutdown",
            pending.len()
        ),
        Err(error) => warn!(
            "Failed to persist {} runtime config edit(s) during shutdown: {error:#}",
            pending.len()
        ),
    }
}

fn persist_mutations(config_store: &ConfigStore, mutations: &[ConfigMutation]) -> Result<()> {
    let document = ConfigDocument::load_from_path(config_store.config_path())?;
    let mut config = document.config().clone();
    let mut applied = false;
    for mutation in mutations {
        if mutation.apply(&mut config) {
            applied = true;
        } else if let ConfigMutation::QuickColor { index, .. } = mutation {
            warn!("Quick color slot {index} is no longer in config.toml; recolor was not saved");
        }
    }
    if applied {
        document.save(config)?;
    }
    Ok(())
}

#[cfg(test)]
fn persist_mutations_to_path(path: &Path, mutations: &[ConfigMutation]) -> Result<()> {
    persist_mutations(&ConfigStore::at_path(path), mutations)
}

#[cfg(test)]
mod tests;
