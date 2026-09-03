//! The overlay's durable config edits, off the dispatch thread.
//!
//! Three gestures write `config.toml`: a shortcut rebind, a preset slot, and a
//! quick-color swatch. Each write parses the file, copies it to a timestamped
//! `.bak`, renames a new one into place, and fsyncs the file and its directory —
//! tens of milliseconds on a healthy disk and unbounded on a sick one. The
//! thread that would otherwise do that work is the one dispatching input and
//! painting, so it hands the write to a worker and picks the answer up later.
//!
//! **Nothing is installed on the way out.** A shortcut edit hands the worker a
//! delta — one action and the bindings it should end up with — and the run keeps
//! the keymap it has until the write reports back. That is what preserves the
//! refusal the write is there to make: a chord the file has given to another
//! action since this run read it is not a degraded save but a rejected edit, and
//! the only honest way to reject it is to never have installed it.
//! `prepare_keybinding_edit` takes the running keymap by shared reference, so
//! no version of this path can install one early. A delta rather than a keymap
//! matters for the other direction too: writes are serialized here, so two edits
//! can be outstanding at once, and a second one installing a whole keymap
//! prepared before the first landed would silently revert it.
//!
//! Presets and quick colors keep their own semantics: the live slot or swatch
//! changes as the user releases the control, because that is the feedback the
//! gesture is for, and only the *wording* waits for the write — a toast that
//! claimed a durable change before the file had one would be the lie worth
//! avoiding.
//!
//! Edits are executed one at a time in submission order, and answered in that
//! same order. Order is not a nicety here: the completions are what install a
//! shortcut, settle a preset slot's wording, and decide each toast, so a
//! completion that overtakes an older one hands the user the wrong answer to the
//! wrong gesture — and, for two gestures on one slot, leaves the *older* value
//! in place while the newer one's toast says it was saved.
//!
//! That is why nothing is ever completed on the spot. A submission joins a
//! staging queue on this side of the worker's channel and is pumped into it as
//! the worker makes room, so an edit the channel had no space for stays behind
//! the edits that arrived before it instead of jumping to the front as an
//! instant failure. The queue is bounded only by the burst that fills it; what
//! is bounded is the worker's own channel, and a full one means the filesystem
//! has stopped answering, not that an edit may be dropped.
//!
//! Teardown (`finish_config_edits`) is part of the same promise. The overlay can
//! be told to exit in the same batch of input events that carried a gesture, and
//! the loop breaks before the pass that would have queued it — so quitting
//! drains the pending gestures one last time, hands the staging queue over, and
//! waits a bounded five seconds for the writes. Only the wording is given up:
//! there is no overlay left to toast on.
//!
//! A bound is a bound, though, and a filesystem that has stopped answering
//! reaches it. Two things happen there, and neither of them is a claim that the
//! edits landed. The worker is not stopped: it goes on writing everything
//! already accepted for as long as the process lives, whether or not anything is
//! still listening for the answers — the wording was always the disposable half,
//! and an answer nobody can hear is no reason to drop a write nobody can
//! reconstruct. And teardown says what it does not know, naming each edit it
//! never heard back about as possibly unsaved rather than reporting it as left
//! to finish. Process exit can still stop the in-flight write and discard every
//! queued edit behind it. The in-flight write is lost whole — writes rename a
//! finished temp file into place — so the file is either that edit or what it
//! said before, never half of either.

use super::RuntimeWakeHandle;
use super::state::{
    WaylandState, queue_keybinding_edit, queue_preset_action, queue_quick_color_edit,
};
use crate::config::{
    Action, Config, ConfigEditOutcome, action_label, persist_keybinding_edit, persist_preset_slot,
    persist_quick_color,
};
use crate::input::state::{
    InputEffect, InputEffectDrain, InputState, KeybindingEditRequest, PresetAction, QuickColorEdit,
};
use anyhow::{Result, anyhow};
use log::{error, info, warn};
use std::collections::VecDeque;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// How many edits the worker's channel holds behind the one being written.
///
/// These arrive one deliberate gesture at a time and each takes milliseconds, so
/// the channel exists to absorb a burst of corrections, not a backlog. Filling
/// it means the filesystem has stopped answering; the edits that find it full
/// wait in the staging queue and go in as it drains, in the order they were
/// made.
const EDIT_QUEUE_CAPACITY: usize = 8;

