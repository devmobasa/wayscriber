//! Shortcut edits from the overlay's palette and toolbar.
//!
//! The palette's Edit/Unbind/Reset controls are an explicit user edit action,
//! so they are durable: one action's `[keybindings]` entry is written to
//! `config.toml`, and only then does the edit land in `self.config.keybindings`
//! — the effective holder the rest of the run reads — with the two runtime maps
//! rebuilt from it. The write itself happens on the config-edit worker (see
//! `crate::backend::wayland::config_edits`), because parsing, copying, and
//! fsyncing a file is not work for the thread that dispatches input.
//!
//! The ordering survives the move. `prepare_keybinding_edit` takes the running
//! keymap by shared reference and hands the *write* a delta — one action and
//! the bindings it should end up with — so there is nothing installed to undo;
//! `shortcut_completion` is what decides, from the write's answer, whether the
//! delta is folded in at all, and `install_keybinding_edit` folds it into
//! whatever keymap the run holds by then rather than into a copy taken earlier.
//!
//! A failed save is not a failed edit. The keymap keeps the change for the run
//! and the toast says the file did not get it, because throwing away a shortcut
//! the user just typed would be the worse of the two outcomes.
//!
//! One refusal is the exception. If the file has given the chord to another
//! action since this run read it, the edit is not degraded but rejected: the
//! save reports it before writing anything, and the run must not be left
//! holding a shortcut the file just said belongs elsewhere.
//!
//! Nothing here claims a write that did not happen. An edit the file already
//! resolves to is not written at all, and every wording that follows one says
//! "already" rather than reporting a save.
//!
//! A second edit issued while the first is still being written is checked
//! against the running keymap with the outstanding deltas folded into it. The
//! keymap alone would be the wrong question to ask: nothing is installed until
//! a write answers, so it still shows the bindings the first edit has already
//! asked to move, and a gesture reaching for a chord that edit gave up would be
//! refused over a claim the file is about to drop. The projection is a check,
//! not an authority — the write re-checks every claim against the file it is
//! about to change, and that is the refusal that counts.
//!
//! If the two edits contest a chord, the second is caught by that on-disk
//! refusal — by then the first edit is in the file — and named as such. If they
//! contest nothing, both land: each completion installs only its own action's
//! bindings, so the second cannot take the first back out.
//!
//! The one pair that reaches neither of those is a first edit whose *write*
//! failed: it kept its chord for the run without the file ever hearing about it,
//! so a second edit onto that chord has nothing to be refused by on disk and is
//! refused here instead. What the user is told then depends on what the file did
//! with the second edit — wrote it, already held it, or failed as well — and the
//! three wordings are kept apart, because "saved to config.toml" over a file
//! that got neither edit, or over one that was never written because it already
//! said this, sends the user looking for something that is not there.

use super::super::config_edits::{
    ConfigEdit, ConfigEditWorker, KeybindingEditWrite, ProjectedShortcut,
};
use super::WaylandState;
use crate::config::{
    Action, ConfigEditNotReadBack, ConfigEditOutcome, ConfigEditWrite, KeyBinding,
    KeybindingsConfig, ShortcutClaimedOnDisk, action_label,
};
use crate::input::state::{
    InputState, KeybindingEditOperation, KeybindingEditRequest, Toast, ToastPriority,
};
use std::collections::HashMap;

/// Why a shortcut edit was refused. Both arms leave the keymap untouched.
#[derive(Debug)]
enum KeybindingEditError {
    /// Unparseable text, an unsupported action, or a request that repeats one
    /// chord — described in the message the user sees.
    Edit(String),
    /// The chord already belongs to another action.
    Conflict {
        binding: String,
        existing_action: Action,
    },
}

/// The binding list the request wants the action to end up with.
///
/// `Reset` resolves here rather than in the request so it always means "the
/// shortcut this build ships", however long the palette row sat open.
fn requested_bindings(request: &KeybindingEditRequest) -> Vec<String> {
    match &request.operation {
        KeybindingEditOperation::Replace(bindings) => bindings.clone(),
        KeybindingEditOperation::Delete => Vec::new(),
        KeybindingEditOperation::Reset => KeybindingsConfig::default()
            .bindings_for_action(request.action)
            .map(<[String]>::to_vec)
            .unwrap_or_default(),
    }
}

/// Apply one edit to a keymap the caller owns, or explain why it cannot be.
///
/// The caller passes a copy: on `Err` nothing has moved, and on `Ok` the copy
/// is the keymap to install.
fn apply_keybinding_edit(
    keybindings: &mut KeybindingsConfig,
    request: &KeybindingEditRequest,
) -> Result<(), KeybindingEditError> {
    let bindings = requested_bindings(request);

    // Build the conflict lookup without the action being edited. Its current
    // bindings are what the edit is replacing, so they cannot contest it.
    let mut other_bindings = keybindings.clone();
    other_bindings
        .set_bindings_for_action(request.action, Vec::new())
        .map_err(KeybindingEditError::Edit)?;
    // Claimed keys, not the strict keymap: a duplicate or a typo somewhere else
    // in the configuration is a problem this edit neither created nor touches,
    // and refusing over it would leave the user unable to change any shortcut
    // at all until they found it (#293).
    let claimed = other_bindings.claimed_keys();
    let mut requested = HashMap::new();
    for binding_text in &bindings {
        let binding = KeyBinding::parse(binding_text).map_err(KeybindingEditError::Edit)?;
        if let Some(existing_action) = claimed.get(&binding) {
            return Err(KeybindingEditError::Conflict {
                binding: binding_text.clone(),
                existing_action: *existing_action,
            });
        }
        if let Some(first) = requested.insert(binding, binding_text.clone()) {
            return Err(KeybindingEditError::Edit(format!(
                "Shortcut not changed — {first} is listed twice for {}.",
                action_label(request.action)
            )));
        }
    }

    keybindings
        .set_bindings_for_action(request.action, bindings)
        .map_err(KeybindingEditError::Edit)
}

fn shortcut_conflict_message(binding: &str, existing_action: Action) -> String {
    format!(
        "Shortcut not changed — {binding} is already assigned to {}.",
        action_label(existing_action)
    )
}

/// What the user is told after an edit lands in the keymap and in the file.
fn shortcut_applied_message(request: &KeybindingEditRequest) -> String {
    let action = action_label(request.action);
    match request.operation {
        KeybindingEditOperation::Replace(_) => format!("Updated shortcut for {action}."),
        KeybindingEditOperation::Delete => format!("Unbound {action}."),
        KeybindingEditOperation::Reset => format!("Reset the shortcut for {action} to default."),
    }
}

/// What the user is told when the file already said what the edit asked for.
///
/// Nothing was written, so nothing may claim it was. Reset reaches this
/// whenever the action already resolves to the shipped shortcut — most often
/// because the file omits it, which is the default state for most actions.
fn shortcut_unchanged_message(request: &KeybindingEditRequest) -> String {
    let action = action_label(request.action);
    match request.operation {
        KeybindingEditOperation::Replace(_) => format!("{action} already uses that shortcut."),
        KeybindingEditOperation::Delete => format!("{action} was already unbound."),
        KeybindingEditOperation::Reset => format!("{action} already uses the default shortcut."),
    }
}

/// What the user is told when the file, not this run's keymap, owns the chord.
fn shortcut_claimed_on_disk_message(binding: &str, existing_action: Action) -> String {
    format!(
        "Shortcut not changed — config.toml now assigns {binding} to {}.",
        action_label(existing_action)
    )
}

/// What the user is told when the keymap took the edit but the file did not.
///
/// One message for every failure — unreadable file, read-only file, a second
/// writer that keeps winning the revision race — because what the user can act
/// on is the same in each case: the shortcut works now, and the reason it was
/// not saved is in the log.
const SHORTCUT_SAVE_FAILED: &str =
    "Shortcut updated for this run, but saving to config.toml failed (see logs).";

/// What the user is told when the write landed but the file does not read back
/// with the shortcut. Separated from the message above because the file *did*
/// change: sending the user off believing it is untouched would be the wrong
/// place to look.
const SHORTCUT_WRITE_UNVERIFIED: &str = concat!(
    "Shortcut updated for this run, but config.toml was written and does not ",
    "read back with it — check the file (see logs)."
);

