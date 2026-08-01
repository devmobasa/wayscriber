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

mod implementation;

pub(in crate::backend::wayland) use implementation::queue_keybinding_edit;