/// How long teardown waits for queued writes to land.
///
/// Long enough for a write that is merely slow, short enough that a filesystem
/// which has stopped answering cannot hold the overlay open.
const SHUTDOWN_DRAIN: Duration = Duration::from_secs(5);

/// A shortcut edit that has been accepted but not yet installed.
///
/// It carries a delta — one action and the binding list it should end up with —
/// and nothing else. Not a replacement keymap: two of these can be in flight at
/// once, and a whole-keymap snapshot taken when the second was accepted would
/// undo the first as it installed. The completion folds the delta into whatever
/// keymap the run holds by then.
#[derive(Debug)]
pub(in crate::backend::wayland) struct KeybindingEditWrite {
    pub(in crate::backend::wayland) request: KeybindingEditRequest,
    /// The list the file is asked to hold for this action, and the same list
    /// the run's keymap takes if the file accepts it.
    pub(in crate::backend::wayland) bindings: Vec<String>,
}

/// A shortcut delta the file has been asked for and has not answered yet.
///
/// Nothing is installed until a write reports back, so the running keymap is a
/// picture of the file as it was *before* any queued edit — and a second edit
/// checked against it is checked against bindings the first one has already
/// asked to move. The chord an outstanding edit freed reads as taken, and the
/// gesture that reaches for it is refused over a claim that is on its way out.
///
/// These are what the check reads instead: the run's keymap with the edits
/// still in flight folded in, in the order they were submitted, which is the
/// keymap the completions are going to leave behind.
#[derive(Debug, Clone)]
pub(in crate::backend::wayland) struct ProjectedShortcut {
    pub(in crate::backend::wayland) action: Action,
    /// The list the file is being asked to hold, which is the list the run's
    /// keymap takes if the file accepts it.
    pub(in crate::backend::wayland) bindings: Vec<String>,
}

/// One explicit user edit, on its way to `config.toml`.
#[derive(Debug)]
pub(in crate::backend::wayland) enum ConfigEdit {
    Keybinding(KeybindingEditWrite),
    Preset(PresetAction),
    QuickColor(QuickColorEdit),
}

impl ConfigEdit {
    /// Which gesture this edit is, in the words a log line can name it by.
    ///
    /// The edit itself goes into the worker's channel and stops being this
    /// side's to describe; this is what stays behind. Teardown that runs out of
    /// time has to tell the user which gestures to check, and "some config
    /// edits" is not something anybody can act on — a slot, a swatch, or an
    /// action is.
    fn identity(&self) -> ConfigEditIdentity {
        match self {
            Self::Keybinding(write) => ConfigEditIdentity::Shortcut(write.request.action),
            Self::Preset(PresetAction::Save { slot, .. } | PresetAction::Clear { slot }) => {
                ConfigEditIdentity::PresetSlot(*slot)
            }
            Self::QuickColor(edit) => ConfigEditIdentity::QuickColorIndex(edit.index),
        }
    }
}

/// What one edit is about, without the edit.
///
/// `Copy` and three words wide, so keeping one per outstanding edit costs
/// nothing per gesture and nothing is borrowed from the edit the worker owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum ConfigEditIdentity {
    Shortcut(Action),
    PresetSlot(usize),
    /// Zero-based in the model; formatted as the one-based slot the UI names.
    QuickColorIndex(usize),
}

impl fmt::Display for ConfigEditIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shortcut(action) => {
                write!(formatter, "the shortcut for {}", action_label(*action))
            }
            Self::PresetSlot(slot) => write!(formatter, "preset slot {slot}"),
            Self::QuickColorIndex(index) => {
                write!(formatter, "quick color slot {}", index.saturating_add(1))
            }
        }
    }
}

/// The edits teardown could not account for, in the order they were made.
///
/// Returned as well as logged. The warning is what the user reads, and this is
/// the same enumeration as a value, so the suite can assert on what teardown
/// knows rather than on how it prints it.
#[derive(Debug, Default, PartialEq, Eq)]
pub(in crate::backend::wayland) struct ConfigEditShutdownReport {
    /// Edits the worker took and did not answer for inside teardown's bound.
    ///
    /// Each of these may or may not be in the file: the worker goes on writing
    /// them while the process lives, and process exit can prevent every one
    /// still outstanding from landing.
    pub(in crate::backend::wayland) unconfirmed: Vec<ConfigEditIdentity>,
    /// Edits no worker ever took, which is the one thing teardown can say
    /// plainly: these are not in the file.
    pub(in crate::backend::wayland) unwritten: Vec<ConfigEditIdentity>,
}