/// What the user is told when the file took the edit but this run cannot.
///
/// Only one thing puts the run here: an earlier edit whose write failed kept its
/// chord for the session, and this edit — which the file accepted, because the
/// file never got that chord — would now put two actions on it. The run keeps
/// the keymap it can still dispatch from, and the message points at the file,
/// where the two now disagree.
const SHORTCUT_NOT_INSTALLED: &str = concat!(
    "Shortcut saved to config.toml, but this run kept its own — another edit ",
    "here already uses that key (see logs)."
);

/// The same disagreement, over a save that never happened.
///
/// The file was already carrying the requested shortcut, so the write had
/// nothing to do and spent no backup. Where the file did take the edit there is
/// something new to go and look at; here there is not, and "saved to
/// config.toml" would claim a change this gesture never made. What the user can
/// act on is the same either way — the file and the run disagree — so that is
/// what the wording says, without the claim.
const SHORTCUT_ALREADY_CURRENT_NOT_INSTALLED: &str = concat!(
    "config.toml already has this shortcut, but this run kept its own — another ",
    "edit here already uses that key (see logs)."
);

/// The same collision one edit later, with the file on the other side of it.
///
/// The earlier edit failed its write and kept its chord for the run; this edit
/// contests that chord *and* failed its own write. So nothing landed anywhere:
/// the file has neither shortcut and the run cannot take a second action onto
/// the key it is already dispatching. Saying "saved to config.toml" here would
/// send the user to a file that never heard of the edit — the failure above is
/// about a disagreement between the file and the run, and this one is about
/// there being nothing to disagree with.
const SHORTCUT_NOT_SAVED_OR_INSTALLED: &str = concat!(
    "Shortcut not changed — config.toml did not take it and another edit here ",
    "already uses that key (see logs)."
);

/// Whether the file ended up holding the list the edit asked for.
///
/// Kept past the message the write's own answer produced, because one more
/// question is asked afterwards — whether the run can install the edit — and the
/// wording for a refusal there is only honest if it knows this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortcutFileOutcome {
    /// `config.toml` holds the requested bindings because this write put them
    /// there.
    Wrote,
    /// It holds them, and held them before this edit was made: the write had no
    /// delta and touched nothing. The file is carrying the shortcut, but this
    /// gesture did not save anything, and the two are not the same claim.
    AlreadyCurrent,
    /// It does not hold them. The write failed, or it landed and the value did
    /// not read back — either way the file is not carrying this shortcut.
    Rejected,
}

/// What the user is told when the run cannot install an edit the write answered.
///
/// All three branches are the same collision and all three leave the keymap
/// alone; what differs is what the file is holding and how it came to, which is
/// the only part the user can act on. One message for all of them would have to
/// claim one of the three, and the claim it used to make — "Shortcut saved to
/// config.toml" — is false both where nothing was saved at all and where the
/// file already said this and no write ran.
fn shortcut_not_installed_message(file: ShortcutFileOutcome) -> &'static str {
    match file {
        ShortcutFileOutcome::Wrote => SHORTCUT_NOT_INSTALLED,
        ShortcutFileOutcome::AlreadyCurrent => SHORTCUT_ALREADY_CURRENT_NOT_INSTALLED,
        ShortcutFileOutcome::Rejected => SHORTCUT_NOT_SAVED_OR_INSTALLED,
    }
}

/// Turn a request into the write to queue, or into the refusal to show.
///
/// Takes the running keymap by shared reference on purpose. The edit it returns
/// is a delta handed to the write and folded in only if the file takes it, and
/// a signature that cannot reach the run's keymap is what makes that ordering a
/// property of the code rather than of the order two statements happen to be in.
///
/// `in_flight` is what keeps that ordering from making the check wrong. Nothing
/// is installed until a write answers, so the running keymap describes the file
/// as it stood before every queued edit: an edit that moved Pen off `F` is
/// invisible in it until its completion lands, and the next gesture reaching for
/// `F` is refused over a claim that is already on its way out — while the file,
/// which will have taken the first edit by the time the second is written,
/// would have accepted it. Folding the outstanding deltas in, in submission
/// order, makes this check about the keymap the completions are going to leave
/// behind rather than the one they started from.
///
/// It stays a *check*, not an authority. These deltas are requests the file has
/// not answered yet, and the file may have been given the chord by somebody
/// else since this run read it, so the write re-checks every claim against the
/// file it is about to change and refuses there ([`ShortcutClaimedOnDisk`]).
/// This layer is what makes an accept accurate; that one is what makes it true.
///
/// The candidate keymap built here is a gate, not a payload: it proves the edit
/// still yields a keymap both runtime views can be built from, and is then
/// dropped. Carrying it to the completion is what let a second edit's install
/// revert a first one that landed while it was in flight.
fn prepare_keybinding_edit(
    keybindings: &KeybindingsConfig,
    in_flight: &[ProjectedShortcut],
    request: KeybindingEditRequest,
) -> Result<KeybindingEditWrite, String> {
    let mut next = keybindings.clone();
    for projected in in_flight {
        next.set_bindings_for_action(projected.action, projected.bindings.clone())
            .map_err(|err| format!("Shortcut not changed: {err}"))?;
    }
    if let Err(error) = apply_keybinding_edit(&mut next, &request) {
        return Err(match error {
            KeybindingEditError::Edit(message) => message,
            KeybindingEditError::Conflict {
                binding,
                existing_action,
            } => shortcut_conflict_message(&binding, existing_action),
        });
    }

    next.build_action_map()
        .map_err(|err| format!("Shortcut not changed: {err}"))?;
    next.build_action_bindings()
        .map_err(|err| format!("Shortcut not changed: {err}"))?;
    // What the keymap would end up with, which is what the file should say.
    let bindings = next
        .bindings_for_action(request.action)
        .map(<[String]>::to_vec)
        .unwrap_or_default();

    Ok(KeybindingEditWrite { request, bindings })
}

/// The run's keymap, and the two views built from it, after one edit lands.
struct InstalledKeybindings {
    keybindings: KeybindingsConfig,
    action_map: HashMap<KeyBinding, Action>,
    action_bindings: HashMap<Action, Vec<KeyBinding>>,
}

/// Fold one landed edit into the keymap the run holds *now*.
///
/// A delta, never a snapshot. Two edits can be outstanding at once — the worker
/// writes them one after another — and installing a keymap prepared before the
/// first one landed would take the first shortcut back out while telling the
/// user both were saved.
///
/// Two edits cannot land on the same chord: the worker serializes writes, and
/// each write re-checks the chord's claims against the file it is about to
/// change, so a second edit contesting a chord the first just took is refused
/// there ([`ShortcutClaimedOnDisk`]) and never reaches this function. The one
/// path that can still present a contested pair is a first edit whose *write*
/// failed and was kept for the run anyway; the rebuild below refuses it rather
/// than installing a keymap the run cannot dispatch from.
fn install_keybinding_edit(
    keybindings: &KeybindingsConfig,
    write: &KeybindingEditWrite,
) -> Result<InstalledKeybindings, String> {
    let mut keybindings = keybindings.clone();
    keybindings.set_bindings_for_action(write.request.action, write.bindings.clone())?;
    let action_map = keybindings.build_action_map()?;
    let action_bindings = keybindings.build_action_bindings()?;
    Ok(InstalledKeybindings {
        keybindings,
        action_map,
        action_bindings,
    })
}

/// What a finished shortcut write means for the run.
///
/// `install` is `None` for exactly one outcome — the file gave the chord away
/// since this run read it — and that is the whole refusal: there is no rollback,
/// because the keymap never moved.
struct ShortcutCompletion {
    /// The delta to fold into the run's keymap, or `None` when the file refused
    /// the edit.
    install: Option<KeybindingEditWrite>,
    message: String,
    /// A refusal and a degraded save are both warnings; only a landed write is
    /// good news.
    saved: bool,
    /// What the file did, kept for the one question asked after this: an install
    /// the run has to refuse is worded differently depending on what the file is
    /// holding and on whether this gesture is what put it there.
    file: ShortcutFileOutcome,
}

