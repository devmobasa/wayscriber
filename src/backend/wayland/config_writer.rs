//! Background persistence for the overlay's remaining durable config edits.
//!
//! Authored UI preferences no longer come through here: an overlay toggle is
//! a current-run change to the effective config. What is left are the
//! editor/content flows (boards, presets, quick colors, shortcuts).
//!
//! The Wayland dispatch thread only queues typed mutations. A single worker
//! batches nearby edits, reloads the latest config document, and performs the
//! durable atomic write so an fsync cannot delay input feedback.

use crate::config::{
    Action, Config, ConfigDocument, QuickColorWrite, RuntimeConfigBackup, ToolPresetConfig,
};
use crate::draw::Color;
use crate::input::boards::PendingBoardConfigUpdate;
use anyhow::Result;
use log::{debug, warn};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const WRITE_DEBOUNCE: Duration = Duration::from_millis(75);
const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(250);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

/// One durable edit the overlay still owns.
///
/// Authored UI preferences are not here: an overlay toggle changes the
/// effective config for the current run and never reaches `config.toml`.
/// What remains are the editor/content flows (boards, presets, palette,
/// shortcuts) that still promise a durable edit.
#[derive(Debug, Clone)]
pub(in crate::backend::wayland) enum ConfigMutation {
    BoardConfig(Box<PendingBoardConfigUpdate>),
    PresetSlot {
        slot: usize,
        preset: Option<Box<ToolPresetConfig>>,
    },
    QuickColor {
        index: usize,
        color: Color,
    },
    /// One action's complete `[keybindings]` entry, as the overlay's shortcut
    /// editor merged it. The editor owns conflict detection and validation
    /// before queueing; this carries only the field that survived that check.
    Keybinding {
        action: Action,
        bindings: Vec<String>,
        receipt: ConfigWriteReceipt,
    },
}

/// Identity the event-loop state assigns to one accepted shortcut edit.
///
/// The writer returns it only after the batch is durable, allowing the live
/// keymap to stop replaying that edit over later on-disk changes without
/// sharing state with the worker thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) struct ConfigWriteReceipt(u64);

impl ConfigWriteReceipt {
    pub(in crate::backend::wayland) const fn initial() -> Self {
        Self(0)
    }