/// An edit and what the file did with it.
#[derive(Debug)]
pub(in crate::backend::wayland) struct ConfigEditCompletion {
    pub(in crate::backend::wayland) edit: ConfigEdit,
    pub(in crate::backend::wayland) result: Result<ConfigEditOutcome>,
}

/// The event loop's end of the config-edit worker.
///
/// The thread is spawned on the first edit. Most runs never make one, and a run
/// that does makes a handful, so the cost of the worker is paid by the gesture
/// that needs it rather than by every overlay start.
#[derive(Debug)]
pub(in crate::backend::wayland) struct ConfigEditWorker {
    wake: RuntimeWakeHandle,
    running: Option<RunningWorker>,
    /// Every submission, oldest first, until the worker's channel has room for
    /// it.
    ///
    /// This is what makes "in submission order" survive a full channel. The
    /// alternative — handing the newest edit back as a failed write on the spot
    /// — answers it *before* the older edits it was made after, and their
    /// completions then apply on top of it: the newest gesture is the one that
    /// disappears, with a toast of its own saying otherwise.
    staged: VecDeque<ConfigEdit>,
    /// Every shortcut delta submitted and not yet answered, oldest first —
    /// wherever it currently is, staged here or inside the worker.
    ///
    /// A list rather than a map from action to bindings: two edits to one
    /// action can be outstanding at once, and folding them together would make
    /// the older one's completion clear the newer one's projection. Removing
    /// from the front is what keeps each entry with its own completion, and the
    /// list is as deep as one burst of deliberate gestures — a handful.
    projected_shortcuts: Vec<ProjectedShortcut>,
    /// Every edit the worker has taken and not answered for, oldest first.
    ///
    /// The staging queue can still name what it holds; an edit handed over
    /// belongs to the worker, and teardown that gives up waiting for it would
    /// otherwise have nothing to say beyond "some writes". Answers arrive in
    /// the order the edits went in, so the front is always the next completion's
    /// and this stays in step by popping one per completion.
    handed_over: VecDeque<ConfigEditIdentity>,
    /// How many edits the worker's channel holds. A field rather than the
    /// constant so the suite can fill one on purpose.
    capacity: usize,
    /// How long teardown waits. A field rather than the constant for the same
    /// reason: the branch that gives up is one the suite has to reach without
    /// stalling a real filesystem for five seconds.
    drain: Duration,
    /// Why there is no worker, when starting one failed. The edits it was meant
    /// to take say this when their turn comes.
    start_failure: Option<String>,
}

#[derive(Debug)]
struct RunningWorker {
    /// Dropped to tell the worker to finish its queue and stop.
    commands: Option<SyncSender<ConfigEdit>>,
    completions: Receiver<ConfigEditCompletion>,
    worker: Option<JoinHandle<()>>,
}

impl ConfigEditWorker {
    pub(in crate::backend::wayland) fn new(wake: RuntimeWakeHandle) -> Self {
        Self::with_capacity(wake, EDIT_QUEUE_CAPACITY)
    }

    fn with_capacity(wake: RuntimeWakeHandle, capacity: usize) -> Self {
        Self {
            wake,
            running: None,
            staged: VecDeque::new(),
            projected_shortcuts: Vec::new(),
            handed_over: VecDeque::new(),
            capacity,
            drain: SHUTDOWN_DRAIN,
            start_failure: None,
        }
    }

    /// [`Self::new`], with teardown's bound shortened for the suite.
    #[cfg(test)]
    fn with_drain(wake: RuntimeWakeHandle, drain: Duration) -> Self {
        let mut worker = Self::new(wake);
        worker.drain = drain;
        worker
    }

    /// The shortcut deltas an edit made now has to take account of, oldest
    /// first.
    ///
    /// Two layers decide a chord, and this is the first. Applying these to the
    /// running keymap is what makes the *accept* accurate — a chord an
    /// outstanding edit has already asked to give up is free, and a chord one
    /// has already asked for is taken — so the palette stops refusing edits the
    /// file would take. It is not the arbiter: these are requests, and the file
    /// may have been given the chord by somebody else since this run read it.
    /// The write re-checks every claim against the file it is about to change
    /// and refuses there ([`crate::config::ShortcutClaimedOnDisk`]), which is
    /// the answer that counts.
    pub(in crate::backend::wayland) fn projected_shortcuts(&self) -> &[ProjectedShortcut] {
        &self.projected_shortcuts
    }