fn shortcut_completion(
    write: KeybindingEditWrite,
    result: Result<ConfigEditOutcome, anyhow::Error>,
) -> ShortcutCompletion {
    match result {
        Ok(outcome) => {
            if let Some(backup) = &outcome.backup_path {
                log::info!("Backed up config to {} before the write", backup.display());
            }
            let (message, file) = match outcome.write {
                ConfigEditWrite::Wrote => (
                    shortcut_applied_message(&write.request),
                    ShortcutFileOutcome::Wrote,
                ),
                ConfigEditWrite::AlreadyCurrent => (
                    shortcut_unchanged_message(&write.request),
                    ShortcutFileOutcome::AlreadyCurrent,
                ),
            };
            ShortcutCompletion {
                install: Some(write),
                message,
                saved: true,
                // Written now or already there: either way the file holds it —
                // but only one of the two is a save, and the question asked
                // after this one is worded on the difference.
                file,
            }
        }
        Err(error) => {
            if let Some(conflict) = error.downcast_ref::<ShortcutClaimedOnDisk>() {
                log::info!(
                    "Refused the shortcut for {:?}: {conflict}",
                    write.request.action
                );
                return ShortcutCompletion {
                    install: None,
                    message: shortcut_claimed_on_disk_message(
                        &conflict.binding,
                        conflict.claimed_by,
                    ),
                    saved: false,
                    file: ShortcutFileOutcome::Rejected,
                };
            }
            log::warn!(
                "Failed to save the shortcut for {:?}: {error:#}",
                write.request.action
            );
            let message = match error.downcast_ref::<ConfigEditNotReadBack>() {
                Some(_) => SHORTCUT_WRITE_UNVERIFIED.to_string(),
                None => SHORTCUT_SAVE_FAILED.to_string(),
            };
            ShortcutCompletion {
                install: Some(write),
                message,
                saved: false,
                // A write that failed and a write that does not read back leave
                // the same thing behind for the next question: a file that is
                // not carrying this shortcut.
                file: ShortcutFileOutcome::Rejected,
            }
        }
    }
}

