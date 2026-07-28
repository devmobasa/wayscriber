//! Background persistence for durable overlay config edits.
//!
//! Nothing durable is left to persist: authored preferences are current-run
//! changes to the effective config, and the editor/content flows (boards,
//! presets, quick colors, shortcuts) are owned by the configurator. The
//! machinery is kept for one phase so the deletion of the writer, its wiring,
//! and the runtime config backup lands as a single change; `ConfigMutation`
//! has no variants, so no batch can ever form.

use crate::config::{Config, ConfigDocument, RuntimeConfigBackup};
use anyhow::Result;
use log::{debug, warn};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const WRITE_DEBOUNCE: Duration = Duration::from_millis(75);
const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(250);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

/// A durable edit the overlay owns.
///
/// Uninhabited: every family that used to travel through here now applies to
/// the current run or opens the configurator instead, so no value of this type
/// can be constructed and the worker below can never receive one.
#[derive(Debug, Clone)]
pub(in crate::backend::wayland) enum ConfigMutation {}

impl ConfigMutation {
    /// Apply one typed edit to a loaded config.
    ///
    /// Matching an uninhabited value takes no arms, which is how the compiler
    /// is told this body cannot run — no panic and no placeholder behaviour.
    fn apply(&self, _config: &mut Config) {
        match *self {}
    }
}

enum WriterCommand {
    /// Unconstructible while `ConfigMutation` has no variants. Kept so the
    /// queue, batching, and durable write stay one unit for Phase G to remove.
    #[allow(dead_code)]
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

fn persist_mutations_to_path(
    path: &Path,
    mutations: &[ConfigMutation],
    backup: &mut RuntimeConfigBackup,
) -> Result<()> {
    let document = ConfigDocument::load_from_path(path)?;
    let mut config = document.config().clone();
    for mutation in mutations {
        mutation.apply(&mut config);
    }
    // A batch that changed nothing must not rewrite the file byte-identically
    // and spend the process's one backup snapshot on it.
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