    /// Queue an edit for the worker.
    ///
    /// It joins the back of the staging queue and goes into the worker's channel
    /// as soon as that channel has room — never before an edit submitted
    /// earlier, and never answered here. Everything the caller hears about this
    /// edit arrives through [`Self::try_recv`], in the order it was submitted,
    /// including the failures this side produces: a worker that will not start
    /// and a worker that has stopped are answers about the write, and answering
    /// them early would put them in front of gestures the user made first.
    pub(in crate::backend::wayland) fn submit(&mut self, edit: ConfigEdit) {
        if let ConfigEdit::Keybinding(write) = &edit {
            self.projected_shortcuts.push(ProjectedShortcut {
                action: write.request.action,
                bindings: write.bindings.clone(),
            });
        }
        self.staged.push_back(edit);
        self.pump();
        if self.stalled() {
            // No worker will finish anything, so nothing else will wake the loop
            // to let the drain turn these into toasts.
            if let Err(error) = self.wake.wake() {
                error!("Failed to wake the overlay for an unqueueable config edit: {error}");
            }
        }
    }

    /// Hand the worker as much of the staging queue as its channel will take.
    fn pump(&mut self) {
        if self.staged.is_empty() {
            return;
        }
        if self.running.is_none() {
            match self.start() {
                Ok(()) => self.start_failure = None,
                Err(error) => {
                    self.start_failure =
                        Some(format!("could not start the config-edit worker: {error}"));
                    return;
                }
            }
        }
        let Some(running) = self.running.as_mut() else {
            return;
        };
        let mut disconnected = false;
        {
            let Some(commands) = running.commands.as_ref() else {
                return;
            };
            while let Some(edit) = self.staged.pop_front() {
                let identity = edit.identity();
                match commands.try_send(edit) {
                    // Named here rather than at submission, because this is
                    // where the edit stops being this side's to describe.
                    Ok(()) => self.handed_over.push_back(identity),
                    // The channel is full: this edit and everything behind it
                    // wait, in order, for the worker to make room.
                    Err(TrySendError::Full(edit)) => {
                        self.staged.push_front(edit);
                        break;
                    }
                    Err(TrySendError::Disconnected(edit)) => {
                        self.staged.push_front(edit);
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        if disconnected {
            running.commands.take();
        }
    }

    /// Whether the staging queue is holding edits no worker will ever answer.
    fn stalled(&self) -> bool {
        !self.staged.is_empty()
            && self
                .running
                .as_ref()
                .is_none_or(|running| running.commands.is_none())
    }

    /// What to tell an edit that never reached a worker.
    fn stop_reason(&self) -> String {
        self.start_failure
            .clone()
            .unwrap_or_else(|| "the config-edit worker has stopped".to_string())
    }

    /// The next finished write, if one is waiting.
    pub(in crate::backend::wayland) fn try_recv(&mut self) -> Option<ConfigEditCompletion> {
        // Room may have freed since the last pass, and an edit that gets in now
        // is answered in its own place rather than at the back.
        self.pump();
        let mut finished = None;
        if let Some(running) = self.running.as_mut() {
            match running.completions.try_recv() {
                Ok(completion) => finished = Some(completion),
                // Everything the worker still holds was submitted before
                // anything staged, so nothing here may be answered yet.
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    // The worker died with its sender; nothing more will arrive
                    // from it, and what it never took is answered below.
                    running.commands.take();
                }
            }
        }
        if finished.is_some() {
            self.handed_over.pop_front();
        }
        let completion = match finished {
            Some(completion) => completion,
            // No worker, or none left: nothing older can still arrive, so the
            // edits it never took come back as failed writes, oldest first.
            None => {
                let edit = self.staged.pop_front()?;
                let reason = self.stop_reason();
                ConfigEditCompletion {
                    edit,
                    result: Err(anyhow!("{reason}")),
                }
            }
        };
        self.retire_projection(&completion);
        Some(completion)
    }

    /// Take a shortcut delta's projection off, now that its edit is answered.
    ///
    /// Every outcome clears it, because after any of them the running keymap is
    /// the honest answer again — the projection exists only to describe edits
    /// nothing has decided yet. A landed write installs the delta, so the
    /// keymap holds it. A write that *failed* installs it too — throwing away a
    /// shortcut the user just typed is the worse outcome — so a chord that edit
    /// freed is genuinely free in the keymap, and stays free without any help
    /// from here. A refusal installs nothing, and the keymap still holds the
    /// bindings the edit asked to move, so the chord it would have freed goes
    /// back to being taken; that also covers the delta the run has to decline
    /// because it collides with an earlier one. Keeping any of these on past
    /// their completion would leave a second, staler source of truth beside the
    /// keymap the run actually dispatches from.
    ///
    /// Answered in submission order, so the oldest projection belongs to the
    /// oldest completion.
    fn retire_projection(&mut self, completion: &ConfigEditCompletion) {
        if matches!(completion.edit, ConfigEdit::Keybinding(_))
            && !self.projected_shortcuts.is_empty()
        {
            self.projected_shortcuts.remove(0);
        }
    }

    /// Wait, briefly, for queued writes to reach the disk, then stop.
    ///
    /// Called from the event loop's teardown: an edit the user made a moment
    /// before quitting has to land, but a filesystem that has stopped answering
    /// must not keep the overlay on screen.
    ///
    /// The staging queue goes with them. An edit waiting for room is an edit the
    /// user made, and quitting is not a reason to drop it — so it is handed over
    /// first, inside the same bound. What is given up here is the *wording*:
    /// there is no overlay left to show a toast on, so the completions are
    /// logged instead of routed.
    ///
    /// The bound can run out, and what teardown owes the user then is an honest
    /// account rather than a reassuring one. It does not stop the worker — the
    /// edits it has taken go on being written for as long as the process lives,
    /// with their answers falling on the floor — and it reports every edit it
    /// did not hear back about by name, as possibly unsaved. A drain that *did*
    /// complete is stronger than that and is treated as such: the worker is
    /// joined, so those writes are on disk before this returns.
    pub(in crate::backend::wayland) fn shutdown(&mut self) -> ConfigEditShutdownReport {
        let deadline = Instant::now() + self.drain;
        self.hand_over_staged(deadline);

        let Some(mut running) = self.running.take() else {
            return self.account_for_the_rest(false);
        };
        // The worker stops once its queue is empty and the sender is gone.
        running.commands.take();

        let mut drained = false;
        let mut timed_out = false;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                timed_out = true;
                break;
            }
            match running.completions.recv_timeout(remaining) {
                Ok(completion) => {
                    self.handed_over.pop_front();
                    log_completion(&completion);
                }
                Err(RecvTimeoutError::Timeout) => {
                    timed_out = true;
                    break;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    drained = true;
                    break;
                }
            }
        }

        // Only join a worker that has already finished: waiting on one that is
        // stuck in a write is the very thing the bound above rules out.
        if drained
            && let Some(worker) = running.worker.take()
            && worker.join().is_err()
        {
            error!("The config-edit worker panicked");
        }
        self.account_for_the_rest(timed_out)
    }

    /// Say, by name, what teardown is leaving behind.
    ///
    /// Two different claims, kept apart because they are not equally certain.
    /// An edit the worker took and never answered for may well be in the file
    /// by now — the worker is still writing — so it is named as one to check,
    /// not as one that was lost. An edit no worker ever took is simply not
    /// there.
    fn account_for_the_rest(&mut self, timed_out: bool) -> ConfigEditShutdownReport {
        let unconfirmed: Vec<ConfigEditIdentity> = self.handed_over.drain(..).collect();
        if !unconfirmed.is_empty() {
            let names = join_identities(&unconfirmed);
            if timed_out {
                warn!(
                    "Config edits were still being written {:?} after the overlay was told to \
                     quit, and these were not confirmed saved: {names}. The worker continues \
                     while the process lives, but process exit can discard every edit still \
                     outstanding; an in-flight write lands whole or not at all.",
                    self.drain
                );
            } else {
                warn!(
                    "The config-edit worker stopped before answering for these edits, which were \
                     not confirmed saved: {names}"
                );
            }
        }
        ConfigEditShutdownReport {
            unconfirmed,
            unwritten: self.report_unwritten(),
        }
    }

    /// Push the staging queue into the worker before teardown drops the sender.
    ///
    /// Room only frees as the worker finishes, and a finished write is how it
    /// says so, so this waits on completions to make space — logging them,
    /// because teardown has nowhere left to show them.
    fn hand_over_staged(&mut self, deadline: Instant) {
        loop {
            self.pump();
            if self.staged.is_empty() {
                return;
            }
            let Some(running) = self.running.as_mut() else {
                return;
            };
            if running.commands.is_none() {
                return;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return;
            }
            let received = running.completions.recv_timeout(remaining);
            match received {
                Ok(completion) => {
                    self.handed_over.pop_front();
                    log_completion(&completion);
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    /// Say what became of edits that never reached the file, rather than letting
    /// them go quiet.
    ///
    /// Nothing is outstanding once this has run — the completions teardown
    /// collected were logged rather than routed through [`Self::try_recv`], and
    /// what is drained here was never written at all — so the projections go
    /// with them. A worker restarted after this (the suite does it) must not
    /// begin with edits nobody is waiting on still masking the keymap.
    fn report_unwritten(&mut self) -> Vec<ConfigEditIdentity> {
        let unwritten: Vec<ConfigEditIdentity> =
            self.staged.drain(..).map(|edit| edit.identity()).collect();
        if !unwritten.is_empty() {
            warn!(
                "These config edits were still waiting for the worker at shutdown and were not \
                 saved: {}",
                join_identities(&unwritten)
            );
        }
        self.projected_shortcuts.clear();
        unwritten
    }

    fn start(&mut self) -> std::io::Result<()> {
        let (command_tx, command_rx) = sync_channel(self.capacity);
        let (completion_tx, completion_rx) = sync_channel(self.capacity);
        let wake = self.wake.clone();
        let worker = thread::Builder::new()
            .name("wayscriber-config-edits".to_string())
            .spawn(move || run_worker(&command_rx, &completion_tx, &wake))?;
        self.running = Some(RunningWorker {
            commands: Some(command_tx),
            completions: completion_rx,
            worker: Some(worker),
        });
        Ok(())
    }
}

impl Drop for ConfigEditWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// One line naming several edits, for a report that has to be readable.
fn join_identities(identities: &[ConfigEditIdentity]) -> String {
    identities
        .iter()
        .map(ConfigEditIdentity::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Write every accepted edit, and keep writing them whether or not anybody is
/// still listening.
///
/// The answers can stop being deliverable at any point: teardown's bound runs
/// out, the event loop's end of the worker goes, and the completion channel's
/// receiver goes with it. That is the moment this thread used to stop — with
/// edits already accepted still in its queue, which is the user's gesture
/// discarded on the grounds that there was nobody left to tell about it. The
/// wording was always the disposable half. The write is the half nobody can
/// reconstruct, so an undeliverable answer is logged once and the queue is
/// written to the end.
///
/// Nothing here can block on the failure: a dropped receiver makes `send`
/// return rather than wait, and the wake is skipped with it, because there is no
/// completion for the loop to pick up.
fn run_worker(
    commands: &Receiver<ConfigEdit>,
    completions: &SyncSender<ConfigEditCompletion>,
    wake: &RuntimeWakeHandle,
) {
    let mut answers_land = true;
    while let Ok(edit) = commands.recv() {
        // A panic in a write is still an answer the caller is owed: without
        // this the completion never arrives and a shortcut edit is neither
        // installed nor refused, which is the one outcome with no wording.
        let result = catch_unwind(AssertUnwindSafe(|| write_edit(&edit)))
            .unwrap_or_else(|_| Err(anyhow!("the config-edit worker panicked while writing")));
        if completions
            .send(ConfigEditCompletion { edit, result })
            .is_err()
        {
            if answers_land {
                warn!(
                    "The overlay stopped listening for config-edit results; the edits it already \
                     accepted are still being written, without wording"
                );
                answers_land = false;
            }
            continue;
        }
        if let Err(error) = wake.wake() {
            error!("Failed to wake the overlay for a finished config edit: {error}");
        }
    }
}

/// The overlay's three durable config writes, and the only place they happen.
fn write_edit(edit: &ConfigEdit) -> Result<ConfigEditOutcome> {
    match edit {
        ConfigEdit::Keybinding(write) => {
            persist_keybinding_edit(write.request.action, &write.bindings)
        }
        ConfigEdit::Preset(PresetAction::Save { slot, preset }) => {
            persist_preset_slot(*slot, Some(preset))
        }
        ConfigEdit::Preset(PresetAction::Clear { slot }) => persist_preset_slot(*slot, None),
        ConfigEdit::QuickColor(edit) => persist_quick_color(edit.index, edit.color),
    }
}

/// What a completion says, for a run that has no toast left to show it.
fn log_completion(completion: &ConfigEditCompletion) {
    match &completion.result {
        Ok(_) => info!("A queued config edit was written during shutdown"),
        Err(error) => warn!("A queued config edit failed during shutdown: {error:#}"),
    }
}

/// Teardown for the config-edit path: queue what is still pending, then stop.
///
/// The overlay's loop can be told to exit in the same batch of input events that
/// carried a gesture — the chord captured on one key press and the Escape that
/// closes the overlay arrive together — and the loop breaks on the exit before
/// the pass that would have queued the gesture ever runs. Teardown is the last
/// place that can still notice, so it drains the outbox's durable config batch
/// and only then stops the worker. Without it a rebind,
/// a recolor accepted on the picker's OK, or a preset slot saved a moment before
/// quitting is silently not written: no error, no toast, and nothing in the file.
///
/// The inventory belongs to `InputEffectDrain::DurableConfig`. A new effect
/// whose drain queues a `ConfigEdit` must join that scope so the running and
/// shutdown paths cannot drift; the running loop alone is not enough because
/// exit skips it.
///
/// What is given up is the wording. These completions are logged rather than
/// shown — there is no overlay left to put a toast on — and the write is the
/// half the user cannot reconstruct from memory.
///
/// The drained effects queue through the same helpers the running loop uses;
/// those cannot each borrow `WaylandState` while this function holds it.
pub(in crate::backend::wayland) fn finish_config_edits(
    config: &mut Config,
    input_state: &mut InputState,
    worker: &mut ConfigEditWorker,
) {
    for effect in input_state.drain_input_effects(InputEffectDrain::DurableConfig) {
        match effect {
            InputEffect::Preset(action) => queue_preset_action(config, worker, action),
            InputEffect::QuickColor(edit) => queue_quick_color_edit(config, worker, edit),
            InputEffect::KeybindingEdit(request) => {
                queue_keybinding_edit(&config.keybindings, input_state, worker, request);
            }
            effect @ (InputEffect::Backend(_)
            | InputEffect::SpotlightMagnifierFeedback
            | InputEffect::ToolbarPersistence(_)
            | InputEffect::OutputFocus(_)
            | InputEffect::Zoom(_)
            | InputEffect::CopyHex(_)
            | InputEffect::PasteHex(_)
            | InputEffect::TextCopy(_)
            | InputEffect::TextPaste(_)
            | InputEffect::SelectionClipboardPublish(_)
            | InputEffect::ClipboardPaste(_)
            | InputEffect::FrozenPass { .. }
            | InputEffect::EyedropperToggle
            | InputEffect::OcrPass { .. }
            | InputEffect::BoardRuntimeUi(_)) => {
                unreachable!("durable config drain returned {effect:?}")
            }
        }
    }
    worker.shutdown();
}

impl WaylandState {
    /// Apply every write that has finished since the last pass.
    pub(in crate::backend::wayland) fn drain_config_edit_completions(&mut self) {
        while let Some(completion) = self.preferences.config_edits_mut().try_recv() {
            self.finish_config_edit(completion);
        }
    }

    pub(in crate::backend::wayland) fn shutdown_config_edits(&mut self) {
        finish_config_edits(
            &mut self.config,
            &mut self.input_state,
            self.preferences.config_edits_mut(),
        );
    }

    fn finish_config_edit(&mut self, completion: ConfigEditCompletion) {
        let ConfigEditCompletion { edit, result } = completion;
        match edit {
            ConfigEdit::Keybinding(write) => self.finish_keybinding_edit(write, result),
            ConfigEdit::Preset(action) => self.finish_preset_action(&action, result),
            ConfigEdit::QuickColor(edit) => self.finish_quick_color_edit(edit, result),
        }
    }
}

#[cfg(test)]
mod tests;