/// Turn one shortcut request into a queued write, or into the refusal to show.
///
/// Over the fields it touches, so teardown can queue a shortcut the user
/// captured in the same batch of events as the exit (see
/// `config_edits::finish_config_edits`), where lending out the whole state is
/// not available.
pub(in crate::backend::wayland) fn queue_keybinding_edit(
    keybindings: &KeybindingsConfig,
    input_state: &mut InputState,
    worker: &mut ConfigEditWorker,
    request: KeybindingEditRequest,
) {
    // In its own statement so the worker is only borrowed for the check: the
    // arm below submits through it.
    let prepared = prepare_keybinding_edit(keybindings, worker.projected_shortcuts(), request);
    match prepared {
        // Queued, not written here: the write parses the file, copies it aside,
        // renames, and fsyncs twice, and this is the thread that dispatches
        // input and paints. The delta it produced rides along and is folded in
        // by the completion, if at all.
        Ok(write) => worker.submit(ConfigEdit::Keybinding(write)),
        Err(message) => {
            input_state.push_toast(ToastPriority::Info, "keybindings", Toast::warning(message));
        }
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn handle_keybinding_edit(
        &mut self,
        request: KeybindingEditRequest,
    ) {
        queue_keybinding_edit(
            &self.config.keybindings,
            &mut self.input_state,
            &mut self.config_edits,
            request,
        );
    }

    pub(in crate::backend::wayland) fn finish_keybinding_edit(
        &mut self,
        write: KeybindingEditWrite,
        result: Result<ConfigEditOutcome, anyhow::Error>,
    ) {
        let ShortcutCompletion {
            install,
            mut message,
            mut saved,
            file,
        } = shortcut_completion(write, result);
        if let Some(write) = install {
            // Against the keymap as it stands, not as it stood when this edit
            // was accepted: another edit may have landed in between, and it
            // keeps what it was given.
            match install_keybinding_edit(&self.config.keybindings, &write) {
                Ok(installed) => {
                    self.config.keybindings = installed.keybindings;
                    self.input_state
                        .set_keybinding_maps(installed.action_map, installed.action_bindings);
                    self.toolbar.mark_dirty();
                }
                Err(error) => {
                    log::warn!(
                        "Kept the running keymap over the shortcut for {:?}: {error}",
                        write.request.action
                    );
                    // The write's own wording described a file this run could
                    // still take the edit into; it cannot, so the message has to
                    // be about the collision — and about whether the file is
                    // holding anything for the user to go and look at.
                    message = shortcut_not_installed_message(file).to_string();
                    saved = false;
                }
            }
        }

        // The chip cannot carry this action's own Keybindings section: a toast
        // action is a bare `Action`, and the destination vocabulary has no
        // matching variant. The tab-level route is the honest substitute.
        let toast = match saved {
            true => Toast::info(message),
            false => Toast::warning(message),
        };
        self.input_state.push_toast(
            ToastPriority::Action,
            "keybindings",
            toast.action("Edit", Action::OpenConfiguratorKeybindings),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::config::io::{is_stale_source_error, persist_keybinding_edit_at};
    use crate::config::persist_keybinding_edit;
    use crate::config::test_helpers::with_temp_config_home;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Written under the real (flat) `[keybindings]` keys, on a chord no
    /// shipped default claims, so the reload assertion is about this run's edit
    /// rather than about conflict resolution.
    const AUTHORED_SHORTCUTS: &str =
        "[keybindings]\nundo = ['Ctrl+Z']\nselect_pen_tool = ['Ctrl+Alt+Shift+P']\n";

    fn replace(action: Action, binding: &str) -> KeybindingEditRequest {
        KeybindingEditRequest {
            action,
            operation: KeybindingEditOperation::Replace(vec![binding.to_string()]),
        }
    }

    /// The prepare a palette makes with nothing outstanding.
    ///
    /// Spelled out here rather than at every call site because it is the
    /// assumption most of these tests are making: an empty projection is a run
    /// whose queue is empty, where the running keymap is the whole of what the
    /// check has to go on.
    fn prepare(
        keybindings: &KeybindingsConfig,
        request: KeybindingEditRequest,
    ) -> Result<KeybindingEditWrite, String> {
        prepare_keybinding_edit(keybindings, &[], request)
    }

    /// The same, with edits already queued and unanswered.
    fn prepare_behind(
        keybindings: &KeybindingsConfig,
        in_flight: &[(Action, &str)],
        request: KeybindingEditRequest,
    ) -> Result<KeybindingEditWrite, String> {
        let in_flight: Vec<ProjectedShortcut> = in_flight
            .iter()
            .map(|(action, binding)| ProjectedShortcut {
                action: *action,
                bindings: vec![(*binding).to_string()],
            })
            .collect();
        prepare_keybinding_edit(keybindings, &in_flight, request)
    }

    /// The point of the restored flow: the chord the user typed is in the
    /// keymap the moment the edit is accepted, and the message claims a durable
    /// change because the write below makes it one.
    #[test]
    fn a_captured_chord_replaces_the_binding_and_reports_a_durable_change() {
        let mut keybindings = KeybindingsConfig::default();
        let request = replace(Action::SelectPenTool, "Ctrl+Alt+Shift+K");

        apply_keybinding_edit(&mut keybindings, &request).expect("a free chord is accepted");

        assert_eq!(
            keybindings.bindings_for_action(Action::SelectPenTool),
            Some(&["Ctrl+Alt+Shift+K".to_string()][..])
        );
        assert_eq!(
            shortcut_applied_message(&request),
            "Updated shortcut for Pen Tool."
        );
    }

    #[test]
    fn a_taken_chord_is_refused_and_names_the_action_that_owns_it() {
        let mut keybindings = KeybindingsConfig::default();

        let error = apply_keybinding_edit(&mut keybindings, &replace(Action::ClearCanvas, "F"))
            .expect_err("the pen shortcut is taken");

        match error {
            KeybindingEditError::Conflict {
                binding,
                existing_action,
            } => {
                assert_eq!(binding, "F");
                assert_eq!(existing_action, Action::SelectPenTool);
                assert_eq!(
                    shortcut_conflict_message(&binding, existing_action),
                    "Shortcut not changed — F is already assigned to Pen Tool."
                );
            }
            other => panic!("expected a structured shortcut conflict, got {other:?}"),
        }
        assert_eq!(
            keybindings.bindings_for_action(Action::ClearCanvas),
            KeybindingsConfig::default().bindings_for_action(Action::ClearCanvas),
            "a refused edit leaves the keymap alone"
        );
    }

    /// The claim check works on parsed bindings, whose equality folds key case,
    /// so respelling a taken chord cannot sneak it past the gate.
    #[test]
    fn an_edit_onto_a_key_spelled_in_a_different_case_is_still_refused() {
        let mut keybindings = KeybindingsConfig::default();
        keybindings.core.undo = vec!["Ctrl+Alt+U".to_string()];

        let error = apply_keybinding_edit(
            &mut keybindings,
            &replace(Action::ClearCanvas, "ctrl+alt+u"),
        )
        .expect_err("the chord is taken regardless of spelling");

        match error {
            KeybindingEditError::Conflict {
                binding,
                existing_action,
            } => {
                assert_eq!(binding, "ctrl+alt+u");
                assert_eq!(existing_action, Action::Undo);
            }
            other => panic!("expected a case-insensitive conflict, got {other:?}"),
        }
    }

    /// Rebinding an action onto a key it already holds is not a self-conflict:
    /// the edited action is excluded from the claim lookup.
    #[test]
    fn an_action_can_be_rebound_onto_a_key_it_already_holds() {
        let mut keybindings = KeybindingsConfig::default();

        apply_keybinding_edit(&mut keybindings, &replace(Action::SelectPenTool, "F"))
            .expect("an action never conflicts with itself");

        assert_eq!(
            keybindings.bindings_for_action(Action::SelectPenTool),
            Some(&["F".to_string()][..])
        );
    }

    #[test]
    fn one_chord_listed_twice_is_refused_without_naming_another_action() {
        let mut keybindings = KeybindingsConfig::default();

        let error = apply_keybinding_edit(
            &mut keybindings,
            &KeybindingEditRequest {
                action: Action::SelectPenTool,
                operation: KeybindingEditOperation::Replace(vec![
                    "Ctrl+Alt+Shift+K".to_string(),
                    "ctrl+alt+shift+k".to_string(),
                ]),
            },
        )
        .expect_err("a repeated chord is not a usable list");

        match error {
            KeybindingEditError::Edit(message) => assert_eq!(
                message,
                "Shortcut not changed — Ctrl+Alt+Shift+K is listed twice for Pen Tool."
            ),
            other => panic!("expected a plain edit refusal, got {other:?}"),
        }
    }

    #[test]
    fn unbinding_empties_the_actions_binding_list() {
        let mut keybindings = KeybindingsConfig::default();
        let request = KeybindingEditRequest {
            action: Action::SelectPenTool,
            operation: KeybindingEditOperation::Delete,
        };

        apply_keybinding_edit(&mut keybindings, &request).expect("an unbind always applies");

        assert_eq!(
            keybindings.bindings_for_action(Action::SelectPenTool),
            Some(&[][..])
        );
        assert_eq!(shortcut_applied_message(&request), "Unbound Pen Tool.");
    }

    #[test]
    fn resetting_restores_the_compiled_default() {
        let mut keybindings = KeybindingsConfig::default();
        let default = keybindings
            .bindings_for_action(Action::SelectPenTool)
            .map(<[String]>::to_vec)
            .expect("the pen tool ships a shortcut");
        keybindings
            .set_bindings_for_action(Action::SelectPenTool, vec!["Ctrl+Alt+Shift+J".to_string()])
            .expect("the pen tool stores a shortcut");

        let request = KeybindingEditRequest {
            action: Action::SelectPenTool,
            operation: KeybindingEditOperation::Reset,
        };
        apply_keybinding_edit(&mut keybindings, &request).expect("a reset to defaults applies");

        assert_eq!(
            keybindings.bindings_for_action(Action::SelectPenTool),
            Some(default.as_slice())
        );
        assert_eq!(
            shortcut_applied_message(&request),
            "Reset the shortcut for Pen Tool to default."
        );
    }

    /// A rebound keymap still builds both runtime views; without that the
    /// handler would refuse its own accepted edit.
    #[test]
    fn an_edited_keymap_still_builds_both_runtime_views() {
        let mut keybindings = KeybindingsConfig::default();
        apply_keybinding_edit(
            &mut keybindings,
            &replace(Action::ClearCanvas, "Ctrl+Alt+Shift+K"),
        )
        .expect("a free chord is accepted");

        let action_map = keybindings.build_action_map().expect("action map");
        let action_bindings = keybindings
            .build_action_bindings()
            .expect("action bindings");

        let chord = KeyBinding::parse("Ctrl+Alt+Shift+K").expect("a parseable chord");
        assert_eq!(action_map.get(&chord), Some(&Action::ClearCanvas));
        assert_eq!(
            action_bindings.get(&Action::ClearCanvas),
            Some(&vec![chord])
        );
    }

    /// The ordering, at the seam the async write moved it to.
    ///
    /// Preparing an edit cannot touch the run's keymap — the signature only
    /// lends it — so the chord the user typed exists nowhere but the write until
    /// the file answers. That is what makes the refusal below a refusal rather
    /// than a rollback.
    #[test]
    fn preparing_an_edit_leaves_the_running_keymap_to_the_completion() {
        let running = KeybindingsConfig::default();
        let before = running
            .bindings_for_action(Action::SelectPenTool)
            .map(<[String]>::to_vec);

        let write = prepare(&running, replace(Action::SelectPenTool, "Ctrl+Alt+Shift+K"))
            .expect("a free chord is accepted");

        assert_eq!(
            write.bindings,
            ["Ctrl+Alt+Shift+K".to_string()],
            "the write carries the list the file is asked to hold"
        );
        assert_eq!(
            write.request.action,
            Action::SelectPenTool,
            "and the action it belongs to, which is the whole of the delta"
        );
        assert_eq!(
            running
                .bindings_for_action(Action::SelectPenTool)
                .map(<[String]>::to_vec),
            before,
            "the running keymap must be exactly as it was"
        );
    }

    /// The chord an outstanding edit is giving up is free for the next one.
    ///
    /// The palette moves Pen off `F` and, before that write reports back, binds
    /// Marker to `F`. Nothing is installed until a completion arrives, so the
    /// running keymap still shows Pen on `F` — and checking against it alone
    /// refuses the second gesture over a claim the file is about to drop, while
    /// the file itself, which will have taken the first edit by the time the
    /// second is written, accepts it. The claim check reads the keymap with the
    /// outstanding delta folded in, so the two agree.
    #[test]
    fn a_chord_an_in_flight_edit_is_giving_up_is_free_for_the_next_edit() {
        let running = KeybindingsConfig::default();
        assert_eq!(
            running.bindings_for_action(Action::SelectPenTool),
            Some(&["F".to_string()][..]),
            "the fixture is the shipped keymap the palette would be reading"
        );

        let refused = prepare(&running, replace(Action::SelectMarkerTool, "F"))
            .expect_err("with nothing queued, Pen still holds the chord");
        assert_eq!(
            refused, "Shortcut not changed — F is already assigned to Pen Tool.",
            "and that refusal is the honest one while no edit is outstanding"
        );

        let write = prepare_behind(
            &running,
            &[(Action::SelectPenTool, "Ctrl+Alt+Shift+P")],
            replace(Action::SelectMarkerTool, "F"),
        )
        .expect("the chord the queued edit is giving up is free");

        assert_eq!(write.request.action, Action::SelectMarkerTool);
        assert_eq!(write.bindings, ["F".to_string()]);
    }

    /// The other direction: a chord an outstanding edit has asked *for* is
    /// taken, though no keymap holds it yet.
    #[test]
    fn a_chord_an_in_flight_edit_asked_for_is_already_taken() {
        let running = KeybindingsConfig::default();

        let refused = prepare_behind(
            &running,
            &[(Action::SelectPenTool, "Ctrl+Alt+Shift+P")],
            replace(Action::SelectMarkerTool, "Ctrl+Alt+Shift+P"),
        )
        .expect_err("the queued edit has already asked for this chord");

        assert_eq!(
            refused, "Shortcut not changed — Ctrl+Alt+Shift+P is already assigned to Pen Tool.",
            "and the refusal names the action that asked for it, not a keymap holder"
        );
    }

    fn prepared() -> KeybindingEditWrite {
        prepare(
            &KeybindingsConfig::default(),
            replace(Action::SelectPenTool, "Ctrl+Alt+Shift+K"),
        )
        .expect("a free chord is accepted")
    }

    /// A landed write is the only outcome that both installs and claims a save.
    #[test]
    fn a_write_that_landed_installs_the_prepared_delta() {
        let completion = shortcut_completion(
            prepared(),
            Ok(ConfigEditOutcome {
                backup_path: None,
                write: ConfigEditWrite::Wrote,
            }),
        );

        assert!(completion.saved);
        assert_eq!(completion.message, "Updated shortcut for Pen Tool.");
        let install = completion.install.expect("a landed write installs");
        let installed = install_keybinding_edit(&KeybindingsConfig::default(), &install)
            .expect("the delta folds into the keymap it was prepared against");
        assert_eq!(
            installed
                .keybindings
                .bindings_for_action(Action::SelectPenTool),
            Some(&["Ctrl+Alt+Shift+K".to_string()][..])
        );
    }

    /// A file that already said this installs too — it is what the file holds —
    /// but the wording must not claim a save that did not happen.
    #[test]
    fn a_write_with_nothing_to_do_installs_and_says_so() {
        let completion = shortcut_completion(
            prepared(),
            Ok(ConfigEditOutcome {
                backup_path: None,
                write: ConfigEditWrite::AlreadyCurrent,
            }),
        );

        assert!(completion.saved);
        assert_eq!(completion.message, "Pen Tool already uses that shortcut.");
        assert!(completion.install.is_some());
        assert_eq!(
            completion.file,
            ShortcutFileOutcome::AlreadyCurrent,
            "the file holds the shortcut, but this gesture is not what put it \
             there, and the question asked after this one is worded on that"
        );
    }

    /// The refusal: the file gave the chord away since this run read it, so
    /// nothing is installed. This is the property the move off the dispatch
    /// thread had to preserve — severing the completion from the install, by
    /// installing regardless of the answer, fails here.
    #[test]
    fn a_chord_claimed_on_disk_installs_nothing_and_names_the_owner() {
        let completion = shortcut_completion(
            prepared(),
            Err(anyhow::anyhow!(ShortcutClaimedOnDisk {
                binding: "Ctrl+Alt+Shift+K".to_string(),
                claimed_by: Action::Undo,
            })),
        );

        assert!(
            completion.install.is_none(),
            "a refused edit must leave the run holding its old keymap"
        );
        assert!(!completion.saved);
        assert_eq!(
            completion.message,
            "Shortcut not changed — config.toml now assigns Ctrl+Alt+Shift+K to Undo."
        );
    }

    /// Every other failure degrades rather than refusing: the shortcut works for
    /// the run and the toast says the file missed it.
    #[test]
    fn a_failed_write_still_installs_for_the_run() {
        let completion = shortcut_completion(prepared(), Err(anyhow::anyhow!("the disk is full")));

        assert!(
            completion.install.is_some(),
            "throwing away a shortcut the user just typed is the worse outcome"
        );
        assert!(!completion.saved);
        assert_eq!(completion.message, SHORTCUT_SAVE_FAILED);
        assert_eq!(
            completion.file,
            ShortcutFileOutcome::Rejected,
            "the file is not carrying this shortcut, and anything said later \
             about it has to start from that"
        );
    }

    /// A write that landed without reading back is still a file that does not
    /// hold the shortcut.
    ///
    /// The file *did* change, which is why the message sends the user to it
    /// rather than claiming it is untouched — but the value is not in it, so
    /// nothing downstream may treat this as a save.
    #[test]
    fn a_write_that_does_not_read_back_leaves_the_file_without_the_shortcut() {
        let completion = shortcut_completion(
            prepared(),
            Err(anyhow::anyhow!(ConfigEditNotReadBack {
                what: "Shortcut".to_string(),
                path: PathBuf::from("/somewhere/config.toml"),
            })),
        );

        assert_eq!(completion.message, SHORTCUT_WRITE_UNVERIFIED);
        assert!(!completion.saved);
        assert_eq!(completion.file, ShortcutFileOutcome::Rejected);
    }

    /// Two edits in flight at once, and both must survive.
    ///
    /// The palette rebinds Pen and then Marker before the first write finishes,
    /// so the second is prepared against a keymap that does not have the first
    /// edit in it yet. Installing what each *write* was prepared with would let
    /// the second put Pen back on `F` while its toast claimed a save, and the
    /// file — which has both — would disagree with the run until restart. Each
    /// completion installs its own action's bindings and nothing else.
    #[test]
    fn a_second_edits_completion_keeps_the_first_edits_chord() {
        let running = KeybindingsConfig::default();
        assert_eq!(
            running.bindings_for_action(Action::SelectPenTool),
            Some(&["F".to_string()][..]),
            "the fixture is the shipped keymap the palette would be reading"
        );
        assert_eq!(
            running.bindings_for_action(Action::SelectMarkerTool),
            Some(&["H".to_string()][..])
        );

        // Both accepted before either write reports back, so both are prepared
        // against the same starting keymap.
        let pen = prepare(&running, replace(Action::SelectPenTool, "Ctrl+Alt+Shift+P"))
            .expect("a free chord is accepted");
        let marker = prepare(
            &running,
            replace(Action::SelectMarkerTool, "Ctrl+Alt+Shift+M"),
        )
        .expect("a free chord is accepted");

        let landed = || {
            Ok(ConfigEditOutcome {
                backup_path: None,
                write: ConfigEditWrite::Wrote,
            })
        };
        let pen_completion = shortcut_completion(pen, landed());
        let after_pen = install_keybinding_edit(
            &running,
            &pen_completion.install.expect("a landed write installs"),
        )
        .expect("the first delta folds in");

        let marker_completion = shortcut_completion(marker, landed());
        let after_both = install_keybinding_edit(
            &after_pen.keybindings,
            &marker_completion.install.expect("a landed write installs"),
        )
        .expect("the second delta folds into what the first left");

        assert_eq!(
            after_both
                .keybindings
                .bindings_for_action(Action::SelectPenTool),
            Some(&["Ctrl+Alt+Shift+P".to_string()][..]),
            "the second completion must not take the first edit back out"
        );
        assert_eq!(
            after_both
                .keybindings
                .bindings_for_action(Action::SelectMarkerTool),
            Some(&["Ctrl+Alt+Shift+M".to_string()][..])
        );
        let pen_chord = KeyBinding::parse("Ctrl+Alt+Shift+P").expect("a parseable chord");
        let marker_chord = KeyBinding::parse("Ctrl+Alt+Shift+M").expect("a parseable chord");
        assert_eq!(
            after_both.action_map.get(&pen_chord),
            Some(&Action::SelectPenTool)
        );
        assert_eq!(
            after_both.action_map.get(&marker_chord),
            Some(&Action::SelectMarkerTool),
            "both runtime views are rebuilt from the keymap that holds both edits"
        );
        assert_eq!(
            after_both.action_bindings.get(&Action::SelectPenTool),
            Some(&vec![pen_chord])
        );
        assert_eq!(
            after_both.action_bindings.get(&Action::SelectMarkerTool),
            Some(&vec![marker_chord])
        );

        // And each gesture gets its own toast, claiming its own save.
        assert!(pen_completion.saved);
        assert_eq!(pen_completion.message, "Updated shortcut for Pen Tool.");
        assert!(marker_completion.saved);
        assert_eq!(
            marker_completion.message,
            "Updated shortcut for Marker Tool."
        );
    }

    /// The one state a delta install can refuse, and what the user is told.
    ///
    /// Two deltas contesting one chord do not normally meet here: the claim
    /// check reads the running keymap with the outstanding deltas folded in, so
    /// the second gesture is refused before it is ever queued. What still
    /// reaches this point is a projection that did not come true — a completion
    /// in front installing something other than what it promised, or nothing at
    /// all — leaving the run dispatching a chord the arriving delta wants. The
    /// run cannot dispatch a keymap with two actions on one chord, so it keeps
    /// the one it has and says which way the two now disagree.
    ///
    /// The pair is built directly rather than driven through a sequence: what is
    /// under test is the refusal and its wording, and every route to it ends in
    /// the same two values.
    #[test]
    fn an_edit_the_running_keymap_cannot_take_is_not_installed() {
        let running = KeybindingsConfig::default();
        // The chord in the run's keymap, put there the way a failed write does:
        // kept for the session, with the file never hearing about it.
        let degraded = shortcut_completion(
            prepare(&running, replace(Action::SelectPenTool, "Ctrl+Alt+Shift+P"))
                .expect("a free chord is accepted"),
            Err(anyhow::anyhow!("the disk is full")),
        );
        let after_pen = install_keybinding_edit(
            &running,
            &degraded.install.expect("a failed write still installs"),
        )
        .expect("the first delta folds in");

        // The second edit, checked against a keymap that does not carry the
        // first chord — which is what a projection promises and a completion can
        // fail to deliver — and written to a file that never got it either.
        let marker = prepare(
            &running,
            replace(Action::SelectMarkerTool, "Ctrl+Alt+Shift+P"),
        )
        .expect("the chord is free in the keymap this was prepared against");
        let completion = shortcut_completion(
            marker,
            Ok(ConfigEditOutcome {
                backup_path: None,
                write: ConfigEditWrite::Wrote,
            }),
        );

        assert!(
            install_keybinding_edit(
                &after_pen.keybindings,
                &completion.install.expect("a landed write installs"),
            )
            .is_err(),
            "two actions on one chord is not a keymap the run can dispatch from"
        );
        assert_eq!(
            after_pen
                .keybindings
                .bindings_for_action(Action::SelectPenTool),
            Some(&["Ctrl+Alt+Shift+P".to_string()][..]),
            "and the refusal leaves the run holding what it had"
        );
        assert_eq!(
            completion.file,
            ShortcutFileOutcome::Wrote,
            "the file took this one, which is what the wording below rests on"
        );
        assert_eq!(
            shortcut_not_installed_message(completion.file),
            SHORTCUT_NOT_INSTALLED
        );
        assert_eq!(
            SHORTCUT_NOT_INSTALLED,
            "Shortcut saved to config.toml, but this run kept its own — another \
             edit here already uses that key (see logs).",
            "the wording must not claim the run took it"
        );
    }

    /// The same collision over a file that was never written at all.
    ///
    /// The second edit asks for a shortcut `config.toml` already resolves to, so
    /// the write has no delta, touches nothing, and spends no backup — and the
    /// run still cannot take a second action onto a chord the first edit is
    /// dispatching. Reporting "saved to config.toml" here would credit this
    /// gesture with a write it never made and send the user to a `.bak` that
    /// does not exist; what is true is that the file and the run disagree.
    #[test]
    fn an_edit_the_file_already_had_and_the_run_cannot_take_must_not_claim_a_save() {
        let running = KeybindingsConfig::default();
        let contested = "Ctrl+Alt+Shift+P";
        let degraded = shortcut_completion(
            prepare(&running, replace(Action::SelectPenTool, contested))
                .expect("a free chord is accepted"),
            Err(anyhow::anyhow!("the disk is full")),
        );
        let after_pen = install_keybinding_edit(
            &running,
            &degraded.install.expect("a failed write still installs"),
        )
        .expect("the first delta folds in");

        // Checked against the same keymap without the first chord in it, and
        // answered by a file that already says exactly this.
        let marker = prepare(&running, replace(Action::SelectMarkerTool, contested))
            .expect("the chord is free in the keymap this was checked against");
        let completion = shortcut_completion(
            marker,
            Ok(ConfigEditOutcome {
                backup_path: None,
                write: ConfigEditWrite::AlreadyCurrent,
            }),
        );

        assert_eq!(
            completion.file,
            ShortcutFileOutcome::AlreadyCurrent,
            "the file holds the shortcut, and no write of this gesture's put it there"
        );
        assert!(
            install_keybinding_edit(
                &after_pen.keybindings,
                &completion
                    .install
                    .expect("a file that already agreed still offers its delta"),
            )
            .is_err(),
            "two actions on one chord is not a keymap the run can dispatch from"
        );
        assert_eq!(
            shortcut_not_installed_message(completion.file),
            SHORTCUT_ALREADY_CURRENT_NOT_INSTALLED
        );
        assert_eq!(
            SHORTCUT_ALREADY_CURRENT_NOT_INSTALLED,
            "config.toml already has this shortcut, but this run kept its own — \
             another edit here already uses that key (see logs).",
        );
        assert!(
            !SHORTCUT_ALREADY_CURRENT_NOT_INSTALLED.contains("saved to config.toml"),
            "nothing was written, so nothing may report a save"
        );
    }

    /// The same collision with the file on the other side: neither edit landed.
    ///
    /// Both writes failed, so `config.toml` has neither shortcut, and the run
    /// still cannot put two actions on one chord. The refusal used to borrow the
    /// wording of the case above and tell the user their shortcut was "saved to
    /// config.toml" — over a file that never received either edit.
    #[test]
    fn an_edit_neither_the_file_nor_the_run_took_must_not_claim_a_save() {
        let running = KeybindingsConfig::default();
        let contested = "Ctrl+Alt+Shift+P";
        let degraded = shortcut_completion(
            prepare(&running, replace(Action::SelectPenTool, contested))
                .expect("a free chord is accepted"),
            Err(anyhow::anyhow!("the disk is full")),
        );
        let after_pen = install_keybinding_edit(
            &running,
            &degraded.install.expect("a failed write still installs"),
        )
        .expect("the first delta folds in");

        // The second edit is checked against the same keymap without the first
        // chord in it, and its write fails the same way the first one's did —
        // the disk did not get better in between.
        let marker = prepare(&running, replace(Action::SelectMarkerTool, contested))
            .expect("the chord is free in the keymap this was checked against");
        let completion = shortcut_completion(marker, Err(anyhow::anyhow!("the disk is full")));

        assert_eq!(
            completion.file,
            ShortcutFileOutcome::Rejected,
            "the file got neither edit"
        );
        assert!(
            install_keybinding_edit(
                &after_pen.keybindings,
                &completion
                    .install
                    .expect("a failed write still offers its delta"),
            )
            .is_err(),
            "two actions on one chord is not a keymap the run can dispatch from"
        );
        assert_eq!(
            shortcut_not_installed_message(completion.file),
            SHORTCUT_NOT_SAVED_OR_INSTALLED
        );
        assert_eq!(
            SHORTCUT_NOT_SAVED_OR_INSTALLED,
            "Shortcut not changed — config.toml did not take it and another edit \
             here already uses that key (see logs).",
        );
        assert!(
            !SHORTCUT_NOT_SAVED_OR_INSTALLED.contains("saved to config.toml"),
            "nothing reached the file, so nothing may send the user to it"
        );
    }

    /// The persistence fixture: hand-authored text the write must respect.
    ///
    /// It deliberately holds everything the loader changes in memory —
    /// comments, an unrelated pair of actions contesting one chord, an
    /// unparseable binding, and a setting this build does not know — so that a
    /// write which leaked validation results would be obvious in the diff.
    const AUTHORED_FILE: &str = "\
# Wayscriber configuration. These comments must survive a shortcut edit.

[keybindings]
# The shortcut under test.
select_pen_tool = [\"Ctrl+Alt+Shift+P\"]
# A contested pair: the loader gives the chord to one of them for the session
# and reports the other, but must never repair the file.
undo = [\"Ctrl+Alt+Shift+Q\"]
redo = [\"Ctrl+Alt+Shift+Q\"]
# Nonsense the loader drops for the session and keeps on disk.
clear_canvas = [\"NotARealKey\"]

[ui]
# An unrelated section, plus a key from some future release.
show_status_bar = false
setting_from_a_later_release = 7
";

    /// The chord the stale-edit fixture below hands to another action.
    const CONTESTED_CHORD: &str = "Ctrl+Alt+Shift+M";

    /// The file as it stands *after* the palette read its keymap: `undo` has
    /// taken the chord the edit is about to ask for, and the action being
    /// edited is one this file omits — which is what used to make validation
    /// treat the requested list as a droppable offer.
    const CLAIMED_ON_DISK_FILE: &str = "\
# A writer this run never saw got here first.
[keybindings]
undo = [\"Ctrl+Alt+Shift+M\"]
";

    fn config_in(dir: &Path) -> PathBuf {
        dir.join("config.toml")
    }

    fn write_fixture(dir: &Path) -> PathBuf {
        let path = config_in(dir);
        fs::write(&path, AUTHORED_FILE).expect("the fixture should be written");
        path
    }

    /// The core durability property: the edited key moves and the file is
    /// otherwise byte-identical, comments and all.
    #[test]
    fn a_shortcut_write_changes_exactly_its_own_key() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());

        persist_keybinding_edit_at(
            &path,
            Action::SelectPenTool,
            &["Ctrl+Alt+Shift+K".to_string()],
        )
        .expect("the write should succeed");

        let after = fs::read_to_string(&path).expect("the config should be readable");
        assert_eq!(
            after,
            AUTHORED_FILE.replace(
                "select_pen_tool = [\"Ctrl+Alt+Shift+P\"]",
                "select_pen_tool = [\"Ctrl+Alt+Shift+K\"]"
            ),
            "a shortcut write must change one key and nothing else"
        );
    }

    /// Unbind and reset write through the same one-key path.
    #[test]
    fn unbinding_and_resetting_also_change_exactly_their_own_key() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());

        persist_keybinding_edit_at(&path, Action::SelectPenTool, &[])
            .expect("an unbind should be writable");
        assert_eq!(
            fs::read_to_string(&path).expect("readable"),
            AUTHORED_FILE.replace(
                "select_pen_tool = [\"Ctrl+Alt+Shift+P\"]",
                "select_pen_tool = []"
            )
        );

        // Reset writes the shipped list out in full rather than deleting the
        // key. Removing it would hand the action back to presence-based
        // resolution, where the same default can stand down against another
        // binding — the user asked for the default, not for the offer of one.
        // (Deleting the key instead is a possible follow-up, but it is a
        // different promise.)
        let default = KeybindingsConfig::default()
            .bindings_for_action(Action::SelectPenTool)
            .map(<[String]>::to_vec)
            .expect("the pen tool ships a shortcut");
        persist_keybinding_edit_at(&path, Action::SelectPenTool, &default)
            .expect("a reset should be writable");
        assert_eq!(
            fs::read_to_string(&path).expect("readable"),
            AUTHORED_FILE.replace(
                "select_pen_tool = [\"Ctrl+Alt+Shift+P\"]",
                "select_pen_tool = [\"F\"]"
            )
        );
    }

    #[test]
    fn a_shortcut_write_leaves_a_timestamped_backup() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());

        let outcome = persist_keybinding_edit_at(
            &path,
            Action::SelectPenTool,
            &["Ctrl+Alt+Shift+K".to_string()],
        )
        .expect("the write should succeed");

        let backup = outcome.backup_path.expect("an existing file is backed up");
        assert_eq!(
            fs::read_to_string(&backup).expect("the backup should be readable"),
            AUTHORED_FILE,
            "the backup holds the contents from before the write"
        );
    }

    /// Presence is what tells an authored shortcut from a compiled-in offer, so
    /// an action the file omitted has to come back explicit once it is written.
    #[test]
    fn a_written_shortcut_reloads_as_explicitly_authored() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());
        assert!(
            !AUTHORED_FILE.contains("select_step_marker_tool"),
            "the fixture must omit the action this test writes"
        );

        persist_keybinding_edit_at(
            &path,
            Action::SelectStepMarkerTool,
            &["Ctrl+Alt+Shift+M".to_string()],
        )
        .expect("the write should succeed");

        let reloaded = crate::config::ConfigDocument::load_from_path(&path)
            .expect("the written config should reload");
        assert!(
            reloaded
                .keybinding_authorship()
                .is_explicit("select_step_marker_tool"),
            "the written key must read back as authored"
        );
        assert_eq!(
            reloaded
                .config()
                .keybindings
                .bindings_for_action(Action::SelectStepMarkerTool),
            Some(&["Ctrl+Alt+Shift+M".to_string()][..])
        );
    }

    /// Reset on an action that already resolves to the shipped shortcut has no
    /// delta to write, so nothing is written — and the caller is told that
    /// rather than being handed a success message for a save that never
    /// happened. Most actions are in this state: the file simply omits them.
    #[test]
    fn resetting_an_omitted_action_already_at_its_default_writes_nothing() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());
        assert!(
            !AUTHORED_FILE.contains("select_marker_tool"),
            "the fixture must omit the action this test resets"
        );
        let default = KeybindingsConfig::default()
            .bindings_for_action(Action::SelectMarkerTool)
            .map(<[String]>::to_vec)
            .expect("the marker tool has a stored shortcut list");
        assert_eq!(default, ["H"], "and it must ship a shortcut to reset to");

        let outcome = persist_keybinding_edit_at(&path, Action::SelectMarkerTool, &default)
            .expect("a no-op edit is not a failure");

        assert_eq!(
            outcome.write,
            ConfigEditWrite::AlreadyCurrent,
            "the file already resolved to this, so nothing was written"
        );
        assert!(
            outcome.backup_path.is_none(),
            "a write that did not happen must not spend a backup"
        );
        assert_eq!(fs::read_to_string(&path).expect("readable"), AUTHORED_FILE);
        assert_eq!(
            shortcut_unchanged_message(&KeybindingEditRequest {
                action: Action::SelectMarkerTool,
                operation: KeybindingEditOperation::Reset,
            }),
            "Marker Tool already uses the default shortcut.",
            "the toast must not claim a durable change this made"
        );
    }

    /// The other Reset case: the file authors something else, so the default is
    /// a real delta — it is written, which is what pins it against a future
    /// build changing the shipped value.
    #[test]
    fn resetting_an_authored_shortcut_writes_the_default_and_reports_it() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());
        let default = KeybindingsConfig::default()
            .bindings_for_action(Action::SelectPenTool)
            .map(<[String]>::to_vec)
            .expect("the pen tool ships a shortcut");

        let outcome = persist_keybinding_edit_at(&path, Action::SelectPenTool, &default)
            .expect("a reset should be writable");

        assert_eq!(outcome.write, ConfigEditWrite::Wrote);
        assert!(outcome.backup_path.is_some());
        assert_eq!(
            fs::read_to_string(&path).expect("readable"),
            AUTHORED_FILE.replace(
                "select_pen_tool = [\"Ctrl+Alt+Shift+P\"]",
                "select_pen_tool = [\"F\"]"
            )
        );
    }

    /// The reviewer's race, staged exactly: the palette's conflict check passed
    /// against the keymap this run loaded, and by the time the write runs the
    /// file gives that chord to another action.
    ///
    /// The edit is refused before anything is written. Letting it through wrote
    /// an *empty* list for the edited action — validation drops a list it still
    /// reads as an unauthored offer against the newer claimant, and the merge
    /// gate writes that difference — after which the read-back check failed and
    /// the caller reported a save failure over a file it had just changed.
    #[test]
    fn a_chord_claimed_on_disk_since_the_edit_is_refused_without_writing() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = config_in(temp.path());
        fs::write(&path, CLAIMED_ON_DISK_FILE).expect("the fixture should be written");
        assert!(
            !CLAIMED_ON_DISK_FILE.contains("clear_canvas"),
            "the edited action must be one the file omits, as in the report"
        );
        assert!(
            !KeybindingsConfig::default()
                .bindings_for_action(Action::ClearCanvas)
                .is_none_or(<[String]>::is_empty),
            "and one with a shipped shortcut, or there is no value to empty out"
        );

        let error =
            persist_keybinding_edit_at(&path, Action::ClearCanvas, &[CONTESTED_CHORD.to_string()])
                .expect_err("the file gives that chord to another action");

        let conflict = error
            .downcast_ref::<ShortcutClaimedOnDisk>()
            .unwrap_or_else(|| {
                panic!("the caller must be able to tell a refusal apart: {error:#}")
            });
        assert_eq!(conflict.claimed_by, Action::Undo);
        assert_eq!(conflict.binding, CONTESTED_CHORD);
        assert_eq!(
            fs::read_to_string(&path).expect("readable"),
            CLAIMED_ON_DISK_FILE,
            "a refused edit must leave the file byte-identical"
        );
        assert_eq!(
            shortcut_claimed_on_disk_message(&conflict.binding, conflict.claimed_by),
            "Shortcut not changed — config.toml now assigns Ctrl+Alt+Shift+M to Undo.",
            "the refusal names the action that owns the chord"
        );
    }

    /// The invariant the delta install rests on, staged through real writes.
    ///
    /// The palette's own check refuses this pair before it is queued — the
    /// second edit is checked against the first one's outstanding delta — but
    /// that check only knows about this run's queue. Another window, the
    /// configurator, or a hand edit can take a chord it has never heard of, so
    /// the write re-reads the file it is about to change and refuses there. It
    /// is the *file*, not the run, that is the authority on a contest, and the
    /// second edit is refused rather than installed. That is why folding deltas
    /// into the current keymap cannot put two actions on one chord.
    ///
    /// The contesting pair is therefore staged directly, by checking each edit
    /// against a keymap that carries neither: what is under test is what the
    /// worker does with two of them, not how they got past the palette.
    #[test]
    fn an_overlapping_edit_onto_the_chord_the_first_took_is_refused_by_the_file() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());
        let running = KeybindingsConfig::default();
        let contested = "Ctrl+Alt+Shift+X";

        let pen = prepare(&running, replace(Action::SelectPenTool, contested))
            .expect("the chord is free in this run's keymap");
        let marker = prepare(&running, replace(Action::SelectMarkerTool, contested))
            .expect("and still free, because the first edit is not installed");

        // The worker writes them in submission order.
        persist_keybinding_edit_at(&path, pen.request.action, &pen.bindings)
            .expect("the first write should land");
        let error = persist_keybinding_edit_at(&path, marker.request.action, &marker.bindings)
            .expect_err("the second write reads the file the first one changed");

        let completion = shortcut_completion(marker, Err(error));
        assert!(
            completion.install.is_none(),
            "a chord the file has just given away is refused, not folded in"
        );
        assert!(!completion.saved);
        assert_eq!(
            completion.message,
            "Shortcut not changed — config.toml now assigns Ctrl+Alt+Shift+X to Pen Tool.",
            "and the refusal names the action that took it"
        );
        let after = fs::read_to_string(&path).expect("readable");
        assert!(
            after.contains("select_pen_tool = [\"Ctrl+Alt+Shift+X\"]"),
            "the first edit is what the file kept: {after}"
        );
        assert!(
            !after.contains("select_marker_tool"),
            "and the refused edit wrote nothing: {after}"
        );
    }

    /// What loading decided in memory stays in memory, even when the write
    /// marks a key authored on its way past.
    ///
    /// The fixture spends `undo`'s shipped `Ctrl+Z` on an authored
    /// `toggle_input_hud`, so loading stands the omitted `undo` default down.
    /// That decision belongs to the session: an edit to an unrelated action
    /// must not pin it into the file, in either direction.
    #[test]
    fn an_edit_leaves_the_omitted_default_that_stood_down_alone() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = config_in(temp.path());
        let authored = "[keybindings]\ntoggle_input_hud = [\"Ctrl+Z\"]\n";
        fs::write(&path, authored).expect("the fixture should be written");

        persist_keybinding_edit_at(
            &path,
            Action::SelectPenTool,
            &["Ctrl+Alt+Shift+K".to_string()],
        )
        .expect("the write should succeed");

        assert_eq!(
            fs::read_to_string(&path).expect("readable"),
            format!("{authored}select_pen_tool = [\"Ctrl+Alt+Shift+K\"]\n"),
            "only the edited action's key may appear"
        );
    }

    /// A config the process cannot write is not a reason to lose the shortcut:
    /// the caller keeps the in-memory edit and is told the file missed it.
    #[test]
    fn a_read_only_config_fails_the_write_and_leaves_the_file_alone() {
        use std::os::unix::fs::PermissionsExt;

        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());
        // The file mode alone would not stop it: the write is an atomic
        // replace, so it is the directory that has to refuse the new entry
        // (and the backup copy that lands beside it).
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).expect("chmod file");
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o555)).expect("chmod dir");

        let result = persist_keybinding_edit_at(
            &path,
            Action::SelectPenTool,
            &["Ctrl+Alt+Shift+K".to_string()],
        );

        // Restore before asserting so a failure cannot leave an unremovable
        // directory behind for the harness.
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o755)).expect("restore dir");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("restore file");

        if unsafe { libc::geteuid() } == 0 {
            // Root ignores these modes; the scenario cannot be staged.
            return;
        }
        assert!(result.is_err(), "a read-only config must fail the write");
        assert_eq!(
            fs::read_to_string(&path).expect("readable"),
            AUTHORED_FILE,
            "a failed write must leave the file exactly as it was"
        );
        assert_eq!(
            SHORTCUT_SAVE_FAILED,
            "Shortcut updated for this run, but saving to config.toml failed (see logs).",
            "the degradation message is what the user sees for this case"
        );
    }

    /// An unparseable config is repaired in the configurator, deliberately and
    /// with the damage on screen — never as a side effect of a rebind.
    #[test]
    fn an_unparseable_config_is_refused_rather_than_rebuilt_from_defaults() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = config_in(temp.path());
        fs::write(&path, "[keybindings\nundo = broken").expect("the fixture should be written");

        let error = persist_keybinding_edit_at(
            &path,
            Action::SelectPenTool,
            &["Ctrl+Alt+Shift+K".to_string()],
        )
        .expect_err("a broken config must not be silently replaced");

        assert!(
            format!("{error:#}").contains("could not be parsed"),
            "error: {error:#}"
        );
        assert_eq!(
            fs::read_to_string(&path).expect("readable"),
            "[keybindings\nundo = broken"
        );
    }

    /// The retry exists for a real error string produced by a real stale save,
    /// so this stages that save rather than trusting the wording.
    #[test]
    fn a_stale_document_is_recognised_and_retried() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = write_fixture(temp.path());

        let document = crate::config::ConfigDocument::load_from_path(&path).expect("load");
        fs::write(&path, format!("{AUTHORED_FILE}\n# a second writer\n")).expect("outside write");
        let error = document
            .save_with_backup(document.config().clone())
            .expect_err("the document is stale");

        assert!(
            is_stale_source_error(&error),
            "the retry gate must recognise this error: {error:#}"
        );

        // And the public path recovers from exactly that situation.
        persist_keybinding_edit_at(
            &path,
            Action::SelectPenTool,
            &["Ctrl+Alt+Shift+K".to_string()],
        )
        .expect("a fresh load succeeds");
        assert!(
            fs::read_to_string(&path)
                .expect("readable")
                .contains("select_pen_tool = [\"Ctrl+Alt+Shift+K\"]")
        );
    }

    /// Restart semantics, now the other way round: what this run wrote is what
    /// the next process loads.
    #[test]
    fn a_fresh_load_returns_the_written_shortcut() {
        with_temp_config_home(|config_root| {
            let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
            fs::create_dir_all(&config_dir).expect("test config directory");
            let path = config_dir.join("config.toml");
            fs::write(&path, AUTHORED_SHORTCUTS).expect("test config should be written");

            // Through the environment-resolved entry point the overlay calls,
            // so the wiring from `Config::get_config_path` is covered too.
            persist_keybinding_edit(Action::SelectPenTool, &["Ctrl+Alt+Shift+K".to_string()])
                .expect("the write should succeed");

            let restarted = Config::load().expect("test config should reload").config;
            assert_eq!(
                restarted
                    .keybindings
                    .bindings_for_action(Action::SelectPenTool),
                Some(&["Ctrl+Alt+Shift+K".to_string()][..])
            );
        });
    }
}
