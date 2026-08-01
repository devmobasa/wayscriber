use super::*;

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
mod tests;