    pub(in crate::backend::wayland) fn successor(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

impl ConfigMutation {
    /// Apply one typed edit to a loaded config. A false return means the
    /// mutation's externally editable target disappeared before persistence.
    pub(in crate::backend::wayland) fn apply(&self, config: &mut Config) -> bool {
        match self {
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
            Self::Keybinding {
                action, bindings, ..
            } => {
                // Runtime-only actions have no stored field. The editor
                // refuses them before queueing, so this only keeps an
                // impossible request from forcing a no-op write.
                return config
                    .keybindings
                    .set_bindings_for_action(*action, bindings.clone())
                    .is_ok();
            }
        }
        true
    }

    /// Whether persisting this edit also moves a runtime-UI seed.
    ///
    /// `runtime_seeds_from_config` derives its seeds from the toolbar item
    /// resolution — which folds `layout_mode`, the legacy section flags, and
    /// `ui.toolbar.items` — and from the configured board pins. An edit to any
    /// of those must refresh the seed registry, or override semantics keep
    /// comparing against the pre-edit baseline until an unrelated event
    /// refreshes them. Spelled out per variant on purpose: a new mutation has
    /// to classify itself rather than inherit a wildcard. (The toolbar
    /// families that used to answer `true` here no longer travel through the
    /// writer at all; they reseed from their apply path instead.)
    pub(in crate::backend::wayland) fn affects_runtime_ui_seeds(&self) -> bool {
        match self {
            Self::BoardConfig(_) => true,
            Self::PresetSlot { .. } | Self::QuickColor { .. } | Self::Keybinding { .. } => false,
        }
    }

    fn key(&self) -> Option<ConfigMutationKey> {
        let key = match *self {
            // Board updates carry merge metadata and must remain ordered.
            Self::BoardConfig(_) => return None,
            Self::PresetSlot { slot, .. } => ConfigMutationKey::PresetSlot(slot),
            Self::QuickColor { index, .. } => ConfigMutationKey::QuickColor(index),
            // Per action: re-editing one shortcut replaces the pending value,
            // while a different action's edit keeps its own entry instead of
            // clobbering the one already queued.
            Self::Keybinding { action, .. } => ConfigMutationKey::Keybinding(action),
        };
        Some(key)
    }

    fn keybinding_receipt(&self) -> Option<ConfigWriteReceipt> {
        match self {
            Self::Keybinding { receipt, .. } => Some(*receipt),
            Self::BoardConfig(_) | Self::PresetSlot { .. } | Self::QuickColor { .. } => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConfigMutationKey {
    PresetSlot(usize),
    QuickColor(usize),
    Keybinding(Action),
}

enum WriterCommand {
    Apply(ConfigMutation),
    Shutdown,
}

type PersistMutations = Box<dyn FnMut(&[ConfigMutation]) -> Result<()> + Send>;

/// Event-loop facade for the channel-owned config writer.
pub(in crate::backend::wayland) struct ConfigWriter {
    sender: Option<Sender<WriterCommand>>,
    completed_keybindings: Receiver<ConfigWriteReceipt>,
    worker: Option<JoinHandle<()>>,
}

impl ConfigWriter {
    pub(in crate::backend::wayland) fn new() -> Self {
        match Config::get_config_path() {
            // The worker owns the overlay process's single config backup: it
            // is the only thing here that rewrites `config.toml`, and one
            // writer is built per overlay, so the guard's lifetime is the
            // process's.
            Ok(path) => Self::for_path(path, RuntimeConfigBackup::new()),
            Err(error) => {
                warn!("Runtime config persistence is unavailable: {error:#}");
                Self::unavailable()
            }
        }
    }

    fn for_path(path: PathBuf, mut backup: RuntimeConfigBackup) -> Self {
        Self::spawn(Box::new(move |mutations| {
            persist_mutations_to_path(&path, mutations, &mut backup)
        }))
    }

    fn spawn(persist: PersistMutations) -> Self {
        let (sender, receiver) = channel();
        let (completion_sender, completed_keybindings) = channel();
        let worker = thread::Builder::new()
            .name("wayscriber-config-writer".to_string())
            .spawn(move || run_writer(receiver, completion_sender, persist));

        match worker {
            Ok(worker) => Self {
                sender: Some(sender),
                completed_keybindings,
                worker: Some(worker),
            },
            Err(error) => {
                warn!("Failed to start runtime config writer: {error}");
                Self::unavailable()
            }
        }
    }

    fn unavailable() -> Self {
        let (_completion_sender, completed_keybindings) = channel();
        Self {
            sender: None,
            completed_keybindings,
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

    /// Drain shortcut edits whose batches have reached durable storage.
    pub(in crate::backend::wayland) fn take_completed_keybinding_writes(
        &self,
    ) -> Vec<ConfigWriteReceipt> {
        self.completed_keybindings.try_iter().collect()
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

fn run_writer(
    receiver: Receiver<WriterCommand>,
    completion_sender: Sender<ConfigWriteReceipt>,
    mut persist: PersistMutations,
) {
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
                persist_before_shutdown(&mut persist, &pending, &completion_sender);
                return;
            }
            WorkerEvent::Timeout => match persist(&pending) {
                Ok(()) => {
                    debug!("Processed {} runtime config edit(s)", pending.len());
                    acknowledge_keybinding_writes(&completion_sender, &pending);
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

fn acknowledge_keybinding_writes(
    completion_sender: &Sender<ConfigWriteReceipt>,
    pending: &[ConfigMutation],
) {
    for receipt in pending
        .iter()
        .filter_map(ConfigMutation::keybinding_receipt)
    {
        if completion_sender.send(receipt).is_err() {
            return;
        }
    }
}

fn persist_before_shutdown(
    persist: &mut PersistMutations,
    pending: &[ConfigMutation],
    completion_sender: &Sender<ConfigWriteReceipt>,
) {
    if pending.is_empty() {
        return;
    }
    match persist(pending) {
        Ok(()) => {
            debug!(
                "Processed {} runtime config edit(s) during shutdown",
                pending.len()
            );
            acknowledge_keybinding_writes(completion_sender, pending);
        }
        Err(error) => warn!(
            "Failed to persist {} runtime config edit(s) during shutdown: {error:#}",
            pending.len()
        ),
    }
}

pub(in crate::backend::wayland) fn persist_mutations_to_path(
    path: &Path,
    mutations: &[ConfigMutation],
    backup: &mut RuntimeConfigBackup,
) -> Result<()> {
    let document = ConfigDocument::load_from_path(path)?;
    let mut config = document.config().clone();
    for mutation in mutations {
        if mutation.apply(&mut config) {
            continue;
        }
        if let ConfigMutation::QuickColor { index, .. } = mutation {
            warn!("Quick color slot {index} is no longer in config.toml; recolor was not saved");
        } else if let ConfigMutation::Keybinding { action, .. } = mutation {
            warn!("{action:?} has no configurable keybinding; the shortcut was not saved");
        }
    }
    // `apply` reports that it stored a value, not that the value differed, so
    // a toolbar toggled back to where it started would otherwise rewrite the
    // file byte-identically and spend the process's one backup snapshot on it.
    // Returning early is still a completed batch: the caller acknowledges a
    // shortcut edit's receipt on `Ok(())`, not on a write having happened, so
    // the editor stops replaying it exactly as if the save had run.
    if config_matches(document.config(), &config) {
        debug!("Runtime config batch changed nothing; leaving config.toml untouched");
        return Ok(());
    }
    // Taken here, after the batch is known to change something, so the copy is
    // of the file as this session found it rather than as an earlier no-op left
    // it.
    backup.ensure_snapshot(path);
    document.save(config)?;
    Ok(())
}

/// Whether two configs would produce the same file.
///
/// `Config` has no `PartialEq`: the derive would have to reach every type in a
/// tree several dozen structs wide, for one comparison. The document merge
/// already measures its diff on the serialized form, so comparing that form
/// asks the save's own question. A config that cannot be serialized cannot be
/// compared either, and is reported by the save rather than skipped here.
fn config_matches(previous: &Config, updated: &Config) -> bool {
    match (
        toml::to_string_pretty(previous),
        toml::to_string_pretty(updated),
    ) {
        (Ok(previous), Ok(updated)) => previous == updated,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
